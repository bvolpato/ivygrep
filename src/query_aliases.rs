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
        if phrase_matches(entry, tokens) {
            aliases.extend_from_slice(entry.aliases);
        }
    }

    aliases
}

pub(crate) fn literal_phrase_aliases(tokens: &[String]) -> Vec<&'static str> {
    let mut aliases = Vec::new();

    for entry in PHRASE_ALIASES {
        if phrase_matches(entry, tokens) {
            aliases.extend(entry.aliases.iter().copied().filter(|alias| {
                alias.len() >= 5
                    || alias.contains('_')
                    || (entry.terms.len() >= 3
                        && alias.len() == 3
                        && alias.bytes().all(|byte| byte.is_ascii_alphanumeric()))
            }));
        }
    }

    aliases
}

fn phrase_matches(entry: &PhraseAlias, tokens: &[String]) -> bool {
    let terms_len = entry.terms.len();
    terms_len > 0
        && tokens.windows(terms_len).any(|window| {
            window
                .iter()
                .zip(entry.terms.iter())
                .all(|(token, term)| token == term)
        })
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
        assert_eq!(token_aliases("composition"), &["compose"]);
        assert_eq!(token_aliases("resolution"), &["resolve", "resolver"]);
        assert_eq!(token_aliases("storage"), &["store"]);
        assert_eq!(token_aliases("embedding"), &["vector"]);
        assert_eq!(token_aliases("limit"), &["bound", "cap"]);
        assert_eq!(token_aliases("similarity"), &["ann", "nearest"]);
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

        let tokens = vec!["binary".to_string(), "part".to_string()];
        assert_eq!(phrase_aliases(&tokens), vec!["multipart"]);

        let tokens = vec!["error".to_string(), "formatting".to_string()];
        assert_eq!(phrase_aliases(&tokens), vec!["formatter"]);

        let tokens = vec!["form".to_string(), "data".to_string()];
        assert_eq!(phrase_aliases(&tokens), vec!["multipart"]);

        let tokens = vec!["communication".to_string(), "channel".to_string()];
        assert_eq!(
            phrase_aliases(&tokens),
            vec!["ipc", "socket", "protocol", "lock"]
        );

        let tokens = vec!["minified".to_string(), "bundle".to_string()];
        assert_eq!(
            phrase_aliases(&tokens),
            vec!["blob", "chunking", "minified"]
        );

        let tokens = vec!["search".to_string(), "run".to_string()];
        assert_eq!(
            phrase_aliases(&tokens),
            vec!["cpu_permits", "semaphore", "permit"]
        );

        let tokens = vec![
            "server".to_string(),
            "sent".to_string(),
            "event".to_string(),
        ];
        assert_eq!(phrase_aliases(&tokens), vec!["sse", "server_sent_events"]);
    }

    #[test]
    fn phrase_aliases_non_contiguous() {
        let tokens = vec!["command".to_string(), "run".to_string(), "line".to_string()];
        assert!(phrase_aliases(&tokens).is_empty());
    }

    #[test]
    fn literal_phrase_aliases_allow_precise_acronyms_only() {
        let sse = vec![
            "server".to_string(),
            "sent".to_string(),
            "event".to_string(),
        ];
        assert_eq!(
            literal_phrase_aliases(&sse),
            vec!["sse", "server_sent_events"]
        );

        let packet_receive = vec!["packet".to_string(), "receive".to_string()];
        assert_eq!(literal_phrase_aliases(&packet_receive), vec!["ingress"]);
    }
}
