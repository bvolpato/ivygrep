#[derive(Debug, Clone, Copy)]
pub(crate) struct PhraseAlias {
    pub(crate) terms: &'static [&'static str],
    pub(crate) aliases: &'static [&'static str],
}

include!(concat!(env!("OUT_DIR"), "/query_aliases.rs"));

pub(crate) fn token_aliases(token: &str) -> &'static [&'static str] {
    TOKEN_ALIASES
        .binary_search_by_key(&token, |(key, _)| *key)
        .map(|idx| TOKEN_ALIASES[idx].1)
        .unwrap_or(&[])
}

pub(crate) fn phrase_aliases(tokens: &[String]) -> Vec<&'static str> {
    let mut aliases = Vec::new();

    for entry in PHRASE_ALIASES {
        let terms_len = entry.terms.len();
        if terms_len > 0
            && tokens.windows(terms_len).any(|window| {
                window
                    .iter()
                    .zip(entry.terms.iter())
                    .all(|(token, term)| token == term)
            })
        {
            aliases.extend_from_slice(entry.aliases);
        }
    }

    aliases
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_aliases_are_sorted_for_binary_search() {
        for pair in TOKEN_ALIASES.windows(2) {
            assert!(pair[0].0 < pair[1].0);
        }
    }

    #[test]
    fn token_aliases_load_from_generated_table() {
        assert_eq!(token_aliases("choose"), &["pick", "select"]);
        assert_eq!(token_aliases("scoring"), &["score", "rank"]);
        assert!(token_aliases("flags").is_empty());
        assert!(token_aliases("output").is_empty());
        assert!(token_aliases("walker").is_empty());
        assert!(token_aliases("unknown").is_empty());
    }

    #[test]
    fn phrase_aliases_load_from_generated_table() {
        let tokens = vec!["command".to_string(), "line".to_string()];
        assert_eq!(phrase_aliases(&tokens), vec!["cli"]);

        let tokens = vec!["work".to_string(), "item".to_string()];
        assert_eq!(
            phrase_aliases(&tokens),
            vec!["job", "queue", "worker", "workqueue"]
        );
    }

    #[test]
    fn phrase_aliases_non_contiguous() {
        let tokens = vec!["command".to_string(), "run".to_string(), "line".to_string()];
        assert!(phrase_aliases(&tokens).is_empty());
    }
}
