//! Contextual Embedding 文本生成
//!
//! Embedding 阶段使用带文档标题、章节路径、页码等上下文的增强文本，
//! 搜索结果展示仍使用原始 content，两者分离。

/// 构建用于 embedding 的上下文增强文本。
///
/// 格式：
/// ```text
/// 文档：{document_title}
/// 章节：{title_path}
/// 页码：{page_number}
/// 内容：{content}
/// ```
///
/// 空字段会自动省略对应行。
pub fn build_embedding_text(
    document_title: &str,
    title_path: &str,
    page_number: Option<i32>,
    content: &str,
) -> String {
    let mut parts = Vec::with_capacity(4);

    if !document_title.is_empty() {
        parts.push(format!("文档：{}", document_title));
    }
    if !title_path.is_empty() {
        parts.push(format!("章节：{}", title_path));
    }
    if let Some(pn) = page_number {
        if pn > 0 {
            parts.push(format!("页码：{}", pn));
        }
    }
    parts.push(format!("内容：{}", content));

    parts.join("\n")
}

/// 为 micro 文档（全文节点）构建 embedding 文本。
pub fn build_micro_embedding_text(document_title: &str, content: &str) -> String {
    build_embedding_text(document_title, "", None, content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_full_context() {
        let text = build_embedding_text("测试文档", "第一章 / 概述", Some(42), "这是正文内容");
        assert!(text.contains("文档：测试文档"));
        assert!(text.contains("章节：第一章 / 概述"));
        assert!(text.contains("页码：42"));
        assert!(text.contains("内容：这是正文内容"));
    }

    #[test]
    fn test_build_minimal() {
        let text = build_embedding_text("", "", None, "正文");
        assert_eq!(text, "内容：正文");
    }

    #[test]
    fn test_build_micro() {
        let text = build_micro_embedding_text("微型文档", "全文内容");
        assert!(text.contains("文档：微型文档"));
        assert!(text.contains("内容：全文内容"));
        assert!(!text.contains("章节："));
        assert!(!text.contains("页码："));
    }

    #[test]
    fn test_page_zero_skipped() {
        let text = build_embedding_text("标题", "路径", Some(0), "内容");
        assert!(!text.contains("页码"));
    }
}
