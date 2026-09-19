use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailMessage {
    pub id: String,
    pub from: String,
    pub to: Vec<String>,
    pub subject: String,
    pub body: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    pub received_at: String,
    pub folder: String,
    #[serde(default)]
    pub cortex_indexed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendEmailRequest {
    pub to: Vec<String>,
    pub subject: String,
    pub body: String,
    #[serde(default)]
    pub from: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailboxStatus {
    pub inbox_count: usize,
    pub sent_count: usize,
    pub archive_count: usize,
    pub drafts_count: usize,
    pub storage_path: String,
    pub smtp_port: u16,
    pub api_port: u16,
    pub cortex_connected: bool,
    pub proton_bridge_detected: bool,
    pub operator_email: String,
}
