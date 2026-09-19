use reqwest::Client;
use tracing::{error, info};

use crate::models::{CreateMessagePayload, MessageReference};
use crate::vault::get_vault_secret;

pub struct GideonBot {
    pub client: Client,
    pub token: String,
}

impl GideonBot {
    pub fn new(token: String) -> Self {
        Self {
            client: Client::builder().build().unwrap_or_default(),
            token,
        }
    }

    pub async fn handle_command(&self, channel_id: &str, message_id: &str, command: &str, author_name: &str) {
        info!("[GideonBot] Processing command from {}: {}", author_name, command);

        let reply_text = match command {
            "!gideon status" | "!status" => {
                let torn_key_status = if get_vault_secret("TORN_API_KEY").is_some() { "ARMED (TPM)" } else { "MISSING" };
                format!("G.I.D.E.O.N. Sovereign Core: ONLINE\nArchitecture: Native Rust (GB10 SparkOS)\nTorn API Vault: {}\nRoles: Deterministic Ledger Active", torn_key_status)
            }
            "!verify" | "!link" => {
                "To verify your Torn membership, use the native verification modal or link your Torn ID directly with Gideon.".to_string()
            }
            "!help" => {
                "G.I.D.E.O.N. Commands:\n-  - Check sovereign bot and vault status\n-  - Torn membership verification\n-  - Command listing".to_string()
            }
            _ => {
                format!("G.I.D.E.O.N. command not recognized: {}. Try .", command)
            }
        };

        let discord_url = format!("https://discord.com/api/v10/channels/{}/messages", channel_id);
        let payload = CreateMessagePayload {
            content: reply_text,
            message_reference: Some(MessageReference {
                message_id: message_id.to_string(),
            }),
        };

        if let Err(e) = self.client.post(&discord_url)
            .header("Authorization", format!("Bot {}", self.token))
            .json(&payload)
            .send()
            .await
        {
            error!("[GideonBot] Failed to send Discord reply: {}", e);
        } else {
            info!("[GideonBot] Replied to {} in channel {}", author_name, channel_id);
        }
    }
}
