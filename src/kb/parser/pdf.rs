//! PDF parser with page-level extraction, header/footer cleaning, and OCR tagging
//!
//! P2-004: Outputs page_number metadata per block
//! P2-005: Heuristic header/footer removal
//! P2-006: Text density detection → needs_ocr flag
//! Phase 1: 增强页级分析 — 图片检测、矢量对象、完整 OCR metadata

use super::{DocumentParser, ParsedDocument};
use crate::error::GBrainError;
use std::collections::HashMap;

pub struct PdfParser;

/// 快速获取 PDF 页数，不解析文本内容
pub fn count_pdf_pages(data: &[u8]) -> Result<usize, GBrainError> {
    let pdf = lopdf::Document::load_mem(data)
        .map_err(|e| GBrainError::FileError(format!("PDF load failed: {}", e)))?;
    Ok(pdf.get_pages().len())
}

impl Default for PdfParser {
    fn default() -> Self {
        Self::new()
    }
}

impl PdfParser {
    pub fn new() -> Self {
        Self
    }
}

impl DocumentParser for PdfParser {
    fn parse(&self, data: &[u8]) -> Result<ParsedDocument, GBrainError> {
        let pdf = lopdf::Document::load_mem(data)
            .map_err(|e| GBrainError::FileError(format!("PDF load failed: {}", e)))?;

        let pages = pdf.get_pages();
        let total_pages = pages.len();
        // get_pages() 返回 BTreeMap<u32, ObjectId>，value 是 PDF 内真实的 page object ID
        let page_ids: Vec<(u32, lopdf::ObjectId)> = pages.into_iter().collect();

        // FIX9-04: 在拼接全文时累计字符偏移，为每页 block 写入 source span
        let mut page_texts: Vec<String> = Vec::new();
        let mut all_text = Vec::new();
        let mut low_density_pages = 0u32;
        // 记录每页 block 在全文中的起止偏移
        let mut page_spans: Vec<(usize, usize)> = Vec::new();
        let mut global_offset: usize = 0;

        // Phase 1: 页级分析结果
        let mut page_analyses: Vec<serde_json::Value> = Vec::new();
        let mut image_rich_pages: Vec<i32> = Vec::new();
        let mut uncertain_pages: Vec<i32> = Vec::new();

        for (page_idx, (page_num, page_obj_id)) in page_ids.iter().enumerate() {
            // Phase 1: 分析页面对象，检测图片和矢量对象
            let (image_count, image_area_ratio, has_vector_objects) =
                analyze_page_objects(&pdf, *page_obj_id);

            let page_num_i32 = (page_idx + 1) as i32;

            if image_area_ratio >= 0.08 || image_count >= 1 {
                image_rich_pages.push(page_num_i32);
            }
            if has_vector_objects {
                uncertain_pages.push(page_num_i32);
            }

            match pdf.extract_text(&[*page_num]) {
                Ok(text) => {
                    let cleaned = clean_text(&text);
                    let deduped = remove_header_footer(&cleaned, &page_texts);

                    let density = deduped.chars().count();
                    if density < 50 {
                        low_density_pages += 1;
                    }

                    // Phase 1: 记录页级分析
                    page_analyses.push(serde_json::json!({
                        "page_number": page_num_i32,
                        "text": deduped,
                        "char_count": density,
                        "image_area_ratio": image_area_ratio,
                        "image_count": image_count,
                        "has_vector_or_unknown_objects": has_vector_objects,
                    }));

                    let page_block = format!("[PAGE:{}]\n{}", page_num, deduped);
                    // FIX9-04: 记录此页在全文中的起止偏移
                    let start = global_offset;
                    // FIX10-08: 统一使用字符偏移（chars().count()），禁止混用 byte 长度
                    let block_len = page_block.chars().count();
                    global_offset += block_len + 2; // 加上 "\n\n" 分隔符长度
                    let end = start + block_len;
                    page_spans.push((start, end));
                    page_texts.push(deduped.clone());
                    if !deduped.is_empty() {
                        all_text.push(page_block);
                    }
                }
                Err(_) => {
                    // Phase 1: 解析失败的页也要记录分析
                    page_analyses.push(serde_json::json!({
                        "page_number": page_num_i32,
                        "text": "",
                        "char_count": 0,
                        "image_area_ratio": image_area_ratio,
                        "image_count": image_count,
                        "has_vector_or_unknown_objects": has_vector_objects,
                    }));
                    // 空页也要记录 span 占位
                    page_spans.push((global_offset, global_offset));
                    continue;
                }
            }
        }

        let content = all_text.join("\n\n");

        let mut metadata = HashMap::new();
        metadata.insert("total_pages".to_string(), total_pages.to_string());
        // P2-004: 每页文本以 JSON 数组记录（含 page_number）
        metadata.insert(
            "page_texts".to_string(),
            serde_json::to_string(&page_texts).unwrap_or_default(),
        );
        // P2-006: 文本密度标记
        let needs_ocr = low_density_pages as f64 / total_pages.max(1) as f64 > 0.5;
        metadata.insert("needs_ocr".to_string(), needs_ocr.to_string());
        metadata.insert(
            "low_density_pages".to_string(),
            low_density_pages.to_string(),
        );

        // Phase 1: 完整 OCR metadata
        metadata.insert(
            "page_analyses".to_string(),
            serde_json::to_string(&page_analyses).unwrap_or_default(),
        );
        metadata.insert(
            "image_rich_pages".to_string(),
            serde_json::to_string(&image_rich_pages).unwrap_or_default(),
        );
        metadata.insert(
            "uncertain_pages".to_string(),
            serde_json::to_string(&uncertain_pages).unwrap_or_default(),
        );

        // FIX9-04: 为每页 block 写入真实的 source span（基于 page_spans）
        let blocks: Vec<crate::kb::types::ParsedBlock> = page_texts
            .iter()
            .enumerate()
            .map(|(i, text)| {
                let (start, end) = page_spans.get(i).copied().unwrap_or((0, 0));
                crate::kb::types::ParsedBlock {
                    text: text.clone(),
                    title_path: String::new(),
                    page_number: Some((i + 1) as i32),
                    source_start: Some(start as i32),
                    source_end: Some(end as i32),
                    block_type: "page".to_string(),
                    metadata: String::new(),
                }
            })
            .collect();

        Ok(ParsedDocument {
            content,
            metadata,
            blocks: Some(blocks),
        })
    }

    fn extensions(&self) -> &[&str] {
        &["pdf"]
    }
}

/// 将 lopdf Object 转为 f64（支持 Integer 和 Real 两种变体）
fn object_to_f64(obj: &lopdf::Object) -> Option<f64> {
    match obj {
        lopdf::Object::Real(f) => Some(*f as f64),
        lopdf::Object::Integer(i) => Some(*i as f64),
        _ => None,
    }
}

/// 解析 XObject 字典中的间接引用，返回实际对象
fn resolve_xobject<'a>(
    pdf: &'a lopdf::Document,
    obj: &'a lopdf::Object,
) -> Option<&'a lopdf::Object> {
    match obj {
        lopdf::Object::Reference(obj_id) => pdf.get_object(*obj_id).ok(),
        other => Some(other),
    }
}

/// 将 Object 解析为字典引用，处理间接引用
fn resolve_as_dict<'a>(
    pdf: &'a lopdf::Document,
    obj: &'a lopdf::Object,
) -> Option<&'a lopdf::Dictionary> {
    match obj {
        lopdf::Object::Reference(obj_id) => pdf.get_object(*obj_id).ok()?.as_dict().ok(),
        lopdf::Object::Dictionary(d) => Some(d),
        _ => None,
    }
}

/// 获取页面的 Resources 字典，解析间接引用并沿父 Pages 节点继承资源
fn get_inherited_resources<'a>(
    pdf: &'a lopdf::Document,
    page_obj_id: lopdf::ObjectId,
) -> Option<&'a lopdf::Dictionary> {
    let mut current_id = page_obj_id;
    loop {
        let obj = pdf.get_object(current_id).ok()?;
        let dict = obj.as_dict().ok()?;
        if let Ok(resources) = dict.get(b"Resources") {
            return resolve_as_dict(pdf, resources);
        }
        // 当前节点无 Resources，沿 Parent 向上查找
        let parent = dict.get(b"Parent").ok()?;
        match parent {
            lopdf::Object::Reference(id) => current_id = *id,
            _ => return None,
        }
    }
}

/// 检测 PDF 页面 content stream 中的 inline image（BI/ID/EI 操作符）
///
/// XObject 只能检测命名的 Image 资源，inline image 通过 BI/ID/EI
/// 操作符直接嵌入在 content stream 中，需要解析 stream 才能发现。
fn detect_inline_images(pdf: &lopdf::Document, page_obj_id: lopdf::ObjectId) -> usize {
    let page_obj = match pdf.get_object(page_obj_id) {
        Ok(obj) => obj,
        Err(_) => return 0,
    };
    let page_dict = match page_obj.as_dict() {
        Ok(d) => d,
        Err(_) => return 0,
    };

    let contents = match page_dict.get(b"Contents") {
        Ok(c) => c,
        Err(_) => return 0,
    };

    let stream_data = collect_content_stream(pdf, contents);
    if stream_data.is_empty() {
        return 0;
    }

    // 在 content stream 中搜索 BI 操作符（标记 inline image 开始）
    // BI 必须作为独立 token 出现，前后需为 PDF 分隔符
    let mut count = 0usize;
    let data = &stream_data;
    let mut i = 0;
    while i + 1 < data.len() {
        if data[i] == b'B' && data[i + 1] == b'I' {
            let prev_ok = i == 0 || is_pdf_delimiter(data[i - 1]);
            let next_ok = i + 2 >= data.len() || is_pdf_delimiter(data[i + 2]);
            if prev_ok && next_ok {
                count += 1;
            }
        }
        i += 1;
    }
    count
}

/// 汇总页面 content stream 数据（处理单个 stream 或 stream 数组）
fn collect_content_stream(pdf: &lopdf::Document, contents: &lopdf::Object) -> Vec<u8> {
    match contents {
        lopdf::Object::Stream(stream) => stream.decompressed_content().unwrap_or_default(),
        lopdf::Object::Reference(id) => match pdf.get_object(*id) {
            Ok(lopdf::Object::Stream(s)) => s.decompressed_content().unwrap_or_default(),
            _ => vec![],
        },
        lopdf::Object::Array(arr) => {
            let mut data = Vec::new();
            for obj in arr {
                let stream_data = match obj {
                    lopdf::Object::Reference(id) => match pdf.get_object(*id) {
                        Ok(lopdf::Object::Stream(s)) => {
                            s.decompressed_content().unwrap_or_default()
                        }
                        _ => continue,
                    },
                    lopdf::Object::Stream(s) => s.decompressed_content().unwrap_or_default(),
                    _ => continue,
                };
                if !stream_data.is_empty() {
                    if !data.is_empty() {
                        data.push(b' ');
                    }
                    data.extend_from_slice(&stream_data);
                }
            }
            data
        }
        _ => vec![],
    }
}

/// 判断字节是否为 PDF 分隔符（空白或结构字符）
fn is_pdf_delimiter(b: u8) -> bool {
    matches!(
        b,
        b' ' | b'\t'
            | b'\r'
            | b'\n'
            | b'\x0c'
            | b'('
            | b')'
            | b'<'
            | b'>'
            | b'['
            | b']'
            | b'{'
            | b'}'
            | b'/'
            | b'%'
    )
}

/// 分析 PDF 页面中的对象，检测图片和矢量对象
///
/// 返回 (image_count, image_area_ratio, has_vector_objects)
/// `page_obj_id` 为 get_pages() 返回的真实 ObjectId，不可用页码构造
fn analyze_page_objects(pdf: &lopdf::Document, page_obj_id: lopdf::ObjectId) -> (usize, f64, bool) {
    let mut image_count = 0usize;
    let mut total_image_area: f64 = 0.0;
    let mut has_vector_objects = false;
    let mut page_area: f64 = 1.0; // 默认 1.0 避免除零

    // 获取页面尺寸
    if let Ok(page_obj) = pdf.get_object(page_obj_id) {
        if let Ok(page_dict) = page_obj.as_dict() {
            // MediaBox 定义页面尺寸
            if let Ok(mediabox) = page_dict.get(b"MediaBox") {
                if let Ok(arr) = mediabox.as_array() {
                    if arr.len() >= 4 {
                        let w = object_to_f64(&arr[2]).unwrap_or(612.0)
                            - object_to_f64(&arr[0]).unwrap_or(0.0);
                        let h = object_to_f64(&arr[3]).unwrap_or(792.0)
                            - object_to_f64(&arr[1]).unwrap_or(0.0);
                        page_area = (w * h).max(1.0);
                    }
                }
            }
        }
    }

    // 遍历页面资源中的 XObject，检测图片
    // 使用 get_inherited_resources 解析间接引用并沿父 Pages 节点继承资源
    if let Some(res_dict) = get_inherited_resources(pdf, page_obj_id) {
        if let Ok(xobjects) = res_dict.get(b"XObject") {
            if let Some(xobj_dict) = resolve_as_dict(pdf, xobjects) {
                for (_name, obj) in xobj_dict.iter() {
                    let Some(xobj) = resolve_xobject(pdf, obj) else {
                        continue;
                    };
                    if let Ok(xobj_dict) = xobj.as_dict() {
                        // 检查 Subtype
                        if let Ok(subtype) = xobj_dict.get(b"Subtype") {
                            match subtype {
                                lopdf::Object::Name(name) if name == b"Image" => {
                                    image_count += 1;
                                    // 估算图片面积
                                    let img_w = xobj_dict
                                        .get(b"Width")
                                        .ok()
                                        .and_then(|w| object_to_f64(w))
                                        .unwrap_or(100.0);
                                    let img_h = xobj_dict
                                        .get(b"Height")
                                        .ok()
                                        .and_then(|h| object_to_f64(h))
                                        .unwrap_or(100.0);
                                    total_image_area += img_w * img_h;
                                }
                                lopdf::Object::Name(name) if name == b"Form" => {
                                    // Form XObject 可能包含矢量图形
                                    has_vector_objects = true;
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }

        // 检查是否有 Graphics State（可能指示复杂矢量图形）
        if res_dict.get(b"ExtGState").is_ok() {
            // 有扩展图形状态，可能是矢量对象
            has_vector_objects = true;
        }
    }

    // 检测 content stream 中的 inline image (BI 操作符)
    // XObject 检测无法覆盖 inline image，需要解析 content stream
    let inline_image_count = detect_inline_images(pdf, page_obj_id);
    if inline_image_count > 0 {
        image_count += inline_image_count;
        // 无法精确估算 inline image 面积，保守按每张占页面 50% 估算
        total_image_area += page_area * 0.5 * inline_image_count as f64;
    }

    // 计算图片面积占比（图片面积之和 / 页面面积）
    // 由于图片坐标可能未归一化，使用简单估算
    let image_area_ratio = if page_area > 0.0 {
        (total_image_area / page_area).min(1.0)
    } else {
        0.0
    };

    (image_count, image_area_ratio, has_vector_objects)
}

fn clean_text(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut result = Vec::new();
    let mut prev_empty = false;

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !prev_empty {
                result.push(String::new());
                prev_empty = true;
            }
        } else {
            let normalized: String = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
            result.push(normalized);
            prev_empty = false;
        }
    }

    result.join("\n")
}

/// P2-005: 启发式去除页眉页脚 — 检测与前页重复的首/尾行
fn remove_header_footer(text: &str, previous_pages: &[String]) -> String {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() <= 2 || previous_pages.is_empty() {
        return text.to_string();
    }

    let first = lines[0].trim();
    let last = lines.last().map(|l| l.trim()).unwrap_or("");

    let mut remove_first = false;
    let mut remove_last = false;

    // 检查与先前页面的重复
    for prev in previous_pages.iter().rev().take(3) {
        let prev_lines: Vec<&str> = prev.lines().collect();
        if !prev_lines.is_empty() && prev_lines[0].trim() == first {
            remove_first = true;
        }
        if let Some(prev_last) = prev_lines.last() {
            if prev_last.trim() == last && last.chars().count() < 50 {
                remove_last = true;
            }
        }
    }

    // 检测纯数字行（页码）
    if first.chars().all(|c| c.is_ascii_digit()) {
        remove_first = true;
    }
    if last.chars().all(|c| c.is_ascii_digit()) {
        remove_last = true;
    }

    let range = if remove_first { 1 } else { 0 }..if remove_last {
        lines.len().saturating_sub(1)
    } else {
        lines.len()
    };

    lines[range].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_header_footer_repeat() {
        let page1 = "Chapter 1\nSome content\nPage 1";
        let page2 = "Chapter 1\nMore content\nPage 2";
        let previous = vec![page1.to_string()];
        let result = remove_header_footer(page2, &previous);
        assert!(!result.contains("Chapter 1"));
        assert!(result.contains("More content"));
        // 页码被移除（footer 变了所以不会被移除，因为不匹配 previous）
    }

    #[test]
    fn test_clean_text() {
        let text = "Hello   world\n\n   \nFoo bar";
        let result = clean_text(text);
        assert!(result.contains("Hello world"));
        assert!(result.contains("Foo bar"));
    }
}
