//! 路由策略模块 — 根据内容类型、意图、策略推断投影计划（设计文档 §6）
//!
//! 输入：content/path/url/text, intent, target_slug, file extension, mime type, caller context
//! 输出：to_kb, to_brain_shadow, to_brain_page, to_file, extract_links, extract_timeline,
//!       generate_review_changes, auto_apply_policy

use crate::artifact::types::{RoutePlan, UploadIntent};

/// 根据意图、扩展名和 MIME 类型推断路由计划
///
/// 设计文档 §6.2 路由规则:
/// | 条件                                  | Artifact | KB | Shadow | File | Promotion |
/// |--------------------------------------|----------|----|--------|------|-----------|
/// | --intent attachment                  | yes      | no | no     | yes  | none      |
/// | --intent evidence                    | yes      | yes| yes    | no   | candidate |
/// | --intent memory                      | yes      | yes| yes    | no   | auto-low  |
/// | --intent promote                     | yes      | yes| yes    | no   | candidate |
/// | PDF/DOCX/XLSX/CSV/HTML/TXT with auto| yes      | yes| yes    | no   | candidate |
/// | Raw Markdown with auto               | yes      | yes| yes    | no   | candidate |
/// | Markdown with gbrain frontmatter     | optional | no | no     | no   | direct put_page |
/// | Code file/repo with auto             | optional | no | no     | no   | code import/sync |
/// | Image/binary with auto               | yes      | no | no     | yes  | none      |
pub fn infer_route_plan(extension: &str, mime: &str, intent: &UploadIntent) -> RoutePlan {
    // 委托给 types.rs 中已有的 infer_route_plan 函数
    crate::artifact::types::infer_route_plan(extension, mime, intent)
}

/// 根据用户友好的 ArtifactIntent 推断路由计划
pub fn infer_route_plan_from_artifact_intent(
    extension: &str,
    mime: &str,
    intent_str: &str,
) -> RoutePlan {
    let intent = match intent_str {
        "memory" => UploadIntent::Memory,
        "evidence" => UploadIntent::Document, // evidence -> 内部 Document
        "promote" => UploadIntent::Promote,
        "attachment" => UploadIntent::Attachment,
        _ => UploadIntent::Auto,
    };
    infer_route_plan(extension, mime, &intent)
}
