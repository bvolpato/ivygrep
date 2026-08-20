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
        self.is_included(rel_path) && !self.is_excluded(rel_path)
    }

    pub fn is_included(&self, rel_path: &Path) -> bool {
        self.include
            .as_ref()
            .is_none_or(|glob_set| glob_set.is_match(rel_path))
    }

    pub fn is_excluded(&self, rel_path: &Path) -> bool {
        self.exclude
            .as_ref()
            .is_some_and(|glob_set| glob_set.is_match(rel_path))
    }
}

pub fn parse_glob_csv(raw: Option<&str>) -> Vec<String> {
    let mut patterns = Vec::new();
    let mut pattern = String::new();
    let mut brace_depth = 0usize;
    let mut in_character_class = false;
    let mut escaped = false;

    for character in raw.unwrap_or_default().chars() {
        if escaped {
            if character != ',' {
                pattern.push('\\');
            }
            pattern.push(character);
            escaped = false;
            continue;
        }

        match character {
            '\\' => escaped = true,
            '[' if !in_character_class => {
                in_character_class = true;
                pattern.push(character);
            }
            ']' if in_character_class => {
                in_character_class = false;
                pattern.push(character);
            }
            '{' if !in_character_class => {
                brace_depth = brace_depth.saturating_add(1);
                pattern.push(character);
            }
            '}' if !in_character_class => {
                brace_depth = brace_depth.saturating_sub(1);
                pattern.push(character);
            }
            ',' if brace_depth == 0 && !in_character_class => {
                let trimmed = pattern.trim();
                if !trimmed.is_empty() {
                    patterns.push(trimmed.to_string());
                }
                pattern.clear();
            }
            _ => pattern.push(character),
        }
    }

    if escaped {
        pattern.push('\\');
    }
    let trimmed = pattern.trim();
    if !trimmed.is_empty() {
        patterns.push(trimmed.to_string());
    }
    patterns
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
    fn parse_glob_csv_preserves_alternatives_classes_and_escaped_commas() {
        assert_eq!(
            parse_glob_csv(Some(
                r"*.{rs,md}, src/**, literal\,name.rs, nested/{a,b}/{x,y}.rs, file[,a].rs"
            )),
            vec![
                "*.{rs,md}",
                "src/**",
                "literal,name.rs",
                "nested/{a,b}/{x,y}.rs",
                "file[,a].rs",
            ]
        );
    }

    #[test]
    fn invalid_glob_returns_error() {
        let result = PathGlobMatcher::new(&["[invalid".to_string()], &[]);
        assert!(result.is_err());
    }
}
