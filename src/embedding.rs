use std::collections::HashMap;

use sha2::{Digest, Sha256};

pub trait EmbeddingModel: Send + Sync {
    fn dimensions(&self) -> usize;
    fn embed(&self, text: &str) -> Vec<f32>;
}

#[derive(Debug, Clone)]
pub struct HashEmbeddingModel {
    dimensions: usize,
    normalization_aliases: HashMap<&'static str, &'static str>,
}

impl HashEmbeddingModel {
    pub fn new(dimensions: usize) -> Self {
        let normalization_aliases = HashMap::from([
            ("calc", "calculate"),
            ("taxes", "tax"),
            ("compute", "calculate"),
            ("sum", "total"),
            ("klass", "class"),
            ("func", "function"),
            ("limits", "limit"),
        ]);

        Self {
            dimensions,
            normalization_aliases,
        }
    }

    fn normalize_token<'a>(&'a self, token: &'a str) -> &'a str {
        self.normalization_aliases
            .get(token)
            .copied()
            .unwrap_or(token)
    }
}

impl EmbeddingModel for HashEmbeddingModel {
    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn embed(&self, text: &str) -> Vec<f32> {
        let mut vector = vec![0.0f32; self.dimensions];
        if text.is_empty() {
            return vector;
        }

        let mut token_count = 0usize;

        for raw_token in text
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|t| !t.is_empty())
        {
            for token in semantic_token_variants(raw_token) {
                let singular = singularize_token(&token);
                let normalized = self.normalize_token(&singular);
                token_count += 1;

                let mut hasher = Sha256::new();
                hasher.update(normalized.as_bytes());
                let hash = hasher.finalize();

                let bucket = u64::from_le_bytes(hash[..8].try_into().expect("slice length"))
                    as usize
                    % self.dimensions;
                let sign = if hash[8] & 1 == 0 { 1.0 } else { -1.0 };
                vector[bucket] += sign;
            }
        }

        if token_count == 0 {
            return vector;
        }

        let norm = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
        if norm > 0.0 {
            for value in &mut vector {
                *value /= norm;
            }
        }

        vector
    }
}

fn semantic_token_variants(raw_token: &str) -> Vec<String> {
    let compact = raw_token.to_ascii_lowercase();
    let segments = split_identifier_segments(raw_token);

    let mut out = Vec::with_capacity(segments.len().saturating_add(2));
    out.push(compact.clone());

    for segment in &segments {
        if segment != &compact {
            out.push(segment.clone());
        }
    }

    if segments.len() > 1 {
        let joined = segments.join("");
        if joined != compact {
            out.push(joined);
        }
    }

    out.sort();
    out.dedup();
    out
}

fn split_identifier_segments(token: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut prev_is_lower = false;
    let mut prev_is_alpha = false;

    for ch in token.chars() {
        if !ch.is_ascii_alphanumeric() {
            if !current.is_empty() {
                segments.push(current.to_ascii_lowercase());
                current.clear();
            }
            prev_is_lower = false;
            prev_is_alpha = false;
            continue;
        }

        let is_upper = ch.is_ascii_uppercase();
        let is_alpha = ch.is_ascii_alphabetic();

        if !current.is_empty() && is_upper && prev_is_lower {
            segments.push(current.to_ascii_lowercase());
            current.clear();
        }

        if !current.is_empty() && is_alpha != prev_is_alpha {
            segments.push(current.to_ascii_lowercase());
            current.clear();
        }

        current.push(ch);
        prev_is_lower = ch.is_ascii_lowercase();
        prev_is_alpha = is_alpha;
    }

    if !current.is_empty() {
        segments.push(current.to_ascii_lowercase());
    }

    segments
}

fn singularize_token(token: &str) -> String {
    let len = token.len();
    if len <= 3 {
        return token.to_string();
    }

    if token.ends_with("ies") && len > 4 {
        return format!("{}y", &token[..len - 3]);
    }

    if token.ends_with("ses")
        || token.ends_with("xes")
        || token.ends_with("zes")
        || token.ends_with("ches")
        || token.ends_with("shes")
    {
        return token[..len - 2].to_string();
    }

    if token.ends_with('s') && !token.ends_with("ss") {
        return token[..len - 1].to_string();
    }

    token.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_embeddings_are_stable() {
        let model = HashEmbeddingModel::new(64);
        let left = model.embed("calculate tax total");
        let right = model.embed("calculate tax total");
        assert_eq!(left, right);
    }

    #[test]
    fn alias_mapping_changes_similarity() {
        let model = HashEmbeddingModel::new(64);
        let v1 = model.embed("calc tax");
        let v2 = model.embed("calculate taxes");
        let cosine = v1.iter().zip(v2.iter()).map(|(a, b)| a * b).sum::<f32>();
        assert!(cosine > 0.2);
    }

    #[test]
    fn identifier_and_plural_forms_align() {
        let model = HashEmbeddingModel::new(128);
        let query = model.embed("apply limits");
        let code = model.embed("void applyLimit() {}");
        let cosine = query
            .iter()
            .zip(code.iter())
            .map(|(a, b)| a * b)
            .sum::<f32>();
        assert!(cosine > 0.15);
    }
}
