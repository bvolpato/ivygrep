use pluralizer::pluralize;

/// Splits an identifier like `calculateTaxTotal` or `snake_case_name` into
/// lowercase segments: `["calculate", "tax", "total"]`.
pub fn split_identifier_segments(token: &str) -> Vec<String> {
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

/// Returns the singular form of a token, or the token unchanged if it's too
/// short or non-alphabetic.
pub fn singularize_token(token: &str) -> String {
    if token.len() <= 3 || !token.chars().all(|ch| ch.is_ascii_alphabetic()) {
        return token.to_string();
    }

    let singular = pluralize(token, 1isize, false).to_ascii_lowercase();
    if singular.is_empty() {
        token.to_string()
    } else {
        singular
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camel_case_split() {
        assert_eq!(
            split_identifier_segments("calculateTaxTotal"),
            vec!["calculate", "tax", "total"]
        );
    }

    #[test]
    fn snake_case_split() {
        assert_eq!(
            split_identifier_segments("apply_tax_rate"),
            vec!["apply", "tax", "rate"]
        );
    }

    #[test]
    fn upper_camel_case_split() {
        assert_eq!(
            split_identifier_segments("HttpResponseCode"),
            vec!["http", "response", "code"]
        );
    }

    #[test]
    fn single_word() {
        assert_eq!(split_identifier_segments("filter"), vec!["filter"]);
    }

    #[test]
    fn singularize_basic() {
        assert_eq!(singularize_token("taxes"), "tax");
        assert_eq!(singularize_token("limits"), "limit");
    }

    #[test]
    fn singularize_short_passthrough() {
        assert_eq!(singularize_token("id"), "id");
        assert_eq!(singularize_token("a"), "a");
    }

    #[test]
    fn singularize_non_alpha_passthrough() {
        assert_eq!(singularize_token("test123"), "test123");
    }
}
