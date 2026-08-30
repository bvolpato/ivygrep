use super::*;
use tantivy::query_grammar::{UserInputAst, UserInputLeaf};

pub(super) type LexicalDocuments = Vec<(usize, f32, TantivyDocument)>;

pub(super) struct BooleanCandidates {
    pub documents: LexicalDocuments,
    pub keys: Arc<HashSet<u64>>,
}

/// Keep identifier-like input and quoted/escaped operator words on the ordinary
/// expansion path. The parser, not this recognizer, validates Boolean grammar.
pub(super) fn has_explicit_boolean_operators(query: &str) -> bool {
    let query = query.trim();
    if !query.chars().any(char::is_whitespace)
        && !query.starts_with('(')
        && !query.starts_with("NOT(")
    {
        return false;
    }
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in query.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if let Some(delimiter) = quote {
            if character == delimiter {
                quote = None;
            }
            continue;
        }
        let previous = query[..index].chars().next_back();
        let boundary =
            previous.is_none_or(|value| value.is_whitespace() || value == '(' || value == ')');
        if character == '"' || (character == '\'' && boundary) {
            quote = Some(character);
            continue;
        }
        if boundary
            && ["AND", "OR", "NOT"].iter().any(|operator| {
                query[index..].strip_prefix(*operator).is_some_and(|tail| {
                    tail.chars()
                        .next()
                        .is_none_or(|value| value.is_whitespace() || value == '(' || value == ')')
                })
            })
        {
            return true;
        }
    }
    false
}

pub(super) fn lexical_query_parser(ctx: &SearchContext, conjunction: bool) -> QueryParser {
    // Raw STRING paths remain available through explicit file_path: clauses,
    // not defaults, so they cannot hide unsupported phrase queries.
    let mut fields = vec![ctx.fields.text];
    fields.extend(ctx.fields.file_path_text);
    fields.extend(ctx.fields.signature);
    let mut parser = QueryParser::for_index(&ctx.indexes[0], fields);
    parser.set_field_boost(ctx.fields.file_path, 2.0);
    if let Some(field) = ctx.fields.file_path_text {
        parser.set_field_boost(field, 5.0);
    }
    if let Some(field) = ctx.fields.signature {
        parser.set_field_boost(field, 5.0);
    }
    if conjunction {
        parser.set_conjunction_by_default();
    }
    parser
}

fn anchor_negative_clauses(ast: &mut UserInputAst) {
    match ast {
        UserInputAst::Clause(clauses) => {
            for (_, child) in clauses.iter_mut() {
                anchor_negative_clauses(child);
            }
            if !clauses.is_empty()
                && clauses
                    .iter()
                    .all(|(occur, _)| *occur == Some(Occur::MustNot))
            {
                // The pinned parser leaves nested NOT clauses purely negative,
                // which Tantivy otherwise evaluates as empty. A zero-scoring
                // universe gives NOT its set-complement meaning without adding
                // any relevance contribution to positive terms.
                clauses.push((
                    Some(Occur::Must),
                    UserInputAst::Boost(Box::new(UserInputLeaf::All.into()), 0.0.into()),
                ));
            }
        }
        UserInputAst::Boost(child, _) => anchor_negative_clauses(child),
        UserInputAst::Leaf(_) => {}
    }
}

/// A structured request has one bounded pool satisfying the original parsed
/// query. Other signals may rank that pool, but may not admit new keys through
/// alias expansion, literal/path recall, symbols, or vector similarity.
pub(super) fn boolean_candidates(
    ctx: &SearchContext,
    text: &str,
    options: &SearchOptions,
    paths: &PathGlobMatcher,
    glob_filter: &GlobPathQueryFilter,
    limit: usize,
) -> Result<Option<BooleanCandidates>> {
    if !has_explicit_boolean_operators(text) {
        return Ok(None);
    }
    let parser = lexical_query_parser(ctx, should_use_conjunctive_numeric_query(text));
    let mut ast = tantivy::query_grammar::parse_query(text)
        .map_err(|_| anyhow::anyhow!("invalid or unsupported Boolean query: {text}"))?;
    anchor_negative_clauses(&mut ast);
    let query = parser
        .build_query_from_user_input_ast(ast)
        .with_context(|| format!("invalid or unsupported Boolean query: {text}"))?;
    let query = constrain_query_to_scope(query, &ctx.fields, options.scope_filter.as_ref())?;
    let query = constrain_query_to_glob_paths(query, &ctx.fields, glob_filter);
    let mut documents = Vec::new();
    let mut keys = HashSet::new();
    for (index, searcher) in ctx.searchers.iter().enumerate() {
        let eligibility = CandidateEligibility::new(ctx, index, options, paths, None);
        for (score, address) in collect_top_docs_with_eligibility(
            searcher,
            query.as_ref(),
            &ctx.fields,
            glob_filter,
            eligibility,
            limit,
            options.cancel_token.as_ref(),
        )? {
            let document = searcher.doc::<TantivyDocument>(address)?;
            if let Some(chunk) = fetch_chunk_by_id(document.clone(), &ctx.fields) {
                keys.insert(chunk.vector_key);
                documents.push((index, score, document));
            }
        }
    }
    Ok(Some(BooleanCandidates {
        documents,
        keys: Arc::new(keys),
    }))
}
