//! OCR 可选模块 (P2-017~P2-019)
//!
//! 为扫描型 PDF 提供 OCR job schema 和状态管理。

use serde::{Deserialize, Serialize};

/// OCR processing status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OcrStatus {
    NotNeeded,
    Needed,
    Queued,
    Processing,
    Done,
    Failed,
}

impl OcrStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NotNeeded => "not_needed",
            Self::Needed => "needed",
            Self::Queued => "queued",
            Self::Processing => "processing",
            Self::Done => "done",
            Self::Failed => "failed",
        }
    }
}

/// OCR 是否可通过配置启用
pub fn is_ocr_enabled(external_ocr_allowed: bool, global_ocr_enabled: bool) -> bool {
    external_ocr_allowed && global_ocr_enabled
}

/// 判断是否需要 OCR（基于文本密度）
pub fn needs_ocr(text_density: f64, threshold: f64) -> bool {
    text_density < threshold
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ocr_disabled() {
        assert!(!is_ocr_enabled(false, true));
        assert!(!is_ocr_enabled(true, false));
        assert!(is_ocr_enabled(true, true));
    }

    #[test]
    fn test_needs_ocr() {
        assert!(needs_ocr(0.01, 0.05));
        assert!(!needs_ocr(0.1, 0.05));
    }
}
