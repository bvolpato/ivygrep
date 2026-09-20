use super::*;
use tantivy::query_grammar::{UserInputAst, UserInputLeaf};

pub(super) type LexicalDocuments = Vec<(usize, f32, TantivyDocument)>;

pub(super) struct BooleanCandidates {
    pub documents: LexicalDocuments,
    pub keys: Arc<HashSet<u64>>,
}

/// How the explicit Boolean operators of a request apply to its search.
pub(super) enum BooleanSearch {
    /// No explicit operators: the ordinary expansion path.
    Absent,
    /// A Boolean expression: every retrieval signal stays inside this pool.
    Pool(BooleanCandidates),
    /// Operator words in a prompt or paste that is not a Boolean expression,
    /// such as issue text with pasted SQL or an emphasized `NOT`. The ordinary
    /// path reads them as text, and the results carry
    /// [`BOOLEAN_NOT_APPLIED_WARNING`].
    NotApplied,
}

/// Returned with the results of a search that read operator words as text (see
/// [`BooleanSearch::NotApplied`]).
pub(crate) const BOOLEAN_NOT_APPLIED_WARNING: &str = "\
    Boolean operators not applied: this multi-line or long query is not a valid Boolean \
    expression, so uppercase AND, OR, and NOT were searched as ordinary words. For a Boolean \
    search, send a short one-line expression such as `alpha AND NOT beta`.";

/// Keep identifier-like input and quoted/escaped/code-span words on the ordinary
/// expansion path. An unfinished quote must not hide a Boolean operator from
/// the strict parser, which validates the remaining grammar.
pub(super) fn has_explicit_boolean_operators(query: &str) -> bool {
    let query = query.trim();
    let syntax_prefix = query
        .strip_prefix('"')
        .or_else(|| query.strip_prefix('\''))
        .unwrap_or(query);
    if !query.chars().any(char::is_whitespace)
        && !syntax_prefix.starts_with('(')
        && !syntax_prefix.starts_with("NOT(")
    {
        return false;
    }
    let mut quote = None;
    let mut escaped = false;
    let mut quoted_operator = false;
    let mut backticks = None;
    let mut skip_until = 0;
    let operator_at = |index: usize, boundary: bool| {
        boundary
            && ["AND", "OR", "NOT"].iter().any(|operator| {
                query[index..].strip_prefix(*operator).is_some_and(|tail| {
                    tail.chars()
                        .next()
                        .is_none_or(|value| value.is_whitespace() || value == '(' || value == ')')
                })
            })
    };
    for (index, character) in query.char_indices() {
        if index < skip_until {
            continue;
        }
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        let previous = query[..index].chars().next_back();
        let boundary =
            previous.is_none_or(|value| value.is_whitespace() || value == '(' || value == ')');
        if let Some((delimiter, start)) = quote {
            if character == delimiter {
                quote = None;
                quoted_operator = false;
            } else if operator_at(index, boundary || index == start + 1) {
                quoted_operator = true;
            }
            continue;
        }
        if character == '`' {
            let width = query[index..]
                .bytes()
                .take_while(|byte| *byte == b'`')
                .count();
            // Never pair a delimiter run with one of its own characters.
            // Only closed spans are opaque; an unmatched run leaves later
            // operators and differently sized spans available to the scanner.
            skip_until = index + width;
            let runs = backticks.get_or_insert_with(|| backtick_runs(query));
            if let Some(starts) = runs.get_mut(&width) {
                while starts.front().is_some_and(|start| *start <= index) {
                    starts.pop_front();
                }
                if let Some(closing) = starts.front() {
                    // Backslashes and quote characters inside code are literal.
                    skip_until = closing + width;
                }
            }
            continue;
        }
        let quote_boundary = boundary || matches!(previous, Some(':' | '+' | '-'));
        if character == '"' || (character == '\'' && quote_boundary) {
            quote = Some((character, index));
            quoted_operator = false;
            continue;
        }
        if operator_at(index, boundary) {
            return true;
        }
    }
    quote.is_some() && quoted_operator
}

fn backtick_runs(query: &str) -> HashMap<usize, VecDeque<usize>> {
    let mut runs: HashMap<usize, VecDeque<usize>> = HashMap::new();
    let mut ticks = query.match_indices('`').peekable();
    while let Some((start, _)) = ticks.next() {
        let mut end = start + 1;
        while ticks.next_if(|(position, _)| *position == end).is_some() {
            end += 1;
        }
        runs.entry(end - start).or_default().push_back(start);
    }
    runs
}

/// Pasted source, with or without Boolean operators, leaves `signature` out of
/// the default fields, and multi-line prose scores it like body text (see
/// `SignatureScoring`). An explicit `signature:` term clause scores 5x on any
/// query shape: the field boost covers one-line queries and pasted source, and
/// `parse_lexical_query` and `boolean_candidates` boost the clause for prose.
pub(super) fn lexical_query_parser(
    ctx: &SearchContext,
    signature_scoring: SignatureScoring,
    conjunction: bool,
) -> QueryParser {
    let fields = &ctx.fields;
    // Raw STRING paths remain available through explicit file_path: clauses,
    // not defaults, so they cannot hide unsupported phrase queries.
    let mut default_fields = vec![fields.text];
    default_fields.extend(fields.file_path_text);
    if signature_scoring != SignatureScoring::Omitted {
        default_fields.extend(fields.signature);
    }
    let mut parser = QueryParser::for_index(&ctx.indexes[0], default_fields);
    parser.set_field_boost(fields.file_path, 2.0);
    if let Some(field) = fields.file_path_text {
        parser.set_field_boost(field, 5.0);
    }
    if let Some(field) = fields.signature {
        parser.set_field_boost(field, signature_scoring.default_boost());
    }
    if conjunction {
        parser.set_conjunction_by_default();
    }
    parser
}

/// Parses text with a parser from `lexical_query_parser`. For multi-line prose,
/// explicit `signature:` clauses get back the boost the field gives up.
pub(super) fn parse_lexical_query(
    parser: &QueryParser,
    text: &str,
    signature_scoring: SignatureScoring,
) -> std::result::Result<Box<dyn Query>, tantivy::query::QueryParserError> {
    if signature_scoring != SignatureScoring::Plain {
        return guard_query_grammar(text, || parser.parse_query(text));
    }
    let mut ast = parse_user_input(text)?;
    boost_explicit_signature_clauses(&mut ast);
    parser.build_query_from_user_input_ast(ast)
}

/// Whether Tantivy's grammar would panic on `text` instead of rejecting it. A
/// standalone `-` or `+` followed by a standalone `*` makes it build an exists
/// clause without a field ("Exist query without a field isn't allowed"). A
/// Markdown list in a pasted question (`-` on one line, `* item` on the next)
/// is enough.
pub(super) fn has_sign_before_bare_star(text: &str) -> bool {
    let boundary = |character: Option<char>| {
        character.is_none_or(|value| value.is_whitespace() || matches!(value, '(' | ')'))
    };
    let mut previous = None;
    for (index, character) in text.char_indices() {
        if matches!(character, '-' | '+') && boundary(previous) {
            // Only whitespace may sit between the sign and the star: `+(*)` is
            // a valid clause that the grammar parses.
            let rest = &text[index + character.len_utf8()..];
            let after_space = rest.trim_start();
            if after_space.len() < rest.len()
                && let Some(tail) = after_space.strip_prefix('*')
                && boundary(tail.chars().next())
            {
                return true;
            }
        }
        previous = Some(character);
    }
    false
}

/// Runs a call into Tantivy's query grammar and reports a panic inside it as a
/// syntax error, which callers already handle like any other text the grammar
/// rejects. The known trigger is answered without calling the grammar, so it
/// prints no panic message; the unwind guard covers inputs not found yet.
fn guard_query_grammar<T>(
    text: &str,
    parse: impl FnOnce() -> std::result::Result<T, tantivy::query::QueryParserError>,
) -> std::result::Result<T, tantivy::query::QueryParserError> {
    let syntax_error = || tantivy::query::QueryParserError::SyntaxError(text.to_string());
    if has_sign_before_bare_star(text) {
        return Err(syntax_error());
    }
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(parse))
        .unwrap_or_else(|_| Err(syntax_error()))
}

fn parse_user_input(
    text: &str,
) -> std::result::Result<UserInputAst, tantivy::query::QueryParserError> {
    guard_query_grammar(text, || {
        tantivy::query_grammar::parse_query(text)
            .map_err(|_| tantivy::query::QueryParserError::SyntaxError(text.to_string()))
    })
}

/// Wraps explicit `signature:` term clauses in a boost that restores 5x when the
/// parser scores default signature matches at 1x. Tantivy applies field boosts
/// to term clauses only, so range, set, regex, and exists clauses keep the same
/// unboosted score on every query shape.
fn boost_explicit_signature_clauses(ast: &mut UserInputAst) {
    match ast {
        UserInputAst::Clause(clauses) => {
            for (_, child) in clauses.iter_mut() {
                boost_explicit_signature_clauses(child);
            }
        }
        UserInputAst::Boost(child, _) => boost_explicit_signature_clauses(child),
        UserInputAst::Leaf(leaf) => {
            let explicit_signature_term = matches!(
                leaf.as_ref(),
                UserInputLeaf::Literal(literal) if literal.field_name.as_deref() == Some("signature")
            );
            if explicit_signature_term {
                let clause = std::mem::replace(ast, UserInputAst::Clause(Vec::new()));
                let restored = SignatureScoring::BOOST / SignatureScoring::Plain.default_boost();
                *ast = UserInputAst::Boost(Box::new(clause), f64::from(restored).into());
            }
        }
    }
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

fn boolean_query_error(text: &str) -> String {
    format!(
        "invalid or unsupported Boolean query: {text}\n\
         Hint: to search code or prose containing AND, OR, or NOT, wrap that text \
         in matching backticks or a fenced code block."
    )
}

/// The strict parse of a request with explicit operators. It fails on syntax
/// the grammar rejects and on clauses the index cannot serve, such as phrases.
fn strict_boolean_query(
    ctx: &SearchContext,
    text: &str,
    signature_scoring: SignatureScoring,
) -> Result<Box<dyn Query>> {
    let parser = lexical_query_parser(
        ctx,
        signature_scoring,
        should_use_conjunctive_numeric_query(text),
    );
    let mut ast = parse_user_input(text).map_err(|_| anyhow::anyhow!(boolean_query_error(text)))?;
    anchor_negative_clauses(&mut ast);
    if signature_scoring == SignatureScoring::Plain {
        boost_explicit_signature_clauses(&mut ast);
    }
    parser
        .build_query_from_user_input_ast(ast)
        .with_context(|| boolean_query_error(text))
}

/// A structured request has one bounded pool satisfying the original parsed
/// query. Other signals may rank that pool, but may not admit new keys through
/// alias expansion, literal/path recall, symbols, or vector similarity.
///
/// A one-line lookup that does not parse fails, and the error teaches the
/// syntax. A prompt or paste (see `is_prompt_shaped`) that does not parse is
/// not a Boolean request: its uppercase `AND`, `OR` or `NOT` is emphasis or
/// pasted SQL, so it takes the ordinary path with a warning instead of failing.
/// Input that parses is a Boolean request on any shape.
pub(super) fn boolean_candidates(
    ctx: &SearchContext,
    text: &str,
    signature_scoring: SignatureScoring,
    options: &SearchOptions,
    paths: &PathGlobMatcher,
    glob_filter: &GlobPathQueryFilter,
    limit: usize,
) -> Result<BooleanSearch> {
    if !has_explicit_boolean_operators(text) {
        return Ok(BooleanSearch::Absent);
    }
    let query = match strict_boolean_query(ctx, text, signature_scoring) {
        Ok(query) => query,
        Err(_) if is_prompt_shaped(text) => return Ok(BooleanSearch::NotApplied),
        Err(error) => return Err(error),
    };
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
    Ok(BooleanSearch::Pool(BooleanCandidates {
        documents,
        keys: Arc::new(keys),
    }))
}
