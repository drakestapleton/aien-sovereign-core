use crate::cortex_sync::CortexSync;
use crate::models::EmailMessage;
use crate::store::MailStore;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use uuid::Uuid;

pub async fn run_smtp_server(
    bind_addr: &str,
    store: Arc<MailStore>,
    cortex: Arc<CortexSync>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let listener = TcpListener::bind(bind_addr).await?;
    tracing::info!("📬 Sovereign SMTP Listener bound on {}", bind_addr);

    loop {
        let (socket, peer_addr) = match listener.accept().await {
            Ok(res) => res,
            Err(e) => {
                tracing::warn!("Failed to accept SMTP connection: {}", e);
                continue;
            }
        };

        let store_clone = Arc::clone(&store);
        let cortex_clone = Arc::clone(&cortex);

        tokio::spawn(async move {
            if let Err(e) = handle_smtp_session(socket, store_clone, cortex_clone).await {
                tracing::debug!("SMTP session with {} closed with: {}", peer_addr, e);
            }
        });
    }
}

async fn handle_smtp_session(
    stream: tokio::net::TcpStream,
    store: Arc<MailStore>,
    cortex: Arc<CortexSync>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);

    // Initial 220 banner
    writer.write_all(b"220 sovereign.spark ESMTP Spark-Mail 1.0 Ready\r\n").await?;

    let mut sender = String::new();
    let mut recipients: Vec<String> = Vec::new();
    let mut line = String::new();

    loop {
        line.clear();
        let bytes_read = buf_reader.read_line(&mut line).await?;
        if bytes_read == 0 {
            break;
        }

        let trimmed = line.trim();
        let upper = trimmed.to_uppercase();

        if upper.starts_with("HELO") || upper.starts_with("EHLO") {
            writer.write_all(b"250-sovereign.spark ESMTP\r\n250 8BITMIME\r\n").await?;
        } else if upper.starts_with("MAIL FROM:") {
            let s = trimmed["MAIL FROM:".len()..].trim();
            sender = s.trim_matches(|c| c == '<' || c == '>').to_string();
            writer.write_all(b"250 2.1.0 Ok\r\n").await?;
        } else if upper.starts_with("RCPT TO:") {
            let r = trimmed["RCPT TO:".len()..].trim();
            let cleaned = r.trim_matches(|c| c == '<' || c == '>').to_string();
            recipients.push(cleaned);
            writer.write_all(b"250 2.1.5 Ok\r\n").await?;
        } else if upper == "DATA" {
            writer.write_all(b"354 End data with <CR><LF>.<CR><LF>\r\n").await?;
            let mut raw_data = String::new();

            loop {
                let mut data_line = String::new();
                let n = buf_reader.read_line(&mut data_line).await?;
                if n == 0 {
                    break;
                }
                if data_line.trim() == "." {
                    break;
                }
                raw_data.push_str(&data_line);
            }

            // Parse simple email
            let id = Uuid::new_v4().to_string();
            let now = Utc::now().to_rfc3339();
            let (subject, body, headers) = parse_raw_email(&raw_data);

            let from_addr = if sender.is_empty() {
                headers.get("from").cloned().unwrap_or_else(|| "unknown@spark".to_string())
            } else {
                sender.clone()
            };

            let to_addrs = if recipients.is_empty() {
                headers.get("to").map(|t| vec![t.clone()]).unwrap_or_default()
            } else {
                recipients.clone()
            };

            let mut msg = EmailMessage {
                id: id.clone(),
                from: from_addr,
                to: to_addrs,
                subject: if subject.is_empty() { "(No Subject)".to_string() } else { subject },
                body,
                headers,
                received_at: now,
                folder: "inbox".to_string(),
                cortex_indexed: false,
            };

            // Save to disk
            let _ = store.save_message(&msg);

            // Async commit to Cortex memory
            if cortex.commit_mail(&msg).await.is_ok() {
                msg.cortex_indexed = true;
                let _ = store.save_message(&msg);
            }

            let resp = format!("250 2.0.0 Ok: queued as {}\r\n", id);
            writer.write_all(resp.as_bytes()).await?;

            // Reset state
            sender.clear();
            recipients.clear();
        } else if upper == "RSET" {
            sender.clear();
            recipients.clear();
            writer.write_all(b"250 2.0.0 Ok\r\n").await?;
        } else if upper == "NOOP" {
            writer.write_all(b"250 2.0.0 Ok\r\n").await?;
        } else if upper == "QUIT" {
            writer.write_all(b"221 2.0.0 Bye\r\n").await?;
            break;
        } else {
            writer.write_all(b"500 5.5.1 Command unrecognized\r\n").await?;
        }
    }

    Ok(())
}

fn parse_raw_email(raw: &str) -> (String, String, HashMap<String, String>) {
    let mut headers = HashMap::new();
    let mut subject = String::new();
    let mut in_headers = true;
    let mut body = String::new();

    for line in raw.lines() {
        if in_headers {
            if line.is_empty() {
                in_headers = false;
                continue;
            }
            if let Some((k, v)) = line.split_once(':') {
                let key = k.trim().to_lowercase();
                let val = v.trim().to_string();
                if key == "subject" {
                    subject = val.clone();
                }
                headers.insert(key, val);
            }
        } else {
            body.push_str(line);
            body.push('\n');
        }
    }

    (subject, body.trim().to_string(), headers)
}
