//! OCR 检测器 — 判断 PDF 页面是否需要 OCR
//!
//! 基于文本密度、图片覆盖率、嵌入图片数量、矢量对象等指标，
//! 输出需要 OCR 的页集合和原因。

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// OCR 判定原因
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OcrReason {
    /// 文本密度低
    LowTextDensity,
    /// 文本层为空
    EmptyTextLayer,
    /// 图片面积占比高
    ImageArea,
    /// 嵌入图片数量多
    EmbeddedImage,
    /// 存在矢量或未知对象
    VectorOrUnknownObjects,
    /// 解析错误
    ParserError,
    /// 字体编码问题
    FontEncodingIssue,
    /// 隐藏或不可见文本层
    HiddenOrInvisibleTextLayer,
    /// 强制全页 OCR
    ForcedAllPages,
}

/// OCR 判定结果
#[derive(Debug, Clone)]
pub struct OcrDetection {
    /// 总页数
    pub total_pages: usize,
    /// 低文本密度页
    pub low_density_pages: Vec<i32>,
    /// 图片丰富页
    pub image_rich_pages: Vec<i32>,
    /// 不确定页（需要 OCR 以确认）
    pub uncertain_pages: Vec<i32>,
    /// 实际需要 OCR 的页
    pub ocr_pages: Vec<i32>,
    /// 是否需要 OCR
    pub needs_ocr: bool,
    /// 低密度页占比
    pub low_density_ratio: f64,
    /// 每页触发 OCR 的原因
    pub reasons_by_page: BTreeMap<i32, Vec<OcrReason>>,
}

/// OCR 范围
#[derive(Debug, Clone, PartialEq)]
pub enum OcrScope {
    /// 无需 OCR
    None,
    /// 部分页需要 OCR
    Partial,
    /// 全部页需要 OCR
    Full,
}

impl OcrDetection {
    /// 获取 OCR 范围
    pub fn scope(&self) -> OcrScope {
        if self.ocr_pages.is_empty() {
            OcrScope::None
        } else if self.ocr_pages.len() == self.total_pages {
            OcrScope::Full
        } else {
            OcrScope::Partial
        }
    }
}

/// 页内文本块（预留，lopdf 当前无法可靠获取文本块坐标时为空 Vec）
#[derive(Debug, Clone)]
pub struct PdfTextBlock {
    pub page_number: i32,
    pub text: String,
    pub bbox: Option<[f64; 4]>,
    pub source_start: usize,
    pub source_end: usize,
}

/// 页级分析结果（由 PdfParser 输出）
#[derive(Debug, Clone)]
pub struct PdfPageAnalysis {
    /// 页码（1-based）
    pub page_number: i32,
    /// 提取的文本
    pub text: String,
    /// 文本块列表（当前 lopdf 无法可靠获取坐标，预留为空 Vec）
    pub text_blocks: Vec<PdfTextBlock>,
    /// 字符数
    pub char_count: usize,
    /// 图片区域列表
    pub image_regions: Vec<PdfImageRegion>,
    /// 图片面积占比
    pub image_area_ratio: f64,
    /// 是否存在矢量或未知对象
    pub has_vector_or_unknown_objects: bool,
    /// 页面宽度
    pub width: Option<u32>,
    /// 页面高度
    pub height: Option<u32>,
}

/// PDF 图片区域
#[derive(Debug, Clone)]
pub struct PdfImageRegion {
    /// 归一化边界框 [x1, y1, x2, y2]
    pub bbox: Option<[f64; 4]>,
    /// 面积占比
    pub area_ratio: f64,
}

/// 执行 OCR 检测
///
/// 根据配置的阈值和每页分析结果，判断哪些页需要 OCR。
pub fn detect_ocr_pages(
    pages: &[PdfPageAnalysis],
    text_density_threshold: usize,
    image_area_threshold: f64,
    image_count_threshold: usize,
    min_low_density_ratio: f64,
    mode: &crate::kb::ocr_provider::OcrMode,
) -> OcrDetection {
    let total_pages = pages.len().max(1);
    let mut reasons_by_page: BTreeMap<i32, Vec<OcrReason>> = BTreeMap::new();
    let mut low_density_pages = Vec::new();
    let mut image_rich_pages = Vec::new();
    let mut uncertain_pages = Vec::new();

    let is_all_pages = matches!(mode, crate::kb::ocr_provider::OcrMode::AllPages);

    for page in pages {
        let mut reasons = Vec::new();

        // 空文本层
        if page.text.trim().is_empty() {
            reasons.push(OcrReason::EmptyTextLayer);
        }

        // 低文本密度
        if page.text.trim().chars().count() < text_density_threshold {
            reasons.push(OcrReason::LowTextDensity);
            low_density_pages.push(page.page_number);
        }

        // 图片面积占比
        if page.image_area_ratio >= image_area_threshold {
            reasons.push(OcrReason::ImageArea);
            image_rich_pages.push(page.page_number);
        }

        // 嵌入图片数量
        if page.image_regions.len() >= image_count_threshold {
            reasons.push(OcrReason::EmbeddedImage);
        }

        // 矢量或未知对象
        if page.has_vector_or_unknown_objects {
            reasons.push(OcrReason::VectorOrUnknownObjects);
            uncertain_pages.push(page.page_number);
        }

        // 强制全页模式
        if is_all_pages {
            reasons.push(OcrReason::ForcedAllPages);
        }

        if !reasons.is_empty() {
            reasons_by_page.insert(page.page_number, reasons);
        }
    }

    let low_density_ratio = if total_pages > 0 {
        low_density_pages.len() as f64 / total_pages as f64
    } else {
        0.0
    };

    // 低密度页比例未超过阈值时，移除 LowTextDensity 原因
    // 避免纯文本 PDF 的封面/目录空白页被误送 OCR
    if low_density_ratio < min_low_density_ratio {
        for reasons in reasons_by_page.values_mut() {
            reasons.retain(|r| !matches!(r, OcrReason::LowTextDensity));
        }
        reasons_by_page.retain(|_, reasons| !reasons.is_empty());
    }

    let ocr_pages: Vec<i32> = reasons_by_page.keys().copied().collect();
    let needs_ocr = !ocr_pages.is_empty();

    OcrDetection {
        total_pages,
        low_density_pages,
        image_rich_pages,
        uncertain_pages,
        ocr_pages,
        needs_ocr,
        low_density_ratio,
        reasons_by_page,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_page(
        page_number: i32,
        text: &str,
        image_ratio: f64,
        image_count: usize,
        has_vec: bool,
    ) -> PdfPageAnalysis {
        let regions: Vec<PdfImageRegion> = (0..image_count)
            .map(|_| PdfImageRegion {
                bbox: Some([0.0, 0.0, 0.5, 0.5]),
                area_ratio: image_ratio,
            })
            .collect();
        PdfPageAnalysis {
            page_number,
            text: text.to_string(),
            text_blocks: vec![],
            char_count: text.chars().count(),
            image_regions: regions,
            image_area_ratio: image_ratio,
            has_vector_or_unknown_objects: has_vec,
            width: None,
            height: None,
        }
    }

    #[test]
    fn test_detect_pure_text_pdf() {
        let pages = vec![
            make_page(1, "这是一段足够长的文本内容用于测试", 0.0, 0, false),
            make_page(2, "第二页也有一些文本内容用于测试", 0.0, 0, false),
        ];
        let result = detect_ocr_pages(
            &pages,
            50,
            0.08,
            1,
            0.3,
            &crate::kb::ocr_provider::OcrMode::Auto,
        );
        assert!(!result.needs_ocr);
        assert_eq!(result.scope(), OcrScope::None);
        assert!(result.ocr_pages.is_empty());
    }

    #[test]
    fn test_detect_scanned_page() {
        let pages = vec![
            make_page(1, "", 0.92, 1, false),
            make_page(2, "", 0.85, 1, false),
        ];
        let result = detect_ocr_pages(
            &pages,
            50,
            0.08,
            1,
            0.3,
            &crate::kb::ocr_provider::OcrMode::Auto,
        );
        assert!(result.needs_ocr);
        assert_eq!(result.ocr_pages, vec![1, 2]);
        assert_eq!(result.scope(), OcrScope::Full);
    }

    #[test]
    fn test_detect_mixed_pdf() {
        let pages = vec![
            make_page(1, "这页文字很充足而且没有图片", 0.0, 0, false),
            make_page(2, "", 0.9, 1, false),
            make_page(3, "短", 0.0, 0, false),
        ];
        let result = detect_ocr_pages(
            &pages,
            50,
            0.08,
            1,
            0.3,
            &crate::kb::ocr_provider::OcrMode::Auto,
        );
        assert!(result.needs_ocr);
        assert_eq!(result.ocr_pages, vec![2, 3]);
        assert_eq!(result.scope(), OcrScope::Partial);
    }

    #[test]
    fn test_detect_image_rich_page_with_text() {
        // 文本充足但图片占比高
        let pages = vec![make_page(1, &"文本内容".repeat(20), 0.5, 2, false)];
        let result = detect_ocr_pages(
            &pages,
            50,
            0.08,
            1,
            0.3,
            &crate::kb::ocr_provider::OcrMode::Auto,
        );
        assert!(result.needs_ocr);
        assert!(result.reasons_by_page[&1]
            .iter()
            .any(|r| matches!(r, OcrReason::ImageArea)));
        assert!(result.reasons_by_page[&1]
            .iter()
            .any(|r| matches!(r, OcrReason::EmbeddedImage)));
    }

    #[test]
    fn test_detect_all_pages_mode() {
        let pages = vec![make_page(
            1,
            "文字足够长的页面内容用于测试检测功能",
            0.0,
            0,
            false,
        )];
        let result = detect_ocr_pages(
            &pages,
            50,
            0.08,
            1,
            0.3,
            &crate::kb::ocr_provider::OcrMode::AllPages,
        );
        assert!(result.needs_ocr);
        assert!(result.reasons_by_page[&1]
            .iter()
            .any(|r| matches!(r, OcrReason::ForcedAllPages)));
    }

    #[test]
    fn test_detect_vector_objects() {
        let pages = vec![make_page(1, "有文字但有矢量对象", 0.0, 0, true)];
        let result = detect_ocr_pages(
            &pages,
            50,
            0.08,
            1,
            0.3,
            &crate::kb::ocr_provider::OcrMode::Auto,
        );
        assert!(result.needs_ocr);
        assert!(result.uncertain_pages.contains(&1));
    }
}
