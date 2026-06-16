//! Client side of the AI feature: relays chat to the AsBar AI backend.
//!
//! The client never holds a provider key — it only knows the relay URL. The
//! relay picks a random key from its obfuscated store and calls the upstream.

use serde::{Deserialize, Serialize};

/// Default relay endpoint. Overridable via the `ASBAR_AI_URL` env var.
const DEFAULT_RELAY: &str = "http://de1.wildex.xyz:25573";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<&'a str>,
    messages: &'a [ChatMessage],
}

#[derive(Deserialize)]
struct ChatResponse {
    text: String,
    #[allow(dead_code)]
    model: String,
    #[allow(dead_code)]
    provider: String,
}

fn relay_url() -> String {
    std::env::var("ASBAR_AI_URL").unwrap_or_else(|_| DEFAULT_RELAY.to_string())
}

/// Models the client is allowed to request (must match the relay's routing).
pub fn allowed_models() -> Vec<&'static str> {
    vec!["gemini-2.5-flash", "gemini-2.5-flash-lite", "qwen/qwen3-32b"]
}

/// Send a conversation to the relay and return the assistant's reply text.
#[tauri::command]
pub async fn ai_chat(
    model: String,
    system: Option<String>,
    messages: Vec<ChatMessage>,
) -> Result<String, String> {
    if !allowed_models().contains(&model.as_str()) {
        return Err(format!("model \"{model}\" is not allowed"));
    }

    let body = ChatRequest {
        model: &model,
        system: system.as_deref(),
        messages: &messages,
    };

    // Generous timeout: the relay may run a multi-step tool loop (web search,
    // page fetches, GitHub reads) before the model produces its final answer.
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .map_err(|e| e.to_string())?;

    let url = format!("{}/v1/chat", relay_url().trim_end_matches('/'));
    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("could not reach the AI server: {e}"))?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();

    if !status.is_success() {
        // Relay errors come back as plain text; surface them to the UI.
        let msg = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(String::from))
            .unwrap_or(text);
        return Err(format!("AI server error ({status}): {msg}"));
    }

    let parsed: ChatResponse =
        serde_json::from_str(&text).map_err(|e| format!("bad server response: {e}"))?;
    Ok(parsed.text)
}

/// Expose the model list to the frontend.
#[tauri::command]
pub fn ai_models() -> Vec<String> {
    allowed_models().into_iter().map(String::from).collect()
}
