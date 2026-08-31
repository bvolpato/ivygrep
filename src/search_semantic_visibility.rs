use super::*;

const ELIGIBLE_KEY_BATCH: usize = 4_096;

fn merge_top_matches(best: &mut Vec<VectorMatch>, more: Vec<VectorMatch>, limit: usize) {
    best.extend(more);
    best.sort_unstable_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then(left.key.cmp(&right.key))
    });
    let mut seen = HashSet::new();
    best.retain(|hit| seen.insert(hit.key));
    best.truncate(limit);
}

fn score_batch(
    keys: &[u64],
    query: &[f32],
    limit: usize,
    stores: (Option<&VectorStore>, Option<&VectorStore>),
    options: &SearchOptions,
    best: &mut Vec<VectorMatch>,
) -> Result<()> {
    for store in [stores.0, stores.1].into_iter().flatten() {
        if options.is_cancelled() {
            break;
        }
        merge_top_matches(
            best,
            store.score_many_top_k_checked(keys, query, limit, options.cancel_token.as_deref())?,
            limit,
        );
    }
    Ok(())
}

/// Underfill recovery. Keep the ordinary ANN/hydration path when every
/// returned key is eligible. Otherwise stream current SQLite keys and exact
/// score fixed-size batches, retaining only top-k. This can scan the eligible
/// corpus, but never expands an ANN result vector to corpus size.
pub(super) fn refill_semantic_matches(
    ctx: &SearchContext,
    paths: &PathGlobMatcher,
    options: &SearchOptions,
    query: &[f32],
    limit: usize,
    stores: (Option<&VectorStore>, Option<&VectorStore>),
) -> Result<Vec<VectorMatch>> {
    if limit == 0 || options.is_cancelled() {
        return Ok(Vec::new());
    }
    let mut best = Vec::new();
    let mut keys = Vec::with_capacity(ELIGIBLE_KEY_BATCH);
    let type_filter = options.canonical_type_filter();
    let filter = FilteredChunkQuery {
        path_matcher: paths,
        scope_filter: options.scope_filter.as_ref(),
        type_filter: type_filter.as_deref(),
        include_globs: &options.include_globs,
        skip_gitignore: options.skip_gitignore,
        max_results: usize::MAX,
    };
    for (index, conn) in std::iter::once(&ctx.sqlite)
        .chain(ctx.base_sqlite.as_ref())
        .enumerate()
    {
        if options.is_cancelled() {
            return Ok(Vec::new());
        }
        visit_filtered_chunks(
            conn,
            filter,
            |chunk| !ctx.is_shadowed_base_file(index, &chunk.file_path),
            options.cancel_token.as_deref(),
            |chunk| {
                if options.is_cancelled() {
                    return Ok(false);
                }
                keys.push(chunk.vector_key);
                if keys.len() == ELIGIBLE_KEY_BATCH {
                    score_batch(&keys, query, limit, stores, options, &mut best)?;
                    keys.clear();
                }
                Ok(!options.is_cancelled())
            },
        )?;
    }
    if options.is_cancelled() {
        return Ok(Vec::new());
    }
    score_batch(&keys, query, limit, stores, options, &mut best)?;
    if options.is_cancelled() {
        return Ok(Vec::new());
    }
    Ok(best)
}

pub(super) fn score_constrained_semantic_keys(
    ctx: &SearchContext,
    keys: &HashSet<u64>,
    query: &[f32],
    limit: usize,
    stores: (Option<&VectorStore>, Option<&VectorStore>),
    options: &SearchOptions,
) -> Result<Vec<(IndexedChunk, f32)>> {
    let keys = keys.iter().copied().collect::<Vec<_>>();
    let mut best = Vec::new();
    for batch in keys.chunks(ELIGIBLE_KEY_BATCH) {
        if options.is_cancelled() {
            return Ok(Vec::new());
        }
        score_batch(batch, query, limit, stores, options, &mut best)?;
    }
    if options.is_cancelled() {
        return Ok(Vec::new());
    }
    let selected = best.iter().map(|hit| hit.key).collect::<Vec<_>>();
    let chunks = ctx.fetch_chunks_by_vector_keys_batch(&selected)?;
    let paths = PathGlobMatcher::new(&options.include_globs, &options.exclude_globs)?;
    let eligibility = CandidateEligibility::new(ctx, 0, options, &paths, None);
    Ok(best
        .into_iter()
        .filter_map(|hit| {
            chunks
                .get(&hit.key)
                .filter(|chunk| eligibility.matches_chunk(chunk))
                .cloned()
                .map(|chunk| (chunk, hit.score))
        })
        .collect())
}
