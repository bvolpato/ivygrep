use std::path::Path;

use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};

#[derive(Debug, Clone, Default)]
pub struct PathGlobMatcher {
    include: Option<GlobSet>,
    exclude: Option<GlobSet>,
}

impl PathGlobMatcher {
    pub fn new(include_globs: &[String], exclude_globs: &[String]) -> Result<Self> {
        let include = build_glob_set(include_globs, "include")?;
        let exclude = build_glob_set(exclude_globs, "exclude")?;
        Ok(Self { include, exclude })
    }

    pub fn matches(&self, rel_path: &Path) -> bool {
        let include_ok = self
            .include
            .as_ref()
            .is_none_or(|glob_set| glob_set.is_match(rel_path));
        let exclude_hit = self
            .exclude
            .as_ref()
            .is_some_and(|glob_set| glob_set.is_match(rel_path));
        include_ok && !exclude_hit
    }
}

pub fn parse_glob_csv(raw: Option<&str>) -> Vec<String> {
    raw.unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn build_glob_set(globs: &[String], label: &str) -> Result<Option<GlobSet>> {
    if globs.is_empty() {
        return Ok(None);
    }

    let mut builder = GlobSetBuilder::new();
    for pattern in globs {
        let glob = Glob::new(pattern)
            .with_context(|| format!("invalid {label} glob pattern: {pattern}"))?;
        builder.add(glob);
    }
    Ok(Some(builder.build().with_context(|| {
        format!("failed building {label} glob matcher")
    })?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_matcher_matches_everything() {
        let m = PathGlobMatcher::new(&[], &[]).unwrap();
        assert!(m.matches(Path::new("src/main.rs")));
        assert!(m.matches(Path::new("README.md")));
    }

    #[test]
    fn include_only_filters_to_pattern() {
        let m = PathGlobMatcher::new(&["*.rs".to_string()], &[]).unwrap();
        assert!(m.matches(Path::new("src/main.rs")));
        assert!(!m.matches(Path::new("README.md")));
    }

    #[test]
    fn exclude_only_rejects_pattern() {
        let m = PathGlobMatcher::new(&[], &["*.md".to_string()]).unwrap();
        assert!(m.matches(Path::new("src/main.rs")));
        assert!(!m.matches(Path::new("README.md")));
    }

    #[test]
    fn include_and_exclude_combined() {
        let m = PathGlobMatcher::new(&["*.rs".to_string()], &["*test*".to_string()]).unwrap();
        assert!(m.matches(Path::new("src/main.rs")));
        assert!(!m.matches(Path::new("src/test_helper.rs")));
        assert!(!m.matches(Path::new("README.md")));
    }

    #[test]
    fn exclude_takes_precedence_over_include() {
        let m = PathGlobMatcher::new(&["*.rs".to_string()], &["*.rs".to_string()]).unwrap();
        // Excluded even though included
        assert!(!m.matches(Path::new("lib.rs")));
    }

    #[test]
    fn parse_glob_csv_splits_and_trims() {
        assert_eq!(parse_glob_csv(Some("*.rs, *.py")), vec!["*.rs", "*.py"]);
    }

    #[test]
    fn parse_glob_csv_handles_none() {
        assert!(parse_glob_csv(None).is_empty());
    }

    #[test]
    fn parse_glob_csv_skips_empty_segments() {
        assert_eq!(parse_glob_csv(Some(",*.rs,,*.py,")), vec!["*.rs", "*.py"]);
    }

    #[test]
    fn invalid_glob_returns_error() {
        let result = PathGlobMatcher::new(&["[invalid".to_string()], &[]);
        assert!(result.is_err());
    }
}
