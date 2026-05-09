//! Chinese NLP: jieba tokenization + pinyin for KB FTS5 indexing

use jieba_rs::Jieba;
use pinyin::ToPinyin;
use std::sync::OnceLock;

const MAX_CONTENT_TOKENS: usize = 10000;
const MAX_PINYIN_CHARS: usize = 200;

static JIEBA: OnceLock<Jieba> = OnceLock::new();

fn jieba() -> &'static Jieba {
    JIEBA.get_or_init(Jieba::new)
}

/// Tokenize document content for FTS5 indexing.
/// Returns space-separated tokens string for kb_document_nodes.content_tokens.
pub fn tokenize_content(content: &str) -> String {
    let words = jieba().cut(content, true);
    let mut token_set = std::collections::HashSet::new();
    let mut result = Vec::new();

    for word in words {
        let token = normalize_token(word);
        if token.is_empty() {
            continue;
        }
        if token_set.insert(token.clone()) {
            result.push(token.clone());
        }
        if has_chinese(&token) {
            if let Some(pinyin_tokens) = generate_pinyin_tokens(&token) {
                for pt in pinyin_tokens {
                    if token_set.insert(pt.clone()) {
                        result.push(pt);
                    }
                }
            }
        }
        if result.len() >= MAX_CONTENT_TOKENS {
            break;
        }
    }
    result.join(" ")
}

/// Tokenize file name for FTS5 indexing.
/// Returns space-separated tokens string for kb_documents.name_tokens.
pub fn tokenize_name(original_name: &str) -> String {
    let (stem, ext) = split_name_extension(original_name);
    let mut token_set = std::collections::HashSet::new();
    let mut result = Vec::new();

    // 1. jieba cut on stem
    let words = jieba().cut(&stem, true);
    for word in words {
        let token = normalize_token(word);
        if token.is_empty() {
            continue;
        }
        if token_set.insert(token.clone()) {
            result.push(token);
        }
    }

    // 2. pinyin for chinese text
    let chinese = extract_chinese(&stem);
    if !chinese.is_empty() && chinese.chars().count() <= MAX_PINYIN_CHARS {
        if let Some(pinyin_tokens) = generate_pinyin_tokens(&chinese) {
            for pt in pinyin_tokens {
                if token_set.insert(pt.clone()) {
                    result.push(pt);
                }
            }
        }
    }

    // 3. split by non-word chars and re-tokenize each part
    for part in split_by_non_word(&stem) {
        let part_words = jieba().cut(&part, true);
        for word in part_words {
            let token = normalize_token(word);
            if !token.is_empty() && token_set.insert(token.clone()) {
                result.push(token);
            }
        }
        if has_chinese(&part) && part.chars().count() <= MAX_PINYIN_CHARS {
            if let Some(pinyin_tokens) = generate_pinyin_tokens(&part) {
                for pt in pinyin_tokens {
                    if token_set.insert(pt.clone()) {
                        result.push(pt);
                    }
                }
            }
        }
    }

    // 4. add extension as token
    if !ext.is_empty() {
        let ext_token = ext.to_lowercase();
        if token_set.insert(ext_token.clone()) {
            result.push(ext_token);
        }
    }

    result.join(" ")
}

/// Build FTS5 MATCH query from user search keywords.
pub fn build_fts_match_query(keyword: &str) -> String {
    let words = jieba().cut(keyword, true);
    let mut parts = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for word in words {
        let token = normalize_token(word);
        if token.is_empty() {
            continue;
        }
        if seen.insert(token.clone()) {
            let escaped = escape_fts5_token(&token);
            parts.push(format!("{}*", escaped));
        }
    }

    if parts.is_empty() {
        for part in split_by_non_word(keyword) {
            let token = normalize_token(&part);
            if !token.is_empty() && seen.insert(token.clone()) {
                let escaped = escape_fts5_token(&token);
                parts.push(format!("{}*", escaped));
            }
        }
    }

    parts.join(" OR ")
}

/// Check if a character is a CJK unified ideograph.
pub fn is_chinese(c: char) -> bool {
    matches!(c,
        '\u{4E00}'..='\u{9FFF}' |
        '\u{3400}'..='\u{4DBF}' |
        '\u{20000}'..='\u{2A6DF}' |
        '\u{2A700}'..='\u{2B73F}' |
        '\u{2B740}'..='\u{2B81F}' |
        '\u{F900}'..='\u{FAFF}'
    )
}

/// Check if a string contains any Chinese characters.
pub fn has_chinese(text: &str) -> bool {
    text.chars().any(|c| is_chinese(c))
}

/// Extract only Chinese characters from a string.
pub fn extract_chinese(text: &str) -> String {
    text.chars().filter(|c| is_chinese(*c)).collect()
}

/// Normalize a token: trim, lowercase, and reject tokens without alphanumeric or Chinese content.
pub fn normalize_token(token: &str) -> String {
    let t = token.trim().to_lowercase();
    if t.is_empty() || !t.chars().any(|c| c.is_alphanumeric() || is_chinese(c)) {
        return String::new();
    }
    t
}

/// Split text by non-word characters (preserving Chinese characters as word characters).
pub fn split_by_non_word(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric() && !is_chinese(c))
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Escape a token for safe use in FTS5 MATCH queries by removing special characters.
pub fn escape_fts5_token(token: &str) -> String {
    token
        .chars()
        .filter(|c| !matches!(c, '"' | '\'' | '*' | '(' | ')' | ':' | '^' | '-'))
        .collect()
}

/// Generate pinyin tokens from Chinese text.
///
/// Returns both the full pinyin concatenation (e.g. "zhongguoren" for "中国人")
/// and the abbreviation (first letter of each pinyin, e.g. "zgr").
pub fn generate_pinyin_tokens(chinese_text: &str) -> Option<Vec<String>> {
    let chinese = extract_chinese(chinese_text);
    if chinese.is_empty() || chinese.chars().count() > MAX_PINYIN_CHARS {
        return None;
    }

    // Use pinyin crate to generate pinyin for each character
    let pinyins: Vec<String> = chinese
        .chars()
        .filter_map(|c| c.to_pinyin().map(|p| p.plain().to_string()))
        .collect();

    if pinyins.is_empty() {
        return None;
    }

    let mut result = Vec::new();

    // Full pinyin concatenation: "中国人" → "zhongguoren"
    let full: String = pinyins.join("");
    if !full.is_empty() {
        result.push(full);
    }

    // Abbreviation: first letter of each pinyin: "中国人" → "zgr"
    let abbrev: String = pinyins.iter().filter_map(|s| s.chars().next()).collect();
    if !abbrev.is_empty() {
        result.push(abbrev);
    }

    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

fn split_name_extension(name: &str) -> (String, String) {
    match name.rfind('.') {
        Some(pos) => (name[..pos].to_string(), name[pos + 1..].to_string()),
        None => (name.to_string(), String::new()),
    }
}
