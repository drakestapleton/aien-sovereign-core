use crate::models::EmailMessage;
use reqwest::Client;
use serde_json::json;
use std::fs;
use std::path::Path;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct CortexSync {
    cortex_url: String,
    client: Client,
}

impl CortexSync {
    pub fn new(cortex_url: Option<&str>) -> Self {
        let url = cortex_url.unwrap_or("http://127.0.0.1:18080").to_string();
        let client = Client::builder()
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { cortex_url: url, client }
    }

    fn get_cortex_token(&self) -> String {
        let token_path = "/home/drakestapleton/.config/cortex/token";
        if Path::new(token_path).exists() {
            fs::read_to_string(token_path).unwrap_or_default().trim().to_string()
        } else {
            String::new()
        }
    }

    pub async fn check_health(&self) -> bool {
        self.client
            .get(format!("{}/health", self.cortex_url))
            .timeout(Duration::from_millis(500))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    pub async fn commit_mail(&self, msg: &EmailMessage) -> Result<(), String> {
        let token = self.get_cortex_token();
        let formatted_content = format!(
            "From: {}\nTo: {}\nSubject: {}\nDate: {}\nFolder: {}\n\n{}",
            msg.from,
            msg.to.join(", "),
            msg.subject,
            msg.received_at,
            msg.folder,
            msg.body
        );

        let payload = json!({
            "kind": "entity",
            "value": {
                "canonicalName": format!("mail:{}", msg.id),
                "entityType": "correspondence",
                "content": formatted_content,
                "metadata": {
                    "mail_id": msg.id,
                    "subject": msg.subject,
                    "from": msg.from,
                    "to": msg.to,
                    "folder": msg.folder,
                    "received_at": msg.received_at
                },
                "space": "atlas-memory"
            }
        });

        let mut req = self.client
            .post(format!("{}/api/cortex/write", self.cortex_url))
            .header("Content-Type", "application/json");

        if !token.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", token));
        }

        let resp = req.json(&payload)
            .send()
            .await
            .map_err(|e| format!("Cortex connection failed: {}", e))?;

        if resp.status().is_success() {
            Ok(())
        } else {
            Err(format!("Cortex write returned HTTP {}", resp.status()))
        }
    }
}
