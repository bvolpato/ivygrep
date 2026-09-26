use super::*;

const ELIGIBLE_KEY_BATCH: usize = 4_096;
const MAX_REFILL_ANN_KEYS: usize = 20_000;
const MAX_REFILL_ANN_ROUNDS: usize = 3;

#[derive(Default)]
struct RefillDiagnostics {
    started: Option<std::time::Instant>,
    ann_rounds: usize,
    ann_keys: usize,
    scanned_keys: usize,
    exact_scan: bool,
}

impl Drop for RefillDiagnostics {
    fn drop(&mut self) {
        tracing::debug!(
            target: "ivygrep::performance",
            stage = "semantic_refill",
            ann_rounds = self.ann_rounds,
            ann_keys = self.ann_keys,
            exact_scan = self.exact_scan,
            scanned_keys = self.scanned_keys,
            elapsed_ms = self.started.map_or(0.0, |started| started.elapsed().as_secs_f64() * 1000.0),
            "semantic eligibility recovery"
        );
    }
}

fn next_ann_limit(current: usize, eligible: usize, wanted: usize, cap: usize) -> usize {
    // Estimate survival from the last round, with 50% headroom.
    let estimated =
        wanted.saturating_mul(current).saturating_mul(3) / eligible.max(1).saturating_mul(2);
    current.saturating_mul(2).max(estimated).min(cap)
}

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

/// Retry ANN with a bounded pool before exact scoring. If eligible candidates
/// still underfill, stream SQLite keys in fixed-size batches and retain top-k.
pub(super) fn refill_semantic_matches(
    ctx: &SearchContext,
    paths: &PathGlobMatcher,
    options: &SearchOptions,
    query: &[f32],
    limit: usize,
    stores: (Option<&VectorStore>, Option<&VectorStore>),
    previous_ann: (usize, usize),
) -> Result<Vec<VectorMatch>> {
    if limit == 0 || options.is_cancelled() {
        return Ok(Vec::new());
    }
    let mut diagnostics = RefillDiagnostics {
        started: Some(std::time::Instant::now()),
        ..Default::default()
    };
    refill_semantic_matches_with_diagnostics(
        ctx,
        paths,
        options,
        query,
        limit,
        stores,
        previous_ann,
        &mut diagnostics,
    )
}

#[allow(clippy::too_many_arguments)]
fn refill_semantic_matches_with_diagnostics(
    ctx: &SearchContext,
    paths: &PathGlobMatcher,
    options: &SearchOptions,
    query: &[f32],
    limit: usize,
    stores: (Option<&VectorStore>, Option<&VectorStore>),
    previous_ann: (usize, usize),
    diagnostics: &mut RefillDiagnostics,
) -> Result<Vec<VectorMatch>> {
    let cap = limit
        .saturating_mul(16)
        .min(MAX_REFILL_ANN_KEYS)
        .max(limit)
        .max(previous_ann.0);
    let mut ann_limit = next_ann_limit(previous_ann.0, previous_ann.1, limit, cap);
    for _ in 0..MAX_REFILL_ANN_ROUNDS {
        if ann_limit <= previous_ann.0 {
            break;
        }
        if options.is_cancelled() {
            return Ok(Vec::new());
        }
        let matches = collect_semantic_vector_matches(query, ann_limit, stores.0, stores.1);
        let keys = matches.iter().map(|hit| hit.key).collect::<Vec<_>>();
        let chunks = ctx.fetch_chunk_metadata_by_vector_keys_batch(&keys)?;
        diagnostics.ann_rounds += 1;
        diagnostics.ann_keys += keys.len();
        let mut eligible = matches
            .into_iter()
            .filter(|hit| {
                chunks.get(&hit.key).is_some_and(|chunk| {
                    (options.skip_gitignore || !chunk.is_ignored)
                        && type_matches(chunk, options.type_filter.as_deref())
                        && scope_matches(chunk, options.scope_filter.as_ref())
                        && path_matches(chunk, paths)
                })
            })
            .take(limit)
            .collect::<Vec<_>>();
        if eligible.len() >= limit {
            eligible.truncate(limit);
            return Ok(eligible);
        }
        if ann_limit >= cap || keys.len() < ann_limit {
            break;
        }
        ann_limit = next_ann_limit(ann_limit, eligible.len(), limit, cap);
    }
    diagnostics.exact_scan = true;
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
                diagnostics.scanned_keys += 1;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedding::{EmbeddingModel, HashEmbeddingModel};
    use serial_test::serial;

    #[test]
    fn overfetch_uses_survival_rate_and_stays_bounded() {
        assert_eq!(next_ann_limit(100, 50, 100, 1600), 300);
        assert_eq!(next_ann_limit(100, 99, 100, 1600), 200);
        assert_eq!(next_ann_limit(100, 0, 100, 1600), 1600);
        assert_eq!(next_ann_limit(usize::MAX, 0, usize::MAX, 20_000), 20_000);
    }

    #[test]
    #[serial]
    fn overfetch_avoids_exact_scan_and_underfill_keeps_exact_recovery() {
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let root = tempfile::tempdir().unwrap();
        for index in 0..4 {
            std::fs::write(
                root.path().join(format!("visible-{index}.txt")),
                "visible source\n",
            )
            .unwrap();
        }
        let workspace = Workspace::resolve(root.path()).unwrap();
        let model = HashEmbeddingModel::new(crate::EMBEDDING_DIMENSIONS);
        crate::indexer::index_workspace(&workspace, &model).unwrap();
        let connection = crate::indexer::open_sqlite_readonly(&workspace.sqlite_path()).unwrap();
        let keys = connection
            .prepare("SELECT vector_key FROM chunks")
            .unwrap()
            .query_map([], |row| row.get::<_, i64>(0))
            .unwrap()
            .map(|key| key.unwrap() as u64)
            .collect::<HashSet<_>>();
        assert_eq!(keys.len(), 4);
        let query = model.embed("visible source");
        let zero = query.iter().position(|value| *value == 0.0).unwrap();
        let mut visible = query.clone();
        visible[zero] = 1.0;
        let mut vectors = VectorStore::open(
            &workspace.vector_path(),
            model.dimensions(),
            HASH_VECTOR_QUANTIZATION,
            crate::vector_store::VectorTier::Hash,
        )
        .unwrap();
        for key in &keys {
            vectors.upsert(*key, visible.clone()).unwrap();
        }
        for orphan in 1_000_000..1_000_008 {
            assert!(!keys.contains(&orphan));
            vectors.add_unchecked(orphan, query.clone()).unwrap();
        }
        assert!(
            vectors
                .search(&query, 4)
                .iter()
                .all(|hit| !keys.contains(&hit.key))
        );
        vectors.save().unwrap();
        drop(vectors);
        let context = SearchContext::load(&workspace, Some(model.dimensions()), false).unwrap();
        let options = SearchOptions::default();
        let paths = PathGlobMatcher::new(&[], &[]).unwrap();
        for (wanted, exact_scan) in [(4, false), (5, true)] {
            let mut diagnostics = RefillDiagnostics::default();
            let matches = refill_semantic_matches_with_diagnostics(
                &context,
                &paths,
                &options,
                &query,
                wanted,
                (context.hash_vectors.as_ref(), None),
                (4, 0),
                &mut diagnostics,
            )
            .unwrap();
            assert_eq!(
                matches.iter().map(|hit| hit.key).collect::<HashSet<_>>(),
                keys
            );
            assert_eq!(diagnostics.exact_scan, exact_scan);
            assert_eq!(diagnostics.scanned_keys, if exact_scan { 4 } else { 0 });
            assert_eq!(diagnostics.ann_rounds, 1);
        }
    }
}
