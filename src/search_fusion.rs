use std::collections::{HashMap, HashSet};

use anyhow::Result;

use crate::indexer::IndexedChunk;
use crate::search_routing::{QueryIntent, QueryRouting};

use super::{
    ChunkBoostContext, FILE_COHERENCE_WEIGHT, FusionCandidates, FusionQuery,
    REPRESENTATIVE_SPAN_MIN_COVERAGE, RankedCandidate, SOURCE_EXACT_SYMBOL, SearchContext,
    SourceMask, SymbolCandidateKind, alias_file_stem_multiplier, apply_file_coherence_boost,
    backfill_enabled, chunk_density_exponent, chunk_kind_boost, definition_name_boost,
    effective_authority_score_with_intent, file_stem_signals, filter_meaningful_scores_with_query,
    is_definition_kind, is_precise_lookup_query_with_tokens, literal_match_boost_with_query,
    location_intent_boost, normalize_lexical_score, normalize_semantic_score,
    path_exact_match_boost_with_query, path_key, path_segment_boost, primary_file_stem_multiplier,
    promote_qualified_symbol_span, promote_representative_span, rerank_candidate_limit_for_routing,
    should_run_literal_pass, source_bit, term_coverage_boost,
};

pub(super) fn fuse_rrf_with_context(
    ctx: Option<&SearchContext>,
    candidates: FusionCandidates,
    direct_ids: Option<HashSet<u64>>,
    semantic_direct_weight: f32,
    query: &FusionQuery<'_>,
    routing: QueryRouting,
    limit: Option<usize>,
) -> Result<Vec<(IndexedChunk, f32, Vec<String>)>> {
    let fuse_started = std::time::Instant::now();
    const K: f32 = 60.0;
    const SEMANTIC_WEIGHT: f32 = 1.0;
    const LITERAL_WEIGHT: f32 = 4.0;
    // Path matches (file_path contains the query) are a useful but bounded
    // signal. They get a moderate rank-based weight — enough to surface a
    // file whose path matches the query when content candidates are weak,
    // without overriding strong content matches. The path-aware boosts below
    // (path_exact_match/path_segment/file_stem) still apply on top.
    const SYMBOL_WEIGHT: f32 = 10.0;
    const LEXICAL_SCORE_WEIGHT: f32 = 0.05;
    const SEMANTIC_SCORE_WEIGHT: f32 = 0.08;
    const SEMANTIC_ONLY_PENALTY: f32 = 0.60;
    const TERM_COVERAGE_WEIGHT: f32 = 0.35;
    const PATH_SEGMENT_WEIGHT: f32 = 0.40;
    const FILE_STEM_WEIGHT: f32 = 0.50;
    const DEFINITION_NAME_BONUS: f32 = 0.25;
    const LOCATION_INTENT_WEIGHT: f32 = 0.20;
    // Path-exact matches now also feed their own ranked RRF list (see the
    // `path` pass above), so this additive boost no longer needs to be large
    // enough to single-handedly win — it was 3.0, ~60x the base RRF score.
    const PATH_EXACT_MATCH_WEIGHT: f32 = 0.8;
    const FILE_COVERAGE_WEIGHT: f32 = 3.0;
    const EXACT_LITERAL_MULTIPLIER: f32 = 1.8;
    const ALIAS_LITERAL_MULTIPLIER: f32 = 1.35;
    // Bound the total additive boost relative to the fused base score so
    // boosts perturb the RRF ranking rather than replace it.
    const MAX_BOOST_RATIO: f32 = 3.0;
    const MAX_BOOST_FLOOR: f32 = 0.25;

    let FusionCandidates {
        lexical,
        semantic,
        literal,
        path,
        path_weight,
        symbols,
    } = candidates;

    const LEXICAL_WEIGHT: f32 = 3.2;
    let query_tokens = query.tokens.as_slice();
    let location_intent = query.location_intent;
    let secondary_intent = query.secondary_intent;
    let direct_ids = direct_ids.unwrap_or_else(|| {
        lexical
            .iter()
            .map(|(chunk, _)| chunk.vector_key)
            .chain(literal.iter().map(|(chunk, _)| chunk.vector_key))
            .chain(path.iter().map(|(chunk, _)| chunk.vector_key))
            .chain(symbols.iter().map(|(chunk, _)| chunk.vector_key))
            .collect()
    });

    struct RrfEntry {
        score: f32,
        chunk: IndexedChunk,
        sources: SourceMask,
    }

    let mut entries: HashMap<u64, RrfEntry> = HashMap::new();
    let mut add_entry = |chunk: IndexedChunk, score: f32, sources: SourceMask| {
        let vector_key = chunk.vector_key;
        let entry = entries.entry(vector_key).or_insert_with(|| RrfEntry {
            score: 0.0,
            chunk,
            sources: 0,
        });
        entry.score += score;
        entry.sources |= sources;
    };

    for (rank, (chunk, lexical_score)) in lexical.into_iter().enumerate() {
        add_entry(
            chunk,
            LEXICAL_WEIGHT / (K + rank as f32 + 1.0)
                + normalize_lexical_score(lexical_score) * LEXICAL_SCORE_WEIGHT,
            source_bit("lexical"),
        );
    }

    for (rank, (chunk, semantic_score, semantic_sources)) in semantic.into_iter().enumerate() {
        // Hash vectors are a cheap provisional recall tier. Keep full strength
        // for semantic-only discovery, but do not let hash collisions overrule
        // direct evidence. Neural vectors use semantic_direct_weight=1.0.
        let direct_weight = if direct_ids.contains(&chunk.vector_key) {
            semantic_direct_weight
        } else {
            1.0
        };
        let semantic_source_mask = semantic_sources
            .into_iter()
            .fold(source_bit("semantic"), |mask, source| {
                mask | source_bit(source)
            });
        add_entry(
            chunk,
            direct_weight * SEMANTIC_WEIGHT / (K + rank as f32 + 1.0)
                + direct_weight * normalize_semantic_score(semantic_score) * SEMANTIC_SCORE_WEIGHT,
            semantic_source_mask,
        );
    }

    // Literal pass: verified exact substring matches get a strong boost
    for (rank, (chunk, _)) in literal.into_iter().enumerate() {
        add_entry(
            chunk,
            LITERAL_WEIGHT / (K + rank as f32 + 1.0),
            source_bit("literal"),
        );
    }

    // Path pass: chunks whose file path matches the query, ranked by their
    // path-field BM25 score. Rank-based only — no raw-score magnitude term —
    // so a path match can't dominate via an out-of-scale score.
    for (rank, (chunk, _)) in path.into_iter().enumerate() {
        add_entry(
            chunk,
            path_weight / (K + rank as f32 + 1.0),
            source_bit("path"),
        );
    }

    for (rank, (chunk, kind)) in symbols.into_iter().enumerate() {
        let weight = match kind {
            SymbolCandidateKind::Exact => SYMBOL_WEIGHT * 20.0,
            SymbolCandidateKind::Inferred => SYMBOL_WEIGHT * 5.0,
            SymbolCandidateKind::Alias => SYMBOL_WEIGHT,
        };
        add_entry(
            chunk,
            weight / (K + rank as f32 + 1.0),
            source_bit(match kind {
                SymbolCandidateKind::Exact => "exact-symbol",
                SymbolCandidateKind::Inferred => "inferred-symbol",
                SymbolCandidateKind::Alias => "symbol",
            }),
        );
    }

    tracing::trace!(
        "fuse_collect={:?} entries={}",
        fuse_started.elapsed(),
        entries.len()
    );

    let rerank_limit = rerank_candidate_limit_for_routing(routing);
    let mut rerank_order = entries
        .iter()
        .map(|(vector_key, entry)| (*vector_key, entry.score))
        .collect::<Vec<_>>();
    rerank_order.sort_by(|left, right| {
        right
            .1
            .total_cmp(&left.1)
            .then_with(|| left.0.cmp(&right.0))
    });
    let rerank_ids = rerank_order
        .into_iter()
        .take(rerank_limit)
        .map(|(chunk_id, _)| chunk_id)
        .collect::<HashSet<_>>();

    let hydrate_started = std::time::Instant::now();
    let mut empty_text_candidates = 0;
    let mut hydrated_candidates = 0;
    if let Some(ctx) = ctx {
        let empty_text_keys = rerank_ids
            .iter()
            .filter_map(|vector_key| entries.get(vector_key))
            .filter(|entry| entry.chunk.text.is_empty())
            .map(|entry| entry.chunk.vector_key)
            .collect::<Vec<_>>();
        empty_text_candidates = empty_text_keys.len();
        if !empty_text_keys.is_empty() {
            let batch = ctx.fetch_chunk_texts_by_vector_keys_batch(&empty_text_keys)?;
            for vector_key in &rerank_ids {
                if let Some(entry) = entries.get_mut(vector_key)
                    && entry.chunk.text.is_empty()
                    && let Some(text) = batch.get(&entry.chunk.vector_key)
                {
                    entry.chunk.text.clone_from(text);
                    hydrated_candidates += 1;
                }
            }
        }
    }
    tracing::trace!(
        "fuse_hydrate={:?} hydrate_io={:?} rerank_candidates={} empty_text_candidates={} hydrated_candidates={}",
        fuse_started.elapsed(),
        hydrate_started.elapsed(),
        rerank_ids.len(),
        empty_text_candidates,
        hydrated_candidates
    );

    let primary_query_tokens = query.primary_tokens.as_slice();
    let mut file_query_matches: HashMap<u64, HashSet<usize>> = HashMap::new();
    let mut boost_contexts = HashMap::with_capacity(entries.len());
    for (vector_key, entry) in &entries {
        if !rerank_ids.contains(vector_key) {
            continue;
        }
        let bctx = ChunkBoostContext::new_with_compact(&entry.chunk, query.compact_candidate_text);
        if primary_query_tokens.len() >= 3 {
            let matches = file_query_matches
                .entry(path_key(&entry.chunk.file_path))
                .or_default();
            for (idx, token) in primary_query_tokens.iter().enumerate() {
                if bctx.text_lower.contains(token.as_str())
                    || bctx.path_lower.contains(token.as_str())
                {
                    matches.insert(idx);
                }
            }
        }
        boost_contexts.insert(*vector_key, bctx);
    }

    // Count how many candidate chunks each file contributes. Secondary-source
    // files with many chunks get a density penalty so they cannot dominate by
    // contributing more candidates; primary implementation files are exempt.
    let mut file_chunk_counts: HashMap<u64, usize> = HashMap::new();
    for e in entries.values() {
        *file_chunk_counts
            .entry(path_key(&e.chunk.file_path))
            .or_insert(0) += 1;
    }
    tracing::trace!("fuse_context={:?}", fuse_started.elapsed());

    let mut ranked = entries
        .into_values()
        .map(|e| {
            let RrfEntry {
                score: base_score,
                chunk,
                sources: source_set,
            } = e;
            if !rerank_ids.contains(&chunk.vector_key) {
                return RankedCandidate {
                    chunk,
                    score: base_score,
                    sources: source_set,
                };
            }

            // Precompute lowercased text/path once per candidate instead of
            // redundantly in every boost function.
            let bctx = boost_contexts
                .remove(&chunk.vector_key)
                .unwrap_or_else(|| ChunkBoostContext::new(&chunk));

            // Accumulate signal boosts separately from the RRF base so they can
            // be bounded. Previously these were added directly and several were
            // 10-60x the base RRF score (~0.05), so a single boost could
            // override the fused rank signal entirely.
            let mut additive_boost = literal_match_boost_with_query(query, &bctx);

            let coverage = if !query_tokens.is_empty() {
                term_coverage_boost(query_tokens, &bctx)
            } else {
                0.0
            };
            additive_boost += coverage * TERM_COVERAGE_WEIGHT;

            if !query_tokens.is_empty() {
                additive_boost += path_segment_boost(query_tokens, &bctx) * PATH_SEGMENT_WEIGHT;
            }

            additive_boost +=
                path_exact_match_boost_with_query(query, &bctx) * PATH_EXACT_MATCH_WEIGHT;

            let (file_stem_score, primary_file_stem_derivation) = file_stem_signals(
                query_tokens,
                &query.token_compacts,
                primary_query_tokens,
                &query.primary_token_compacts,
                &bctx,
            );
            additive_boost += file_stem_score * FILE_STEM_WEIGHT;

            if !query_tokens.is_empty() {
                additive_boost +=
                    definition_name_boost(query_tokens, &bctx) * DEFINITION_NAME_BONUS;
            }

            if location_intent {
                additive_boost += location_intent_boost(&chunk, &bctx) * LOCATION_INTENT_WEIGHT;
            }

            // Keep RRF as the primary ranking signal: cap the total additive
            // boost so it perturbs the fused base score rather than dominating
            // it. The cap scales with the base (with a small floor so even
            // weak-base candidates get a meaningful, bounded lift).
            let boost_cap = (base_score * MAX_BOOST_RATIO).max(MAX_BOOST_FLOOR);
            let mut score = base_score + additive_boost.min(boost_cap);

            if let Some(matches) = file_query_matches.get(&path_key(&chunk.file_path))
                && matches.len() >= 2
            {
                let file_coverage = matches.len() as f32 / primary_query_tokens.len() as f32;
                score *= 1.0 + file_coverage * file_coverage * FILE_COVERAGE_WEIGHT;
            }

            if source_set & source_bit("literal") != 0 {
                score *= if should_run_literal_pass(query.text) {
                    EXACT_LITERAL_MULTIPLIER
                } else {
                    ALIAS_LITERAL_MULTIPLIER
                };
            }

            if source_set
                & (source_bit("lexical")
                    | source_bit("literal")
                    | source_bit("path")
                    | source_bit("inferred-symbol"))
                == 0
            {
                score *= SEMANTIC_ONLY_PENALTY;
            }

            // Chunks with zero query term overlap despite having text are noise
            if !query_tokens.is_empty()
                && coverage < f32::EPSILON
                && source_set & (source_bit("literal") | source_bit("path")) == 0
            {
                score *= 0.5;
            }

            score *= chunk_kind_boost(&chunk);
            score *= effective_authority_score_with_intent(query_tokens, &bctx, secondary_intent);
            score *= primary_file_stem_multiplier(
                primary_query_tokens,
                primary_file_stem_derivation,
                &bctx,
            );
            score *= alias_file_stem_multiplier(&query.alias_token_compacts, &bctx);

            // Apply chunk-density normalization: 1/n^x where n is the number
            // of chunks this file has in the candidate set. Primary
            // implementation files use x=0 and are unaffected.
            let n_file_chunks = file_chunk_counts
                .get(&path_key(&chunk.file_path))
                .copied()
                .unwrap_or(1) as f32;
            score /= n_file_chunks.powf(chunk_density_exponent(&bctx));

            RankedCandidate {
                chunk,
                score,
                sources: source_set,
            }
        })
        .collect::<Vec<_>>();
    tracing::trace!("fuse_score={:?}", fuse_started.elapsed());

    if is_precise_lookup_query_with_tokens(query.text, &query.primary_tokens)
        && let Some(max_score) = ranked.iter().map(|item| item.score).reduce(f32::max)
    {
        let exact_depths = ranked
            .iter()
            .filter(|item| item.sources & SOURCE_EXACT_SYMBOL != 0)
            .map(|item| {
                (
                    item.chunk.vector_key,
                    crate::symbols::exact_name_namespace_depth(&item.chunk, query.text),
                )
            })
            .collect::<HashMap<_, _>>();
        if let Some(exact) = ranked
            .iter_mut()
            .filter(|item| item.sources & SOURCE_EXACT_SYMBOL != 0)
            .max_by(|left, right| {
                let left_depth = exact_depths
                    .get(&left.chunk.vector_key)
                    .copied()
                    .flatten()
                    .unwrap_or(usize::MAX);
                let right_depth = exact_depths
                    .get(&right.chunk.vector_key)
                    .copied()
                    .flatten()
                    .unwrap_or(usize::MAX);
                (left_depth != usize::MAX)
                    .cmp(&(right_depth != usize::MAX))
                    .then_with(|| right_depth.cmp(&left_depth))
                    .then_with(|| {
                        is_definition_kind(&left.chunk.kind)
                            .cmp(&is_definition_kind(&right.chunk.kind))
                    })
                    .then_with(|| left.score.total_cmp(&right.score))
                    .then_with(|| right.chunk.vector_key.cmp(&left.chunk.vector_key))
            })
        {
            exact.score = max_score + (max_score * 0.01).max(0.01);
        }
    }

    ranked.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.chunk.vector_key.cmp(&right.chunk.vector_key))
    });
    if query.primary_tokens.len() >= 4 {
        apply_file_coherence_boost(&mut ranked, FILE_COHERENCE_WEIGHT, query.secondary_intent);
        ranked.sort_by(|left, right| {
            right
                .score
                .total_cmp(&left.score)
                .then_with(|| left.chunk.vector_key.cmp(&right.chunk.vector_key))
        });
    }
    // A qualified member in prose is an explicit request for that definition.
    // Keep the winning file and score, but center its preview on the exact
    // member instead of a nearby helper with stronger prose overlap.
    promote_qualified_symbol_span(&mut ranked, query.text);
    // "How" queries are architecture-oriented; term-dense prose can be less
    // useful than the implementation span already selected by the ranker.
    if matches!(
        routing.intent,
        QueryIntent::NaturalLanguage | QueryIntent::DocsTestsExamples | QueryIntent::Mixed
    ) && !query.lower.starts_with("how ")
    {
        // File-level evidence determines rank. For descriptive queries, show
        // the strongest local evidence from the top file without changing its
        // score, source signals, or position.
        promote_representative_span(
            &mut ranked,
            &query.primary_tokens,
            REPRESENTATIVE_SPAN_MIN_COVERAGE,
        );
    }

    // Per-file hit diversity cap: keep the best chunk per file at full score,
    // then aggressively decay. This mirrors web-search result diversity: a
    // second snippet from the same file can still show up, but should not crowd
    // out another authoritative file.
    let mut file_hit_counts: HashMap<u64, usize> = HashMap::new();
    for item in &mut ranked {
        let count = file_hit_counts
            .entry(path_key(&item.chunk.file_path))
            .or_insert(0);
        *count += 1;
        match *count {
            1 => {}
            2 => item.score *= 0.35,
            3..=4 => item.score *= 0.15,
            _ => item.score *= 0.05,
        }
    }
    ranked.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.chunk.vector_key.cmp(&right.chunk.vector_key))
    });
    let enable_backfill = backfill_enabled(&ranked);

    if matches!(
        routing.intent,
        QueryIntent::NaturalLanguage | QueryIntent::DocsTestsExamples | QueryIntent::Mixed
    ) {
        let mut seen_files = HashSet::new();
        ranked.retain(|item| seen_files.insert(path_key(&item.chunk.file_path)));
    }

    let mut filtered = filter_meaningful_scores_with_query(ranked, query, enable_backfill);

    if let Some(limit) = limit {
        filtered.truncate(limit);
    }
    tracing::trace!(
        "fuse_filter={:?} results={}",
        fuse_started.elapsed(),
        filtered.len()
    );

    Ok(filtered
        .into_iter()
        .map(RankedCandidate::into_tuple)
        .collect())
}
