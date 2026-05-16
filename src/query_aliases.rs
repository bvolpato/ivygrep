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
        if entry
            .terms
            .iter()
            .all(|term| tokens.iter().any(|token| token == term))
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
        assert_eq!(token_aliases("scoring"), &["score", "rank"]);
        assert_eq!(token_aliases("flags"), &["cli", "arg", "option"]);
        assert!(token_aliases("unknown").is_empty());
    }

    #[test]
    fn phrase_aliases_load_from_generated_table() {
        let tokens = vec!["command".to_string(), "line".to_string()];
        assert_eq!(phrase_aliases(&tokens), vec!["cli"]);
    }
}
