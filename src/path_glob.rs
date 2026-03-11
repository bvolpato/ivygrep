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
