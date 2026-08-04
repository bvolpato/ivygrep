// Search execution composes bounded retrieval passes. Candidate algorithms remain
// in the parent module so each pass stays independently testable.
use super::*;

pub(crate) fn hybrid_search_with_context_and_neural_job(
    ctx: &SearchContext,
    workspace: &Workspace,
    query_text: &str,
    embedding_model: Option<&dyn EmbeddingModel>,
    options: &SearchOptions,
    mut neural_query_vector_job: Option<NeuralQueryVectorJob>,
) -> Result<Vec<SearchHit>> {
    let query_text = query_text.trim();
    // An empty/whitespace query has no lexical or literal terms; without this
    // guard the semantic pass would still embed "" and return arbitrary
    // nearest-neighbour noise. Match literal_search and return nothing.
    if query_text.is_empty() {
        return Ok(Vec::new());
    }

    let t0 = std::time::Instant::now();
    let bounded_limit = options.bounded_limit();
    let output_limit = bounded_limit.unwrap_or(DEFAULT_SEARCH_LIMIT);
    let mut routing = QueryRouting::classify(query_text);
    if options.force_neural {
        routing.use_neural = true;
    }
    let corpus_multiplier =
        corpus_candidate_multiplier(ctx.searchers.iter().map(tantivy::Searcher::num_docs).sum());
    // Tantivy lexical candidates: enough headroom for post-hoc filters
    // (gitignore, scope, globs) without blowing up on huge repos.
    // Default natural-language query: 50 → 250, --limit 500 → 2.5K.
    let candidate_limit = if output_limit == usize::MAX {
        50_000
    } else {
        output_limit
            .saturating_mul(routing.lexical_multiplier)
            .clamp(150, 50_000)
    };
    // Literal pass needs exact substring verification via SQLite (text not
    // stored in Tantivy), so cap tighter: default → 250, scales up with limit.
    let literal_limit = if output_limit == usize::MAX {
        25_000
    } else {
        output_limit
            .saturating_mul(routing.literal_multiplier)
            .saturating_mul(corpus_multiplier)
            .clamp(250, 25_000)
    };
    // Semantic (vector ANN) search: keep proportional but bounded.
    // Default ~50 → 50, --limit 500 → 500, --limit 5000 → 2000.
    // k=200 is ~30ms on 3M vectors; k=2000 is ~200ms. Both acceptable.
    let semantic_limit = if output_limit == usize::MAX {
        2_000
    } else {
        output_limit
            .saturating_mul(routing.semantic_multiplier)
            .saturating_mul(corpus_multiplier)
            .clamp(50, 2_000)
    };
    let path_matcher = PathGlobMatcher::new(&options.include_globs, &options.exclude_globs)?;
    let glob_path_filter = build_glob_path_query_filter(ctx, &path_matcher, options)?;

    tracing::trace!("open_tantivy={:?}", t0.elapsed());

    if options.is_cancelled() {
        return Ok(Vec::new());
    }

    // ── Literal pass ────────────────────────────────────────────────────
    // Always run a fast index-backed literal substring scan so exact matches
    // surface even when tokenization splits them differently.
    // Build a regex alternation of the original query plus snake_case/camelCase
    // variants so "hybrid search" also matches "hybrid_search" and "hybridSearch".
    let trimmed = query_text;
    // Compute once — used by literal pass, lexical pass, and path-match pass.
    let lexical_queries = build_lexical_queries(trimmed);
    let literal_queries = build_literal_queries(trimmed, &lexical_queries);
    let symbol_candidate_limit = output_limit.clamp(20, routing.symbol_limit);
    let literal_matcher = if !literal_queries.is_empty() {
        Some(LiteralMatcher::from_queries(
            literal_queries.iter().map(String::as_str),
            true,
        )?)
    } else {
        None
    };
    let literal_chunks: Vec<(IndexedChunk, f32)> = if let Some(ref matcher) = literal_matcher {
        let target_hits = bounded_limit
            .unwrap_or(100)
            .saturating_mul(literal_queries.len())
            .min(literal_limit);
        let all_candidates = collect_literal_candidates_for_queries(
            ctx,
            &literal_queries,
            matcher,
            &path_matcher,
            &glob_path_filter,
            options,
            (literal_limit, target_hits),
        )?;
        tracing::trace!(
            "literal_pass={:?} found={}",
            t0.elapsed(),
            all_candidates.len()
        );
        all_candidates
            .into_iter()
            .map(|c| {
                let count = matcher.match_count(&c.text).max(1) as f32;
                let score = 1.0 + (count - 1.0).min(4.0) * 0.15; // 1.0 → 1.6 for 5+ matches
                (c, score)
            })
            .collect()
    } else {
        Vec::new()
    };
    if options.is_cancelled() {
        return Ok(Vec::new());
    }

    // ── Lexical (BM25) pass ─────────────────────────────────────────────
    // BM25F: search across text, tokenized file path, and definition signature.
    // Boosts on path/signature fields implement Sourcegraph-style BM25F where
    // matches on filenames and symbol definitions count 5× more than body text.
    let mut search_fields = vec![ctx.fields.text, ctx.fields.file_path];
    if let Some(f) = ctx.fields.file_path_text {
        search_fields.push(f);
    }
    if let Some(f) = ctx.fields.signature {
        search_fields.push(f);
    }
    let mut parser = QueryParser::for_index(&ctx.indexes[0], search_fields);
    parser.set_field_boost(ctx.fields.file_path, 2.0);
    if let Some(f) = ctx.fields.file_path_text {
        parser.set_field_boost(f, 5.0);
    }
    if let Some(f) = ctx.fields.signature {
        parser.set_field_boost(f, 5.0);
    }
    let conjunctive_numeric_query = should_use_conjunctive_numeric_query(trimmed);
    if conjunctive_numeric_query {
        parser.set_conjunction_by_default();
    }

    let mut allowed_languages = Vec::new();
    let mut can_pushdown_languages = options.include_globs.is_empty();
    if let Some(tf) = &options.type_filter {
        let resolved = crate::chunking::resolve_type_alias(tf)
            .map(|s| s.to_string())
            .unwrap_or_else(|| tf.to_string());
        allowed_languages.push(resolved);
        can_pushdown_languages = true;
    } else if !options.include_globs.is_empty() {
        can_pushdown_languages = true;
        for glob in &options.include_globs {
            let trimmed = glob.trim();
            if trimmed.starts_with("*.") && !trimmed.contains('/') && !trimmed.contains('?') {
                let ext = &trimmed[1..];
                if let Some(lang) =
                    crate::chunking::language_for_path(&PathBuf::from(format!("dummy{}", ext)))
                {
                    allowed_languages.push(lang.to_string());
                } else {
                    can_pushdown_languages = false;
                    break;
                }
            } else {
                can_pushdown_languages = false;
                break;
            }
        }
    }

    let mut lexical_by_id = HashMap::<u64, (IndexedChunk, f32)>::new();
    let lexical_search_queries =
        lexical_search_queries_for_routing(&lexical_queries, routing, conjunctive_numeric_query);
    let lexical_query_limits =
        lexical_query_candidate_limits(candidate_limit, lexical_search_queries.len());
    let executor = LexicalQueryExecutor {
        fields: &ctx.fields,
        parser: &parser,
        conjunctive_numeric_query,
        scope_filter: options.scope_filter.as_ref(),
        glob_path_filter: &glob_path_filter,
        can_pushdown_languages,
        allowed_languages: &allowed_languages,
        searchers: &ctx.searchers,
    };
    let collect_docs = |(lexical_query, query_candidate_limit): (&String, usize)| {
        executor.collect_docs(lexical_query, query_candidate_limit)
    };
    let lexical_doc_batches =
        if lexical_search_queries.len() > 1 && rayon::current_num_threads() > 1 {
            lexical_search_queries
                .par_iter()
                .zip(lexical_query_limits)
                .map(collect_docs)
                .collect::<Result<Vec<_>>>()?
        } else {
            lexical_search_queries
                .iter()
                .zip(lexical_query_limits)
                .map(collect_docs)
                .collect::<Result<Vec<_>>>()?
        };
    for docs in lexical_doc_batches {
        for (i, score, doc) in docs {
            if let Some(chunk) = fetch_chunk_by_id(doc, &ctx.fields)
                .filter(|c| !ctx.is_shadowed_base_file(i, &c.file_path))
                .filter(|chunk| type_matches(chunk, options.type_filter.as_deref()))
                .filter(|chunk| scope_matches(chunk, options.scope_filter.as_ref()))
                .filter(|chunk| path_matches(chunk, &path_matcher))
                .filter(|chunk| options.skip_gitignore || !chunk.is_ignored)
            {
                let boosted = if is_definition_kind(&chunk.kind) {
                    score * 2.0
                } else {
                    score
                };
                lexical_by_id
                    .entry(chunk.vector_key)
                    .and_modify(|(_, best)| *best = best.max(boosted))
                    .or_insert((chunk, boosted));
            }
        }
    }
    // Sort by BM25 score and truncate to candidate_limit BEFORE populating
    // text from SQLite. This avoids O(all_results) individual SQLite lookups
    // — we only fetch text for the top-scoring candidates that will survive
    // RRF fusion.
    let mut lexical_chunks = lexical_by_id.into_values().collect::<Vec<_>>();
    lexical_chunks.sort_by(|left, right| {
        right
            .1
            .total_cmp(&left.1)
            .then_with(|| left.0.vector_key.cmp(&right.0.vector_key))
    });
    lexical_chunks.truncate(candidate_limit);
    tracing::trace!(
        "lexical_bm25={:?} candidates={} expansions={}",
        t0.elapsed(),
        lexical_chunks.len(),
        lexical_search_queries.len()
    );

    // Exact persisted symbol definitions provide a separate bounded rank
    // signal. This avoids inferring every definition solely from text while
    // keeping symbol lookup independent from the main candidate volume.
    let exact_symbol_names = exact_symbol_query_names(trimmed);
    let mut exact_symbol_chunks = crate::symbols::definition_candidates(
        &ctx.sqlite,
        &exact_symbol_names,
        symbol_candidate_limit,
    )?;
    if let Some(base_sqlite) = &ctx.base_sqlite {
        let remaining = symbol_candidate_limit.saturating_sub(exact_symbol_chunks.len());
        if remaining > 0 {
            exact_symbol_chunks.extend(
                crate::symbols::definition_candidates(base_sqlite, &exact_symbol_names, remaining)?
                    .into_iter()
                    .filter(|chunk| !ctx.is_shadowed_base_file(1, &chunk.file_path)),
            );
        }
    }
    exact_symbol_chunks.retain(|chunk| {
        type_matches(chunk, options.type_filter.as_deref())
            && scope_matches(chunk, options.scope_filter.as_ref())
            && path_matches(chunk, &path_matcher)
            && (options.skip_gitignore || !chunk.is_ignored)
    });
    exact_symbol_chunks.truncate(symbol_candidate_limit);
    let exact_symbol_ids = exact_symbol_chunks
        .iter()
        .map(|chunk| chunk.vector_key)
        .collect::<HashSet<_>>();

    let remaining = symbol_candidate_limit.saturating_sub(exact_symbol_chunks.len());
    let inferred_symbol_names = natural_language_symbol_queries(trimmed);
    let mut inferred_symbol_chunks = if remaining > 0 && !inferred_symbol_names.is_empty() {
        crate::symbols::definition_candidates(&ctx.sqlite, &inferred_symbol_names, remaining)?
    } else {
        Vec::new()
    };
    if let Some(base_sqlite) = &ctx.base_sqlite {
        let base_remaining = remaining.saturating_sub(inferred_symbol_chunks.len());
        if base_remaining > 0 {
            inferred_symbol_chunks.extend(
                crate::symbols::definition_candidates(
                    base_sqlite,
                    &inferred_symbol_names,
                    base_remaining,
                )?
                .into_iter()
                .filter(|chunk| !ctx.is_shadowed_base_file(1, &chunk.file_path)),
            );
        }
    }
    inferred_symbol_chunks.retain(|chunk| {
        !exact_symbol_ids.contains(&chunk.vector_key)
            && type_matches(chunk, options.type_filter.as_deref())
            && scope_matches(chunk, options.scope_filter.as_ref())
            && path_matches(chunk, &path_matcher)
            && (options.skip_gitignore || !chunk.is_ignored)
    });
    inferred_symbol_chunks.truncate(remaining);
    let inferred_symbol_ids = inferred_symbol_chunks
        .iter()
        .map(|chunk| chunk.vector_key)
        .collect::<HashSet<_>>();

    let remaining = remaining.saturating_sub(inferred_symbol_chunks.len());
    let mut alias_symbol_chunks = if remaining > 0 {
        crate::symbols::definition_candidates(&ctx.sqlite, &lexical_queries, remaining)?
    } else {
        Vec::new()
    };
    if let Some(base_sqlite) = &ctx.base_sqlite {
        let base_remaining = remaining.saturating_sub(alias_symbol_chunks.len());
        if base_remaining > 0 {
            alias_symbol_chunks.extend(
                crate::symbols::definition_candidates(
                    base_sqlite,
                    &lexical_queries,
                    base_remaining,
                )?
                .into_iter()
                .filter(|chunk| !ctx.is_shadowed_base_file(1, &chunk.file_path)),
            );
        }
    }
    alias_symbol_chunks.retain(|chunk| {
        !exact_symbol_ids.contains(&chunk.vector_key)
            && !inferred_symbol_ids.contains(&chunk.vector_key)
            && type_matches(chunk, options.type_filter.as_deref())
            && scope_matches(chunk, options.scope_filter.as_ref())
            && path_matches(chunk, &path_matcher)
            && (options.skip_gitignore || !chunk.is_ignored)
    });
    alias_symbol_chunks.truncate(remaining);

    let symbol_chunks = exact_symbol_chunks
        .into_iter()
        .map(|chunk| (chunk, SymbolCandidateKind::Exact))
        .chain(
            inferred_symbol_chunks
                .into_iter()
                .map(|chunk| (chunk, SymbolCandidateKind::Inferred)),
        )
        .chain(
            alias_symbol_chunks
                .into_iter()
                .map(|chunk| (chunk, SymbolCandidateKind::Alias)),
        )
        .collect::<Vec<_>>();
    tracing::trace!(
        "lexical_symbols={:?} candidates={}",
        t0.elapsed(),
        symbol_chunks.len()
    );

    // ── Path-match pass ──────────────────────────────────────────────────
    // Collect chunks whose file_path contains the query as a directory/file
    // name. This ensures "my-service" finds files under
    // apps/my-service/ even when the code-content BM25 candidates are
    // dominated by generic single-token matches like "service". These feed
    // their own ranked list in fusion (see fuse_rrf) rather than being
    // injected into the lexical pool with a fake score.
    let mut path_chunks: Vec<(IndexedChunk, f32)> = Vec::new();
    let exact_path_pass = matches!(
        routing.intent,
        QueryIntent::ExactIdentifier | QueryIntent::Path
    ) || raw_query_terms(trimmed).len() <= 3;
    let path_query_variants = if exact_path_pass {
        lexical_queries.clone()
    } else {
        natural_language_path_recall_query(trimmed)
            .into_iter()
            .collect()
    };
    if !path_query_variants.is_empty()
        && let Some(fpt_field) = ctx.fields.file_path_text
    {
        let mut path_parser = QueryParser::for_index(&ctx.indexes[0], vec![fpt_field]);
        let path_candidate_limit = if exact_path_pass {
            100
        } else {
            NATURAL_LANGUAGE_PATH_FILE_LIMIT * NATURAL_LANGUAGE_PATH_DOCUMENT_OVERFETCH
        };
        if exact_path_pass {
            path_parser.set_conjunction_by_default();
        }
        let lexical_ids: HashSet<u64> = lexical_chunks.iter().map(|(c, _)| c.vector_key).collect();
        let mut path_by_id: HashMap<u64, (IndexedChunk, f32)> = HashMap::new();
        let mut path_by_file: HashMap<PathBuf, (IndexedChunk, f32)> = HashMap::new();
        for pq in &path_query_variants {
            let parsed = match path_parser.parse_query(pq) {
                Ok(parsed) => parsed,
                Err(err) => {
                    tracing::debug!(
                        query_variant = pq,
                        error = %err,
                        "skipping path recall variant rejected by Tantivy parser"
                    );
                    continue;
                }
            };
            let parsed =
                constrain_query_to_scope(parsed, &ctx.fields, options.scope_filter.as_ref())?;
            let parsed = constrain_query_to_glob_paths(parsed, &ctx.fields, &glob_path_filter);
            for (i, searcher) in ctx.searchers.iter().enumerate() {
                let docs = searcher.search(
                    &parsed,
                    &TopDocs::with_limit(path_candidate_limit).order_by_score(),
                )?;
                for (score, addr) in docs {
                    let doc = searcher.doc::<TantivyDocument>(addr)?;
                    if let Some(chunk) = fetch_chunk_by_id(doc, &ctx.fields)
                        .filter(|c| !ctx.is_shadowed_base_file(i, &c.file_path))
                        .filter(|c| type_matches(c, options.type_filter.as_deref()))
                        .filter(|c| scope_matches(c, options.scope_filter.as_ref()))
                        .filter(|c| path_matches(c, &path_matcher))
                        .filter(|c| options.skip_gitignore || !c.is_ignored)
                    {
                        if exact_path_pass {
                            if !lexical_ids.contains(&chunk.vector_key) {
                                path_by_id
                                    .entry(chunk.vector_key)
                                    .and_modify(|(_, best)| *best = best.max(score))
                                    .or_insert((chunk, score));
                            }
                        } else {
                            path_by_file
                                .entry(chunk.file_path.clone())
                                .and_modify(|(_, best)| *best = best.max(score))
                                .or_insert((chunk, score));
                        }
                    }
                }
            }
        }

        if exact_path_pass {
            path_chunks = path_by_id.into_values().collect();
        } else {
            path_chunks = path_by_file
                .into_iter()
                .map(|(file_path, (fallback, score))| {
                    lexical_chunks
                        .iter()
                        .find(|(chunk, _)| chunk.file_path == file_path)
                        .map(|(chunk, _)| (chunk.clone(), score))
                        .unwrap_or((fallback, score))
                })
                .collect();
        }
        path_chunks.sort_by(|left, right| {
            right
                .1
                .total_cmp(&left.1)
                .then_with(|| left.0.vector_key.cmp(&right.0.vector_key))
        });
        if !exact_path_pass {
            path_chunks.truncate(NATURAL_LANGUAGE_PATH_FILE_LIMIT);
        }
    }
    tracing::trace!(
        "lexical_path={:?} candidates={}",
        t0.elapsed(),
        path_chunks.len()
    );
    tracing::trace!("lexical={:?} found={}", t0.elapsed(), lexical_chunks.len());

    tracing::trace!("open_vector={:?}", t0.elapsed());

    if options.is_cancelled() {
        return Ok(Vec::new());
    }

    let mut semantic_chunks = Vec::new();
    let mut neural_executed = false;
    let has_hash_vectors = ctx.hash_vectors.as_ref().map_or(0, |v| v.size()) > 0
        || ctx.base_hash_vectors.as_ref().map_or(0, |v| v.size()) > 0;
    let has_neural_vectors = ctx.neural_vectors.as_ref().map_or(0, |v| v.size()) > 0
        || ctx.base_neural_vectors.as_ref().map_or(0, |v| v.size()) > 0;

    // Treat neural retrieval as fallback for weak or ambiguous lexical evidence.
    // When it runs, retain hash candidates because the tiers have complementary recall.
    let neural_profile_matches = embedding_model.is_none_or(|model| {
        let Some(active_identity) = model.model_identity() else {
            let Some(active_profile) = model.profile_info() else {
                return true;
            };
            return ctx
                .neural_profile
                .as_deref()
                .or(ctx.base_neural_profile.as_deref())
                .unwrap_or("general")
                == active_profile;
        };
        let Some(persisted_identity) = ctx.neural_model.as_ref().or(ctx.base_neural_model.as_ref())
        else {
            // Identity-less neural vectors predate complete model metadata and
            // must not be queried with a potentially incompatible revision.
            return false;
        };
        persisted_identity == active_identity
    });
    let execute_neural = neural_fallback_needed(
        routing,
        options.force_neural,
        lexical_chunks.first().map(|(_, score)| *score),
        lexical_chunks.get(1).map(|(_, score)| *score),
    );
    let neural_available = execute_neural
        && embedding_model.is_some_and(|model| model.model_identity().is_some())
        && has_neural_vectors
        && neural_profile_matches;
    let hash_vector_count = ctx
        .hash_vectors
        .as_ref()
        .map_or(0, VectorStore::size)
        .saturating_add(ctx.base_hash_vectors.as_ref().map_or(0, VectorStore::size));
    let neural_vector_count = ctx
        .neural_vectors
        .as_ref()
        .map_or(0, VectorStore::size)
        .saturating_add(
            ctx.base_neural_vectors
                .as_ref()
                .map_or(0, VectorStore::size),
        );
    let hash_weight =
        semantic_hash_weight(neural_available, neural_vector_count, hash_vector_count);
    let neural_model = embedding_model.filter(|model| {
        execute_neural
            && model.model_identity().is_some()
            && has_neural_vectors
            && neural_profile_matches
    });
    let direct_ids = lexical_chunks
        .iter()
        .map(|(chunk, _)| chunk.vector_key)
        .chain(literal_chunks.iter().map(|(chunk, _)| chunk.vector_key))
        .chain(path_chunks.iter().map(|(chunk, _)| chunk.vector_key))
        .chain(symbol_chunks.iter().map(|(chunk, _)| chunk.vector_key))
        .collect::<HashSet<_>>();

    if embedding_model.is_some() && (has_hash_vectors || neural_model.is_some()) {
        let semantic_started = std::time::Instant::now();
        let mut semantic_by_id = SemanticCandidatesById::new();
        let semantic_filters_active = has_semantic_filters(options);

        if !semantic_filters_active {
            let mut sources = Vec::with_capacity(2);
            let neural_matches = if let Some(model) = neural_model {
                neural_executed = true;
                let neural_query_vector =
                    neural_query_vector(model, query_text, &mut neural_query_vector_job);
                tracing::trace!("semantic_neural_embed={:?}", semantic_started.elapsed());
                let matches = collect_semantic_vector_matches(
                    &neural_query_vector,
                    semantic_limit,
                    ctx.neural_vectors.as_ref(),
                    ctx.base_neural_vectors.as_ref(),
                );
                tracing::trace!("semantic_neural_ann={:?}", semantic_started.elapsed());
                Some(matches)
            } else {
                None
            };

            if has_hash_vectors {
                let hash_query_vector = embed_hash_query(trimmed);
                tracing::trace!("semantic_hash_embed={:?}", semantic_started.elapsed());
                sources.push((
                    collect_semantic_vector_matches(
                        &hash_query_vector,
                        semantic_limit,
                        ctx.hash_vectors.as_ref(),
                        ctx.base_hash_vectors.as_ref(),
                    ),
                    hash_weight,
                    "hash",
                ));
                tracing::trace!("semantic_hash_ann={:?}", semantic_started.elapsed());
            }
            if let Some(neural_matches) = neural_matches {
                sources.push((neural_matches, 1.08, "neural"));
            }
            semantic_by_id = collect_unfiltered_semantic_candidates(ctx, options, sources)?;
            tracing::trace!("semantic_hydrate={:?}", semantic_started.elapsed());
        } else {
            let filter_plan = build_semantic_filter_plan(ctx, &path_matcher, options)?;
            if has_hash_vectors {
                let hash_query_vector = embed_hash_query(trimmed);
                tracing::trace!("semantic_hash_embed={:?}", semantic_started.elapsed());
                let hash_hits = collect_semantic_candidates(
                    ctx,
                    &path_matcher,
                    options,
                    &hash_query_vector,
                    semantic_limit,
                    (ctx.hash_vectors.as_ref(), ctx.base_hash_vectors.as_ref()),
                    Some(&filter_plan),
                )?;
                merge_semantic_candidates(&mut semantic_by_id, hash_hits, hash_weight, "hash");
            }

            if let Some(model) = neural_model {
                neural_executed = true;
                let neural_query_vector =
                    neural_query_vector(model, query_text, &mut neural_query_vector_job);
                let neural_hits = collect_semantic_candidates(
                    ctx,
                    &path_matcher,
                    options,
                    &neural_query_vector,
                    semantic_limit,
                    (
                        ctx.neural_vectors.as_ref(),
                        ctx.base_neural_vectors.as_ref(),
                    ),
                    Some(&filter_plan),
                )?;
                merge_semantic_candidates(&mut semantic_by_id, neural_hits, 1.08, "neural");
            }
        }

        semantic_chunks = semantic_by_id.into_values().collect::<Vec<_>>();
        semantic_chunks.sort_by(|left, right| {
            right
                .1
                .total_cmp(&left.1)
                .then_with(|| left.0.vector_key.cmp(&right.0.vector_key))
        });
    }
    tracing::trace!(
        "semantic={:?} found={}",
        t0.elapsed(),
        semantic_chunks.len()
    );

    if options.is_cancelled() {
        return Ok(Vec::new());
    }

    let fusion_query = FusionQuery::new(query_text);
    let merged = fuse_rrf_with_context(
        Some(ctx),
        FusionCandidates {
            lexical: lexical_chunks,
            semantic: semantic_chunks,
            literal: literal_chunks,
            path: path_chunks,
            path_weight: if exact_path_pass { 1.5 } else { 3.0 },
            symbols: symbol_chunks,
        },
        Some(direct_ids),
        if neural_available || query_targets_secondary_sources(query_text) {
            1.0
        } else {
            0.25
        },
        &fusion_query,
        routing,
        bounded_limit,
    )?;
    tracing::trace!("fuse_rrf={:?} merged={}", t0.elapsed(), merged.len());

    let presentation_query = PresentationQuery::from_fusion(&fusion_query);

    // Group hits by file path so we read and index each source file only once.
    let merged_len = merged.len();
    let mut hits_by_file: HashMap<PathBuf, Vec<(IndexedChunk, f32, Vec<String>)>> = HashMap::new();
    for (chunk, score, sources) in merged {
        hits_by_file
            .entry(workspace.root.join(&chunk.file_path))
            .or_default()
            .push((chunk, score, sources));
    }

    let file_count = hits_by_file.len();
    let mut hits = Vec::with_capacity(merged_len);
    for (file_path, file_hits) in hits_by_file {
        let file_content = ctx.read_file_content(&file_path);
        for (chunk, score, sources) in file_hits {
            hits.push(to_hit(
                workspace,
                chunk,
                score,
                sources,
                file_content.as_ref(),
                HitPresentation {
                    context_lines: options.bounded_context(),
                    query: &presentation_query,
                    routing,
                    neural_executed,
                },
            )?);
        }
    }
    // Re-sort since grouping by file changed the order
    hits.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.file_path.cmp(&right.file_path))
            .then_with(|| left.start_line.cmp(&right.start_line))
            .then_with(|| left.end_line.cmp(&right.end_line))
    });
    if matches!(routing.intent, QueryIntent::LiteralOrError) {
        let accepted_len = hits
            .iter()
            .position(|hit| hit.sources.iter().any(|source| source == "backfill"))
            .unwrap_or(hits.len());
        crate::reranker::rerank_hits(query_text, &mut hits[..accepted_len]);
    }
    tracing::trace!(
        "to_hit={:?} hits={} files_read={}",
        t0.elapsed(),
        hits.len(),
        file_count
    );

    Ok(hits)
}
