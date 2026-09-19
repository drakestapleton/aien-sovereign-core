use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use reqwest::Client;
use serde_json::{json, Value};

use crate::models::{ChatMessage, NormalizedChunk};

pub fn build_headers(api_key: Option<&str>, custom_headers: Option<Vec<(String, String)>>) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    if let Some(key) = api_key {
        let auth_val = format!("Bearer {}", key.trim());
        if let Ok(hv) = HeaderValue::from_str(&auth_val) {
            headers.insert(AUTHORIZATION, hv);
        }
    }

    if let Some(custom) = custom_headers {
        for (k, v) in custom {
            if let (Ok(hk), Ok(hv)) = (
                reqwest::header::HeaderName::from_bytes(k.as_bytes()),
                HeaderValue::from_str(&v)
            ) {
                headers.insert(hk, hv);
            }
        }
    }

    headers
}

pub fn format_openai_payload(model: &str, messages: &[ChatMessage], temperature: Option<f32>, stream: bool) -> Value {
    let formatted_msgs: Vec<Value> = messages
        .iter()
        .map(|m| {
            json!({
                "role": m.role,
                "content": m.content
            })
        })
        .collect();

    json!({
        "model": model,
        "messages": formatted_msgs,
        "temperature": temperature.unwrap_or(0.7),
        "stream": stream
    })
}

pub fn parse_openai_sse_line(line: &str) -> Option<NormalizedChunk> {
    let trimmed = line.trim();
    if !trimmed.starts_with("data:") {
        return None;
    }

    let payload = trimmed.strip_prefix("data:")?.trim();
    if payload == "[DONE]" {
        return Some(NormalizedChunk {
            content: String::new(),
            reasoning: String::new(),
            is_done: true,
        });
    }

    let val: Value = serde_json::from_str(payload).ok()?;
    let choice = val.get("choices")?.as_array()?.first()?;
    let delta = choice.get("delta")?;

    let content = delta.get("content").and_then(Value::as_str).unwrap_or("").to_string();
    let reasoning = delta.get("reasoning_content")
        .or_else(|| delta.get("reasoning"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    Some(NormalizedChunk {
        content,
        reasoning,
        is_done: false,
    })
}

pub async fn call_openai_unary(
    client: &Client,
    endpoint: &str,
    model: &str,
    api_key: Option<&str>,
    messages: &[ChatMessage],
    temperature: Option<f32>,
) -> Result<(String, Option<String>, usize), String> {
    let payload = format_openai_payload(model, messages, temperature, false);
    let headers = build_headers(api_key, None);

    let res = client
        .post(endpoint)
        .headers(headers)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Failed to send request to {}: {}", endpoint, e))?;

    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        return Err(format!("Provider returned HTTP {}: {}", status, body));
    }

    let val: Value = res
        .json()
        .await
        .map_err(|e| format!("Failed to parse JSON response: {}", e))?;

    let choice = val
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .ok_or_else(|| "Malformed response: missing choices array".to_string())?;

    let message = choice
        .get("message")
        .ok_or_else(|| "Missing message object in choice".to_string())?;

    let content = message
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let reasoning = message
        .get("reasoning_content")
        .or_else(|| message.get("reasoning"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let token_count = val
        .get("usage")
        .and_then(|u| u.get("total_tokens"))
        .and_then(|t| t.as_u64())
        .unwrap_or(0) as usize;

    Ok((content, reasoning, token_count))
}
