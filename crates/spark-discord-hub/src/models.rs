#[allow(dead_code)]
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct DiscordUser {
    pub id: String,
    pub username: String,
    pub bot: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiscordMessage {
    pub id: String,
    pub channel_id: String,
    pub content: String,
    pub author: DiscordUser,
    pub guild_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateMessagePayload {
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_reference: Option<MessageReference>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MessageReference {
    pub message_id: String,
}
