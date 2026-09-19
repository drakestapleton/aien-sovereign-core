use reqwest::Client;
use serde_json::json;
use std::time::Duration;

pub const EMBEDDING_DIMENSIONS: usize = 768;

pub fn embedding_to_bytes(v: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(v.len() * 4);
    for &f in v {
        bytes.extend_from_slice(&f.to_le_bytes());
    }
    bytes
}

pub fn bytes_to_embedding(b: &[u8]) -> Vec<f32> {
    let mut v = Vec::with_capacity(b.len() / 4);
    for chunk in b.chunks_exact(4) {
        let arr: [u8; 4] = [chunk[0], chunk[1], chunk[2], chunk[3]];
        v.push(f32::from_le_bytes(arr));
    }
    v
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        norm_a += a[i] * a[i];
        norm_b += b[i] * b[i];
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom > 0.0 {
        dot / denom
    } else {
        0.0
    }
}

pub fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    1.0 - cosine_similarity(a, b)
}

pub async fn fetch_embedding(client: &Client, text: &str, encoder_url: &str) -> Result<Vec<f32>, String> {
    let payload = json!({
        "text": text,
        "modelId": "BAAI/bge-base-en-v1.5",
        "modelRevision": "cortex-bge-base-en-v1.5-768-v1",
        "priority": "recall"
    });

    let resp = client
        .post(format!("{}/embed", encoder_url.trim_end_matches("/")))
        .timeout(Duration::from_millis(1500))
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Failed to connect to embedding encoder on {}: {}", encoder_url, e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Encoder returned error HTTP {}: {}", status, body));
    }

    let val: serde_json::Value = resp.json().await
        .map_err(|e| format!("Failed to parse embedding response JSON: {}", e))?;

    if let Some(arr) = val.get("embedding").and_then(|v| v.as_array()) {
        let mut emb = Vec::with_capacity(arr.len());
        for num in arr {
            if let Some(f) = num.as_f64() {
                emb.push(f as f32);
            }
        }
        if emb.len() == EMBEDDING_DIMENSIONS {
            return Ok(emb);
        } else {
            return Err(format!("Expected {} dimensions, got {}", EMBEDDING_DIMENSIONS, emb.len()));
        }
    }

    Err("Invalid embedding payload structure from encoder service".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_bytes_roundtrip() {
        let orig = vec![0.123f32, -0.456, 0.789, 1.0];
        let bytes = embedding_to_bytes(&orig);
        assert_eq!(bytes.len(), 16);
        let recovered = bytes_to_embedding(&bytes);
        assert_eq!(orig, recovered);
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0f32, 0.0, 0.0];
        let b = vec![1.0f32, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-5);

        let c = vec![0.0f32, 1.0, 0.0];
        assert!((cosine_similarity(&a, &c) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn test_cosine_distance_and_empty_handling() {
        let a = vec![1.0f32, 0.0, 0.0];
        let b = vec![1.0f32, 0.0, 0.0];
        assert!((cosine_distance(&a, &b) - 0.0).abs() < 1e-5);

        let orthogonal = vec![0.0f32, 1.0, 0.0];
        assert!((cosine_distance(&a, &orthogonal) - 1.0).abs() < 1e-5);

        let opposite = vec![-1.0f32, 0.0, 0.0];
        assert!((cosine_distance(&a, &opposite) - 2.0).abs() < 1e-5);

        let empty: Vec<f32> = vec![];
        assert_eq!(cosine_similarity(&empty, &empty), 0.0);
        assert_eq!(cosine_distance(&empty, &empty), 1.0);

        let mismatched = vec![1.0f32, 0.0];
        assert_eq!(cosine_similarity(&a, &mismatched), 0.0);
        assert_eq!(cosine_distance(&a, &mismatched), 1.0);

        let zeros = vec![0.0f32, 0.0, 0.0];
        assert_eq!(cosine_similarity(&zeros, &zeros), 0.0);
        assert_eq!(cosine_distance(&zeros, &zeros), 1.0);
    }
}
