//! Keyword search via FTS5
//! Mirrors gbrain's src/core/search/keyword.ts

use tracing::trace;

/// Build an FTS5 match expression from a user query.
/// Escapes FTS5 special characters to prevent syntax injection.
/// Returns empty string for empty/whitespace-only queries (caller must handle).
pub fn build_fts_query(query: &str) -> String {
    trace!(query = %query, "Building FTS5 query");
    let terms: Vec<&str> = query.split_whitespace().filter(|t| !t.is_empty()).collect();

    if terms.is_empty() {
        return String::new();
    }

    if terms.len() > 1 {
        // Phrase search: strip internal quotes and sanitize FTS5-special characters
        // to prevent column filter injection (e.g., "title:secret" being interpreted
        // as a column-scoped search by FTS5).
        let clean_query: String = query
            .chars()
            .filter(|c| *c != '"')
            .map(|c| {
                if c.is_alphanumeric() || c == '_' || c == '\'' || c.is_whitespace() {
                    c
                } else {
                    ' '
                }
            })
            .collect();
        let phrase = format!("\"{}\"", clean_query);
        // Individual token search: escape each term, filter empty, add prefix wildcard.
        // Each term is double-quoted to prevent FTS5 operators (AND, OR, NOT, NEAR)
        // from being interpreted as query syntax rather than literal search terms.
        let individual: Vec<String> = terms
            .iter()
            .filter_map(|t| {
                let escaped = escape_fts_term(t);
                if escaped.is_empty() {
                    None
                } else {
                    Some(format!("\"{}\"*", escaped))
                }
            })
            .collect();
        if individual.is_empty() {
            // All terms were stripped to nothing — return phrase only
            return phrase;
        }
        format!("{} OR {}", phrase, individual.join(" AND "))
    } else {
        let escaped = escape_fts_term(terms[0]);
        if escaped.is_empty() {
            return String::new();
        }
        format!("\"{}\"*", escaped)
    }
}

/// Escape special FTS5 characters that could inject query syntax.
/// Removes: quotes, parens, braces, colon, caret, asterisk, dot, brackets.
/// FTS5 boolean operators (AND, OR, NOT, NEAR) are handled by
/// splitting on whitespace and joining with explicit operators,
/// plus double-quoting terms in build_fts_query.
pub fn escape_fts_term(term: &str) -> String {
    // Replace special chars with spaces (not strip) to preserve search semantics.
    // E.g. "C++" becomes "C  " → first word "C", "state-of-the-art" → "state of the art" → "state"
    term.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '\'' {
                c
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_end_matches('\'')
        .to_string()
}
