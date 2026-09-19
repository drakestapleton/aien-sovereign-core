use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use reqwest::Client;
use serde_json::{json, Value};

use crate::models::{ChatMessage, NormalizedChunk};

pub fn build_anthropic_headers(api_key: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        reqwest::header::HeaderName::from_static("x-api-key"),
        HeaderValue::from_str(api_key.trim()).unwrap_or_else(|_| HeaderValue::from_static(""))
    );
    headers.insert(
        reqwest::header::HeaderName::from_static("anthropic-version"),
        HeaderValue::from_static("2023-06-01")
    );
    headers
}

pub fn format_anthropic_payload(model: &str, messages: &[ChatMessage], temperature: Option<f32>, stream: bool) -> Value {
    let mut system_prompt = String::new();
    let mut formatted_msgs = Vec::new();

    for m in messages {
        if m.role == "system" {
            if !system_prompt.is_empty() {
                system_prompt.push_str("

");
            }
            system_prompt.push_str(&m.content);
        } else {
            formatted_msgs.push(json!({
                "role": if m.role == "assistant" { "assistant" } else { "user" },
                "content": m.content
            }));
        }
    }

    if formatted_msgs.is_empty() {
        formatted_msgs.push(json!({
            "role": "user",
            "content": "Hello"
        }));
    }

    let mut payload = json!({
        "model": model,
        "messages": formatted_msgs,
        "max_tokens": 4096,
        "stream": stream,
        "temperature": temperature.unwrap_or(0.7)
    });

    if !system_prompt.is_empty() {
        payload["system"] = json!(system_prompt);
    }

    payload
}

pub fn parse_anthropic_sse_line(line: &str) -> Option<NormalizedChunk> {
    let trimmed = line.trim();
    if !trimmed.starts_with("data:") {
        return None;
    }

    let payload = trimmed.strip_prefix("data:")?.trim();
    let val: Value = serde_json::from_str(payload).ok()?;
    let event_type = val.get("type").and_then(Value::as_str).unwrap_or("");

    match event_type {
        "content_block_delta" => {
            let delta = val.get("delta")?;
            let delta_type = delta.get("type").and_then(Value::as_str).unwrap_or("");
            if delta_type == "text_delta" {
                let text = delta.get("text").and_then(Value::as_str).unwrap_or("").to_string();
                Some(NormalizedChunk {
                    content: text,
                    reasoning: String::new(),
                    is_done: false,
                })
            } else if delta_type == "thinking_delta" {
                let thinking = delta.get("thinking").and_then(Value::as_str).unwrap_or("").to_string();
                Some(NormalizedChunk {
                    content: String::new(),
                    reasoning: thinking,
                    is_done: false,
                })
            } else {
                None
            }
        }
        "message_stop" => Some(NormalizedChunk {
            content: String::new(),
            reasoning: String::new(),
            is_done: true,
        }),
        _ => None,
    }
}

pub async fn call_anthropic_unary(
    client: &Client,
    endpoint: &str,
    model: &str,
    api_key: &str,
    messages: &[ChatMessage],
    temperature: Option<f32>,
) -> Result<(String, Option<String>, usize), String> {
    let payload = format_anthropic_payload(model, messages, temperature, false);
    let headers = build_anthropic_headers(api_key);

    let res = client
        .post(endpoint)
        .headers(headers)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Failed to send request to Anthropic: {}", e))?;

    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        return Err(format!("Anthropic returned HTTP {}: {}", status, body));
    }

    let val: Value = res
        .json()
        .await
        .map_err(|e| format!("Failed to parse Anthropic JSON response: {}", e))?;

    let content_blocks = val
        .get("content")
        .and_then(|c| c.as_array())
        .ok_or_else(|| "Malformed Anthropic response: missing content array".to_string())?;

    let mut text_acc = String::new();
    let mut reasoning_acc = String::new();

    for block in content_blocks {
        let block_type = block.get("type").and_then(Value::as_str).unwrap_or("");
        if block_type == "text" {
            if let Some(t) = block.get("text").and_then(Value::as_str) {
                text_acc.push_str(t);
            }
        } else if block_type == "thinking" {
            if let Some(th) = block.get("thinking").and_then(Value::as_str) {
                reasoning_acc.push_str(th);
            }
        }
    }

    let input_tokens = val.get("usage").and_then(|u| u.get("input_tokens")).and_then(|t| t.as_u64()).unwrap_or(0);
    let output_tokens = val.get("usage").and_then(|u| u.get("output_tokens")).and_then(|t| t.as_u64()).unwrap_or(0);
    let total_tokens = (input_tokens + output_tokens) as usize;

    let reasoning = if reasoning_acc.is_empty() { None } else { Some(reasoning_acc) };

    Ok((text_acc, reasoning, total_tokens))
}
