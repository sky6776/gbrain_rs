//! 隐私与外部模型治理 (P5-019, P5-020)
//!
//! 每个 library 可控制是否调用外部模型，支持敏感信息脱敏。

use regex::Regex;
use std::sync::OnceLock;

/// Library 隐私策略（对齐 kb_libraries 中的 governance 字段）
#[derive(Debug, Clone)]
pub struct PrivacyPolicy {
    pub external_embedding_allowed: bool,
    pub external_rerank_allowed: bool,
    pub external_summary_allowed: bool,
    pub external_ocr_allowed: bool,
    pub redaction_enabled: bool,
}

impl Default for PrivacyPolicy {
    fn default() -> Self {
        Self {
            external_embedding_allowed: true,
            external_rerank_allowed: true,
            external_summary_allowed: true,
            external_ocr_allowed: true,
            redaction_enabled: false,
        }
    }
}

impl PrivacyPolicy {
    /// 是否允许任何外部模型调用
    pub fn any_external_allowed(&self) -> bool {
        self.external_embedding_allowed
            || self.external_rerank_allowed
            || self.external_summary_allowed
            || self.external_ocr_allowed
    }
}

/// P5-020: 对发送给外部模型的文本进行脱敏
pub fn redact_content(text: &str) -> String {
    let mut result = text.to_string();

    // 邮箱
    if let Ok(re) = email_regex() {
        result = re.replace_all(&result, "[EMAIL]").to_string();
    }

    // 手机号（中国）
    if let Ok(re) = phone_regex() {
        result = re.replace_all(&result, "[PHONE]").to_string();
    }

    // 身份证号
    if let Ok(re) = id_card_regex() {
        result = re.replace_all(&result, "[ID_NUMBER]").to_string();
    }

    // API key 模式（sk-..., pk-..., etc）
    if let Ok(re) = api_key_regex() {
        result = re.replace_all(&result, "[API_KEY]").to_string();
    }

    // 银行卡号（16-19位数字）
    if let Ok(re) = bank_card_regex() {
        result = re.replace_all(&result, "[BANK_CARD]").to_string();
    }

    result
}

fn email_regex() -> Result<&'static Regex, regex::Error> {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}").unwrap());
    Ok(RE.get().unwrap())
}

fn phone_regex() -> Result<&'static Regex, regex::Error> {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"1[3-9]\d{9}").unwrap());
    Ok(RE.get().unwrap())
}

fn id_card_regex() -> Result<&'static Regex, regex::Error> {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\d{17}[\dXx]").unwrap());
    Ok(RE.get().unwrap())
}

fn api_key_regex() -> Result<&'static Regex, regex::Error> {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\b(sk-[a-zA-Z0-9]{20,}|pk-[a-zA-Z0-9]{20,}|[a-zA-Z0-9]{32,})\b").unwrap()
    });
    Ok(RE.get().unwrap())
}

fn bank_card_regex() -> Result<&'static Regex, regex::Error> {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\b\d{16,19}\b").unwrap());
    Ok(RE.get().unwrap())
}

/// 记录外部模型调用（写入 kb_external_model_calls）
pub fn log_external_model_call(
    conn: &rusqlite::Connection,
    library_id: Option<i64>,
    document_id: Option<i64>,
    call_type: &str,
    provider: &str,
    model: &str,
    input_tokens: i32,
    output_tokens: i32,
    latency_ms: i32,
    cost_estimate: f64,
    success: bool,
    error_message: &str,
) -> Result<i64, rusqlite::Error> {
    conn.execute(
        "INSERT INTO kb_external_model_calls \
         (library_id, document_id, call_type, provider, model, \
          input_tokens, output_tokens, latency_ms, cost_estimate, success, error_message) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        rusqlite::params![
            library_id,
            document_id,
            call_type,
            provider,
            model,
            input_tokens,
            output_tokens,
            latency_ms,
            cost_estimate,
            success as i32,
            error_message,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_email() {
        let text = "Contact alice@example.com for support";
        let redacted = redact_content(text);
        assert!(!redacted.contains("alice@example.com"));
        assert!(redacted.contains("[EMAIL]"));
    }

    #[test]
    fn test_redact_phone() {
        let text = "Call 13800138000";
        let redacted = redact_content(text);
        assert!(!redacted.contains("13800138000"));
        assert!(redacted.contains("[PHONE]"));
    }

    #[test]
    fn test_redact_no_sensitive() {
        let text = "This is normal text without PII";
        let redacted = redact_content(text);
        assert_eq!(text, redacted);
    }

    #[test]
    fn test_privacy_policy_default() {
        let policy = PrivacyPolicy::default();
        assert!(policy.external_rerank_allowed);
        assert!(!policy.redaction_enabled);
    }

    #[test]
    fn test_redact_api_key() {
        let text = "Authorization: Bearer sk-1234567890abcdefghijklmnop";
        let redacted = redact_content(text);
        assert!(redacted.contains("[API_KEY]"));
    }
}
