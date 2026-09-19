use reqwest::Client;
use serde_json::{json, Value};
use std::process::Command;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("Network request failed: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("Vault key resolution failed for {0}: {1}")]
    VaultKey(String, String),
    #[error("API returned error: {0}")]
    Api(String),
}

pub struct HarvesterClient {
    http: Client,
}

impl Default for HarvesterClient {
    fn default() -> Self {
        Self::new()
    }
}

impl HarvesterClient {
    pub fn new() -> Self {
        Self {
            http: Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .unwrap(),
        }
    }

    pub fn resolve_vault_key(key_name: &str) -> Result<String, ClientError> {
        let output = Command::new("/home/drakestapleton/.local/bin/atlas-vault")
            .arg("get")
            .arg(key_name)
            .output();

        match output {
            Ok(out) if out.status.success() => {
                let secret = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if secret.is_empty() {
                    Err(ClientError::VaultKey(key_name.to_string(), "empty secret".to_string()))
                } else {
                    Ok(secret)
                }
            }
            Ok(out) => Err(ClientError::VaultKey(
                key_name.to_string(),
                String::from_utf8_lossy(&out.stderr).trim().to_string(),
            )),
            Err(e) => Err(ClientError::VaultKey(key_name.to_string(), e.to_string())),
        }
    }

    pub async fn query_openai_compat(
        &self,
        endpoint_url: &str,
        api_key: &str,
        model: &str,
        prompt: &str,
    ) -> Result<String, ClientError> {
        let payload = json!({
            "model": model,
            "messages": [{"role": "user", "content": prompt}],
            "temperature": 0.6
        });

        let res = self.http
            .post(endpoint_url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await?;

        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            return Err(ClientError::Api(format!("{}: {}", status, text)));
        }

        let body: Value = res.json().await?;
        let content = body["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();
        Ok(content)
    }
}
