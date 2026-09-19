use crate::cortex_sync::CortexSync;
use crate::models::{EmailMessage, SendEmailRequest};
use crate::store::MailStore;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct MailRelay {
    pub bridge_host: String,
    pub bridge_port: u16,
    pub store: Arc<MailStore>,
    pub cortex: Arc<CortexSync>,
    pub default_sender: String,
}

impl MailRelay {
    pub fn new(
        bridge_host: &str,
        bridge_port: u16,
        store: Arc<MailStore>,
        cortex: Arc<CortexSync>,
        default_sender: &str,
    ) -> Self {
        Self {
            bridge_host: bridge_host.to_string(),
            bridge_port,
            store,
            cortex,
            default_sender: default_sender.to_string(),
        }
    }

    pub async fn check_bridge(&self) -> bool {
        let addr = format!("{}:{}", self.bridge_host, self.bridge_port);
        tokio::time::timeout(Duration::from_millis(400), TcpStream::connect(&addr))
            .await
            .map(|r| r.is_ok())
            .unwrap_or(false)
    }

    pub async fn send_email(&self, req: SendEmailRequest) -> Result<EmailMessage, String> {
        let sender = req.from.unwrap_or_else(|| self.default_sender.clone());
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        let mut headers = HashMap::new();
        headers.insert("From".to_string(), sender.clone());
        headers.insert("To".to_string(), req.to.join(", "));
        headers.insert("Subject".to_string(), req.subject.clone());
        headers.insert("Date".to_string(), now.clone());
        headers.insert("Message-ID".to_string(), format!("<{}@sovereign.spark>", id));

        let mut msg = EmailMessage {
            id: id.clone(),
            from: sender.clone(),
            to: req.to.clone(),
            subject: req.subject.clone(),
            body: req.body.clone(),
            headers,
            received_at: now,
            folder: "sent".to_string(),
            cortex_indexed: false,
        };

        // Try relaying to Proton Bridge if available
        let bridge_online = self.check_bridge().await;
        if bridge_online {
            let addr = format!("{}:{}", self.bridge_host, self.bridge_port);
            if let Ok(mut stream) = TcpStream::connect(&addr).await {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf).await;
                let _ = stream.write_all(b"EHLO sovereign.spark\r\n").await;
                let _ = stream.read(&mut buf).await;
                let _ = stream.write_all(format!("MAIL FROM:<{}>\r\n", sender).as_bytes()).await;
                let _ = stream.read(&mut buf).await;
                for recipient in &req.to {
                    let _ = stream.write_all(format!("RCPT TO:<{}>\r\n", recipient).as_bytes()).await;
                    let _ = stream.read(&mut buf).await;
                }
                let _ = stream.write_all(b"DATA\r\n").await;
                let _ = stream.read(&mut buf).await;
                let email_payload = format!(
                    "From: {}\r\nTo: {}\r\nSubject: {}\r\nDate: {}\r\n\r\n{}\r\n.\r\n",
                    sender,
                    req.to.join(", "),
                    req.subject,
                    Utc::now().to_rfc2822(),
                    req.body
                );
                let _ = stream.write_all(email_payload.as_bytes()).await;
                let _ = stream.read(&mut buf).await;
                let _ = stream.write_all(b"QUIT\r\n").await;
            }
        }

        // Save sent message to local hardware storage
        self.store.save_message(&msg)?;

        // Index in Cortex memory
        if self.cortex.commit_mail(&msg).await.is_ok() {
            msg.cortex_indexed = true;
            let _ = self.store.save_message(&msg);
        }

        Ok(msg)
    }
}
