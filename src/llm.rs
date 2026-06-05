//! Helpers for OpenAI-compatible chat completion requests.

/// DeepSeek V4 enables thinking by default. Most gbrain call sites expect the
/// final answer in `choices[0].message.content`, so disable thinking unless a
/// caller explicitly builds a reasoning request.
pub fn is_deepseek_chat_api(base_url: &str, model: &str) -> bool {
    base_url.to_ascii_lowercase().contains("api.deepseek.com")
        || model.to_ascii_lowercase().starts_with("deepseek-")
}

pub fn apply_deepseek_chat_options(body: &mut serde_json::Value, base_url: &str, model: &str) {
    if !is_deepseek_chat_api(base_url, model) {
        return;
    }

    if let Some(obj) = body.as_object_mut() {
        obj.insert(
            "thinking".to_string(),
            serde_json::json!({ "type": "disabled" }),
        );
    }
}

pub fn terminal_finish_reason(data: &serde_json::Value) -> Option<&str> {
    let reason = data
        .get("choices")?
        .as_array()?
        .first()?
        .get("finish_reason")?
        .as_str()?;

    match reason {
        "length" | "content_filter" | "insufficient_system_resource" => Some(reason),
        _ => None,
    }
}
