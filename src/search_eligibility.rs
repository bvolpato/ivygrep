use super::*;

type SharedPaths = Arc<HashSet<String>>;

/// Hard eligibility belongs before a bounded candidate heap, not after it.
/// Share large path/key sets so Tantivy's owned segment collectors do not copy
/// them for every segment or expanded query.
#[derive(Clone)]
pub(super) struct CandidateEligibility {
    skip_gitignore: bool,
    type_filter: Option<String>,
    scope: Option<WorkspaceScope>,
    paths: Option<PathGlobMatcher>,
    hidden: Option<(SharedPaths, SharedPaths)>,
    allowed_keys: Option<Arc<HashSet<u64>>>,
    cancel_token: Option<Arc<std::sync::atomic::AtomicBool>>,
}

impl Default for CandidateEligibility {
    fn default() -> Self {
        Self {
            skip_gitignore: true,
            type_filter: None,
            scope: None,
            paths: None,
            hidden: None,
            allowed_keys: None,
            cancel_token: None,
        }
    }
}

impl CandidateEligibility {
    pub(super) fn definition_candidates(
        &self,
        conn: &Connection,
        names: &[String],
        limit: usize,
        excluded: &HashSet<u64>,
    ) -> Result<Vec<IndexedChunk>> {
        let needs_predicate = self.type_filter.is_some()
            || self.scope.is_some()
            || self.paths.is_some()
            || self.hidden.is_some()
            || self.allowed_keys.is_some()
            || !excluded.is_empty();
        let include = |chunk: &IndexedChunk| {
            self.matches_chunk(chunk) && !excluded.contains(&chunk.vector_key)
        };
        crate::symbols::definition_candidates_eligible(
            conn,
            names,
            limit,
            self.skip_gitignore,
            needs_predicate.then_some(&include as &dyn Fn(&IndexedChunk) -> bool),
            self.cancel_token.as_deref(),
        )
    }

    pub(super) fn new(
        ctx: &SearchContext,
        searcher: usize,
        options: &SearchOptions,
        paths: &PathGlobMatcher,
        allowed_keys: Option<&Arc<HashSet<u64>>>,
    ) -> Self {
        Self {
            skip_gitignore: options.skip_gitignore,
            type_filter: options.canonical_type_filter(),
            scope: options.scope_filter.clone(),
            paths: (!options.include_globs.is_empty() || !options.exclude_globs.is_empty())
                .then(|| paths.clone()),
            hidden: (searcher == 1
                && (!ctx.tombstones.is_empty() || !ctx.overlay_files.is_empty()))
            .then(|| (Arc::clone(&ctx.tombstones), Arc::clone(&ctx.overlay_files))),
            allowed_keys: allowed_keys.cloned(),
            cancel_token: options.cancel_token.clone(),
        }
    }

    pub(super) fn unrestricted(&self, fields: &TantivyFields) -> bool {
        (self.skip_gitignore || fields.is_ignored.is_none())
            && self.type_filter.is_none()
            && self.scope.is_none()
            && self.paths.is_none()
            && self.hidden.is_none()
            && self.allowed_keys.is_none()
    }

    fn matches_metadata(&self, path: &Path, language: &str, ignored: bool, key: u64) -> bool {
        (self.skip_gitignore || !ignored)
            && self
                .type_filter
                .as_deref()
                .is_none_or(|expected| language.eq_ignore_ascii_case(expected))
            && self.scope.as_ref().is_none_or(|scope| scope.matches(path))
            && self
                .paths
                .as_ref()
                .is_none_or(|matcher| matcher.matches(path))
            && self.hidden.as_ref().is_none_or(|(tombstones, overlay)| {
                let path = index_path_string(path);
                !tombstones.contains(&path) && !overlay.contains(&path)
            })
            && self
                .allowed_keys
                .as_ref()
                .is_none_or(|keys| keys.contains(&key))
    }

    pub(super) fn matches_chunk(&self, chunk: &IndexedChunk) -> bool {
        self.matches_metadata(
            &chunk.file_path,
            &chunk.language,
            chunk.is_ignored,
            chunk.vector_key,
        )
    }

    pub(super) fn matches_document(
        &self,
        document: &TantivyDocument,
        fields: &TantivyFields,
    ) -> bool {
        use tantivy::schema::Value;
        if self.unrestricted(fields) {
            return true;
        }
        let Some(path) = document
            .get_first(fields.file_path)
            .and_then(|value| value.as_str())
        else {
            return false;
        };
        let Some(language) = document
            .get_first(fields.language)
            .and_then(|value| value.as_str())
        else {
            return false;
        };
        let Some(key) = document
            .get_first(fields.vector_key)
            .and_then(|value| value.as_u64())
        else {
            return false;
        };
        let ignored = fields
            .is_ignored
            .and_then(|field| document.get_first(field))
            .and_then(|value| value.as_u64())
            .is_some_and(|value| value != 0);
        self.matches_metadata(Path::new(path), language, ignored, key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tantivy::schema::Value;

    #[test]
    fn cancellable_topdocs_preserves_eligible_scores_ties_and_refills_rejections() {
        let root = tempfile::tempdir().unwrap();
        let (index, fields) = open_tantivy_index(root.path()).unwrap();
        let mut writer = index
            .writer_with_num_threads::<TantivyDocument>(1, 15_000_000)
            .unwrap();
        writer.set_merge_policy(Box::new(tantivy::merge_policy::NoMergePolicy));
        for segment in 0..3 {
            for ordinal in 0..30 {
                let ignored = ordinal % 3 == 0;
                let text = if ignored {
                    "needle needle needle"
                } else {
                    "needle eligible and some longer content"
                };
                writer
                    .add_document(tantivy::doc!(
                        fields.file_path => format!("src/{segment}_{ordinal}.rs"),
                        fields.language => "Rust",
                        fields.vector_key => (segment * 30 + ordinal) as u64,
                        fields.is_ignored.unwrap() => u64::from(ignored),
                        fields.text => text
                    ))
                    .unwrap();
            }
            writer.commit().unwrap();
        }
        writer.delete_term(tantivy::Term::from_field_text(
            fields.file_path,
            "src/0_1.rs",
        ));
        writer.commit().unwrap();
        let reader = index.reader().unwrap();
        let searcher = reader.searcher();
        let eligibility = CandidateEligibility {
            skip_gitignore: false,
            ..Default::default()
        };
        let filter = GlobPathQueryFilter::default();
        let cancelled = Arc::new(AtomicBool::new(false));
        for term in ["eligible", "needle"] {
            let query = TermQuery::new(
                tantivy::Term::from_field_text(fields.text, term),
                IndexRecordOption::WithFreqs,
            );
            let ranked = searcher
                .search(&query, &TopDocs::with_limit(100).order_by_score())
                .unwrap();
            let ignored = |address| {
                searcher
                    .doc::<TantivyDocument>(address)
                    .unwrap()
                    .get_first(fields.is_ignored.unwrap())
                    .unwrap()
                    .as_u64()
                    == Some(1)
            };
            assert_eq!(
                ranked.iter().take(7).any(|(_, address)| ignored(*address)),
                term == "needle"
            );
            let eligible = ranked
                .into_iter()
                .filter(|(_, address)| !ignored(*address))
                .collect::<Vec<_>>();
            for limit in [1, 7, 100] {
                let expected = eligible.iter().copied().take(limit).collect::<Vec<_>>();
                let actual = collect_top_docs_with_eligibility(
                    &searcher,
                    &query,
                    &fields,
                    &filter,
                    eligibility.clone(),
                    limit,
                    Some(&cancelled),
                )
                .unwrap();
                assert_eq!(actual, expected, "{term}, limit={limit}");
            }
            cancelled.store(true, Ordering::Relaxed);
            assert!(
                collect_top_docs_with_eligibility(
                    &searcher,
                    &query,
                    &fields,
                    &filter,
                    eligibility.clone(),
                    7,
                    Some(&cancelled)
                )
                .unwrap()
                .is_empty()
            );
            cancelled.store(false, Ordering::Relaxed);
        }
    }
}
