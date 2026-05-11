//! HTML parser with structure cleaning and heading hierarchy preservation.
//!
//! P2-009: Removes script, style, nav, footer before text extraction.
//! P2-010: Preserves h1-h6 hierarchy and title/meta extraction.

use super::{DocumentParser, ParsedDocument};
use crate::error::GBrainError;
use std::collections::HashMap;

pub struct HtmlParser;

impl Default for HtmlParser {
    fn default() -> Self {
        Self::new()
    }
}

impl HtmlParser {
    pub fn new() -> Self {
        Self
    }
}

impl DocumentParser for HtmlParser {
    fn parse(&self, data: &[u8]) -> Result<ParsedDocument, GBrainError> {
        let raw = std::str::from_utf8(data)
            .map_err(|e| GBrainError::FileError(format!("HTML is not valid UTF-8: {}", e)))?;

        // P2-009: 清理非内容元素
        let cleaned = clean_html(raw);

        // 提取 metadata（title, meta 标签）
        let mut metadata = HashMap::new();
        extract_metadata(raw, &mut metadata);

        // P2-010: 提取标题层级
        let headings = extract_headings(raw);
        if !headings.is_empty() {
            metadata.insert("headings".to_string(), headings.join(" > "));
        }

        // 正文提取
        let text = html2text::from_read(cleaned.as_bytes(), 80)
            .map_err(|e| GBrainError::FileError(format!("HTML parse failed: {}", e)))?;
        let content = clean_html_text(&text);

        // P1-010/P2-010: 构建结构化 blocks（按标题层级分段）
        let blocks = build_html_blocks(&text, &headings);
        Ok(ParsedDocument {
            content,
            metadata,
            blocks: Some(blocks),
        })
    }

    fn extensions(&self) -> &[&str] {
        &["html", "htm"]
    }
}

/// P2-009: 移除 script/style/nav/footer/隐藏元素
fn clean_html(raw: &str) -> String {
    let mut result = raw.to_string();

    // 移除 <script>...</script>
    result = remove_tags(&result, "script");
    // 移除 <style>...</style>
    result = remove_tags(&result, "style");
    // 移除 <nav>...</nav>
    result = remove_tags(&result, "nav");
    // 移除 <footer>...</footer>
    result = remove_tags(&result, "footer");
    // 移除 <header>...</header>
    result = remove_tags(&result, "header");
    // 移除 <noscript>...</noscript>
    result = remove_tags(&result, "noscript");

    // 移除 hidden/display:none 元素
    if let Ok(re) = regex::Regex::new(
        r#"(?i)<[^>]*\b(?:display\s*:\s*none|visibility\s*:\s*hidden|aria-hidden\s*=\s*['"]true['"])[^>]*>.*?</[^>]+>"#,
    ) {
        result = re.replace_all(&result, "").to_string();
    }

    result
}

fn remove_tags(html: &str, tag: &str) -> String {
    let pattern = format!(
        r"(?is)<{}[\s>].*?</{}>",
        regex::escape(tag),
        regex::escape(tag)
    );
    if let Ok(re) = regex::Regex::new(&pattern) {
        re.replace_all(html, "").to_string()
    } else {
        html.to_string()
    }
}

/// P2-010: 提取 h1-h6 标题层级
fn extract_headings(raw: &str) -> Vec<String> {
    let mut headings = Vec::new();
    for level in 1..=6 {
        let pattern = format!(r"<h{}[^>]*>(.*?)</h{}>", level, level);
        if let Ok(re) = regex::Regex::new(&pattern) {
            for cap in re.captures_iter(raw) {
                if let Some(m) = cap.get(1) {
                    let text = strip_tags(m.as_str()).trim().to_string();
                    if !text.is_empty() {
                        headings.push(text);
                    }
                }
            }
        }
    }
    headings
}

/// 提取 <title> 和 <meta> 标签内容到 metadata
fn extract_metadata(raw: &str, meta: &mut HashMap<String, String>) {
    // <title>
    if let Ok(re) = regex::Regex::new(r"<title[^>]*>(.*?)</title>") {
        if let Some(cap) = re.captures(raw) {
            if let Some(m) = cap.get(1) {
                let title = strip_tags(m.as_str()).trim().to_string();
                if !title.is_empty() {
                    meta.insert("title".to_string(), title);
                }
            }
        }
    }
    // <meta name="description" content="...">
    for name in &["description", "keywords", "author"] {
        let pattern = format!(
            r#"<meta\s+name=["']{}["']\s+content=["']([^"']+)["']"#,
            regex::escape(name)
        );
        if let Ok(re) = regex::Regex::new(&pattern) {
            if let Some(cap) = re.captures(raw) {
                if let Some(m) = cap.get(1) {
                    meta.insert(name.to_string(), m.as_str().to_string());
                }
            }
        }
    }
}

fn strip_tags(html: &str) -> String {
    if let Ok(re) = regex::Regex::new(r"<[^>]*>") {
        re.replace_all(html, "").to_string()
    } else {
        html.to_string()
    }
}

fn clean_html_text(text: &str) -> String {
    text.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// P2-010: 从纯文本和标题列表构建 ParsedBlock
fn build_html_blocks(text: &str, headings: &[String]) -> Vec<crate::kb::types::ParsedBlock> {
    let heading_str = if headings.is_empty() {
        String::new()
    } else {
        headings.join(" > ")
    };
    let paragraphs: Vec<&str> = text
        .split("\n\n")
        .filter(|p| !p.trim().is_empty())
        .collect();
    if paragraphs.is_empty() {
        return vec![crate::kb::types::ParsedBlock::paragraph(text)];
    }
    let mut offset = 0usize;
    paragraphs
        .iter()
        .map(|p| {
            let start = offset as i32;
            offset += p.len() + 2; // +2 for \n\n separator
            crate::kb::types::ParsedBlock {
                text: p.to_string(),
                title_path: heading_str.clone(),
                page_number: None,
                source_start: Some(start),
                source_end: Some(offset as i32 - 2),
                block_type: "paragraph".to_string(),
                metadata: String::new(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_script() {
        let html = "<html><head><script>alert('xss')</script></head><body>Hello</body></html>";
        let cleaned = clean_html(html);
        assert!(!cleaned.contains("alert"));
        assert!(cleaned.contains("Hello"));
    }

    #[test]
    fn test_remove_nav_footer() {
        let html = "<html><body><nav>Menu</nav><main>Content</main><footer>Copyright</footer></body></html>";
        let cleaned = clean_html(html);
        assert!(!cleaned.contains("Menu"));
        assert!(!cleaned.contains("Copyright"));
        assert!(cleaned.contains("Content"));
    }

    #[test]
    fn test_extract_headings() {
        let html = "<html><body><h1>Title</h1><h2>Section A</h2><h3>Detail</h3></body></html>";
        let headings = extract_headings(html);
        assert_eq!(headings, vec!["Title", "Section A", "Detail"]);
    }

    #[test]
    fn test_extract_title_meta() {
        let html = r#"<html><head><title>My Page</title><meta name="description" content="A test page"></head></html>"#;
        let mut meta = HashMap::new();
        extract_metadata(html, &mut meta);
        assert_eq!(meta.get("title").unwrap(), "My Page");
        assert_eq!(meta.get("description").unwrap(), "A test page");
    }

    #[test]
    fn test_full_parse() {
        let html = r#"<html><head><title>Test</title><style>body{color:red}</style></head><body><nav>Menu</nav><h1>Hello</h1><p>World</p><script>evil()</script><footer>bye</footer></body></html>"#;
        let parser = HtmlParser::new();
        let result = parser.parse(html.as_bytes()).unwrap();
        assert!(result.content.contains("Hello"));
        assert!(result.content.contains("World"));
        assert!(!result.content.contains("evil"));
        assert!(!result.content.contains("Menu"));
        assert!(!result.content.contains("bye"));
        assert!(result.metadata.contains_key("title"));
    }
}
