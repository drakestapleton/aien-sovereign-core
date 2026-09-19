use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::sleep;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tracing::{error, info, warn};

use crate::atlas_bot::AtlasBot;
use crate::gideon_bot::GideonBot;
use crate::models::DiscordMessage;

pub struct DiscordGatewayHub {
    token: String,
    atlas_bot: Arc<AtlasBot>,
    gideon_bot: Arc<GideonBot>,
    bot_user_id: Arc<tokio::sync::Mutex<Option<String>>>,
}

impl DiscordGatewayHub {
    pub fn new(token: String) -> Self {
        Self {
            atlas_bot: Arc::new(AtlasBot::new(token.clone())),
            gideon_bot: Arc::new(GideonBot::new(token.clone())),
            token,
            bot_user_id: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let gateway_url = "wss://gateway.discord.gg/?v=10&encoding=json";
        info!("[GatewayHub] Connecting to Discord Gateway at {}...", gateway_url);

        let mut backoff_secs = 5;

        loop {
            match connect_async(gateway_url).await {
                Ok((ws_stream, _)) => {
                    info!("[GatewayHub] Connected to Discord Gateway WebSocket.");
                    backoff_secs = 5;

                    let (mut write, mut read) = ws_stream.split();
                    let (outbound_tx, mut outbound_rx) = mpsc::channel::<WsMessage>(64);

                    // Writer task: forwards outbound messages to the WebSocket sink
                    let writer_handle = tokio::spawn(async move {
                        while let Some(msg) = outbound_rx.recv().await {
                            if let Err(e) = write.send(msg).await {
                                warn!("[GatewayHub] WebSocket send failed: {}", e);
                                break;
                            }
                        }
                    });

                    let last_seq = Arc::new(AtomicI64::new(-1));
                    let mut heartbeat_handle: Option<tokio::task::JoinHandle<()>> = None;

                    while let Some(msg_result) = read.next().await {
                        match msg_result {
                            Ok(WsMessage::Text(text)) => {
                                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                                    let op = v.get("op").and_then(|o| o.as_i64()).unwrap_or(-1);

                                    // Track sequence number
                                    if let Some(s) = v.get("s").and_then(|s| s.as_i64()) {
                                        last_seq.store(s, Ordering::Relaxed);
                                    }

                                    // Opcode 10: Hello
                                    if op == 10 {
                                        if let Some(hb_ms) = v.pointer("/d/heartbeat_interval").and_then(|h| h.as_u64()) {
                                            info!("[GatewayHub] Received Hello. Heartbeat interval: {}ms", hb_ms);

                                            // 1. Identify immediately (Intents: Guilds 1 + Guild Messages 512 + Message Content 32768 = 33281)
                                            let identify = json!({
                                                "op": 2,
                                                "d": {
                                                    "token": self.token,
                                                    "intents": 33281,
                                                    "properties": {
                                                        "os": "linux",
                                                        "browser": "spark-discord-hub",
                                                        "device": "spark-gb10"
                                                    }
                                                }
                                            });
                                            let _ = outbound_tx.send(WsMessage::Text(identify.to_string())).await;

                                            // 2. Spawn periodic heartbeat task
                                            if let Some(h) = heartbeat_handle.take() {
                                                h.abort();
                                            }
                                            let hb_tx = outbound_tx.clone();
                                            let seq_ref = last_seq.clone();
                                            heartbeat_handle = Some(tokio::spawn(async move {
                                                let mut interval = tokio::time::interval(Duration::from_millis(hb_ms));
                                                interval.tick().await; // first tick finishes immediately
                                                loop {
                                                    interval.tick().await;
                                                    let s = seq_ref.load(Ordering::Relaxed);
                                                    let hb_payload = if s >= 0 {
                                                        json!({"op": 1, "d": s})
                                                    } else {
                                                        json!({"op": 1, "d": null})
                                                    };
                                                    if hb_tx.send(WsMessage::Text(hb_payload.to_string())).await.is_err() {
                                                        break;
                                                    }
                                                }
                                            }));
                                        }
                                    }

                                    // Opcode 1: Heartbeat requested by Discord server
                                    if op == 1 {
                                        let s = last_seq.load(Ordering::Relaxed);
                                        let hb_reply = if s >= 0 {
                                            json!({"op": 1, "d": s})
                                        } else {
                                            json!({"op": 1, "d": null})
                                        };
                                        let _ = outbound_tx.send(WsMessage::Text(hb_reply.to_string())).await;
                                    }

                                    // Opcode 7: Reconnect request from server
                                    if op == 7 {
                                        warn!("[GatewayHub] Discord requested reconnect (Opcode 7).");
                                        break;
                                    }

                                    // Opcode 9: Invalid session
                                    if op == 9 {
                                        warn!("[GatewayHub] Discord invalid session (Opcode 9).");
                                        break;
                                    }

                                    // Opcode 0: Dispatch event
                                    if op == 0 {
                                        let event_name = v.get("t").and_then(|t| t.as_str()).unwrap_or("");
                                        if event_name == "READY" {
                                            if let Some(id) = v.pointer("/d/user/id").and_then(|i| i.as_str()) {
                                                *self.bot_user_id.lock().await = Some(id.to_string());
                                                info!("[GatewayHub] Gateway Ready. Authenticated as bot ID: {}", id);
                                            }
                                        } else if event_name == "MESSAGE_CREATE" {
                                            if let Some(d) = v.get("d") {
                                                if let Ok(msg) = serde_json::from_value::<DiscordMessage>(d.clone()) {
                                                    self.route_message(msg).await;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            Ok(WsMessage::Close(frame)) => {
                                warn!("[GatewayHub] WebSocket closed by Discord: {:?}", frame);
                                break;
                            }
                            Err(e) => {
                                error!("[GatewayHub] WebSocket read error: {}", e);
                                break;
                            }
                            _ => {}
                        }
                    }

                    if let Some(h) = heartbeat_handle {
                        h.abort();
                    }
                    writer_handle.abort();
                }
                Err(e) => {
                    error!("[GatewayHub] Failed to connect: {}. Retrying in {}s...", e, backoff_secs);
                }
            }

            sleep(Duration::from_secs(backoff_secs)).await;
            backoff_secs = (backoff_secs * 2).min(60);
        }
    }

    async fn route_message(&self, msg: DiscordMessage) {
        if msg.author.bot.unwrap_or(false) {
            return;
        }

        let my_id = self.bot_user_id.lock().await.clone().unwrap_or_default();
        let content = msg.content.trim();

        // 1. Gideon bot commands
        if content.starts_with("!gideon") || content.starts_with("!verify") || content.starts_with("!link") {
            let g = self.gideon_bot.clone();
            let c_id = msg.channel_id.clone();
            let m_id = msg.id.clone();
            let text = content.to_string();
            let author = msg.author.username.clone();
            tokio::spawn(async move {
                g.handle_command(&c_id, &m_id, &text, &author).await;
            });
            return;
        }

        // 2. Atlas bot inquiries
        let is_atlas_mention = !my_id.is_empty() && content.contains(&format!("<@{}>", my_id));
        let is_atlas_command = content.starts_with("!atlas") || content.starts_with("!ask");

        if is_atlas_mention || is_atlas_command {
            let clean_prompt = if is_atlas_command {
                content.trim_start_matches("!atlas").trim_start_matches("!ask").trim().to_string()
            } else {
                content.replace(&format!("<@{}>", my_id), "").trim().to_string()
            };

            if clean_prompt.is_empty() {
                return;
            }

            let a = self.atlas_bot.clone();
            let c_id = msg.channel_id.clone();
            let m_id = msg.id.clone();
            let prompt = clean_prompt.to_string();
            let author = msg.author.username.clone();
            tokio::spawn(async move {
                a.handle_message(&c_id, &m_id, &prompt, &author).await;
            });
        }
    }
}
