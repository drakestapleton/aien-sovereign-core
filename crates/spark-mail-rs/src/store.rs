use crate::models::{EmailMessage, MailboxStatus};
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct MailStore {
    pub base_path: PathBuf,
}

impl MailStore {
    pub fn new(custom_path: Option<PathBuf>) -> Self {
        let base_path = custom_path.unwrap_or_else(|| {
            dirs_or_home().join(".local/share/sovereign/mail")
        });

        let store = Self { base_path };
        store.ensure_dirs();
        store
    }

    fn ensure_dirs(&self) {
        let folders = ["inbox", "sent", "archive", "drafts"];
        for folder in &folders {
            let p = self.base_path.join(folder);
            let _ = fs::create_dir_all(&p);
        }
    }

    pub fn save_message(&self, message: &EmailMessage) -> Result<(), String> {
        self.ensure_dirs();
        let folder_path = self.base_path.join(&message.folder);
        if !folder_path.exists() {
            let _ = fs::create_dir_all(&folder_path);
        }
        let file_path = folder_path.join(format!("{}.json", message.id));
        let data = serde_json::to_string_pretty(message)
            .map_err(|e| format!("Failed to serialize email: {}", e))?;
        fs::write(&file_path, data)
            .map_err(|e| format!("Failed to write email to disk: {}", e))?;
        Ok(())
    }

    pub fn get_message(&self, id: &str) -> Option<EmailMessage> {
        let folders = ["inbox", "sent", "archive", "drafts"];
        for folder in &folders {
            let file_path = self.base_path.join(folder).join(format!("{}.json", id));
            if file_path.exists() {
                if let Ok(data) = fs::read_to_string(&file_path) {
                    if let Ok(msg) = serde_json::from_str::<EmailMessage>(&data) {
                        return Some(msg);
                    }
                }
            }
        }
        None
    }

    pub fn list_folder(&self, folder: &str) -> Vec<EmailMessage> {
        let folder_path = self.base_path.join(folder);
        if !folder_path.exists() {
            return Vec::new();
        }

        let mut messages = Vec::new();
        if let Ok(entries) = fs::read_dir(&folder_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("json") {
                    if let Ok(data) = fs::read_to_string(&path) {
                        if let Ok(msg) = serde_json::from_str::<EmailMessage>(&data) {
                            messages.push(msg);
                        }
                    }
                }
            }
        }

        // Sort descending by received_at
        messages.sort_by(|a, b| b.received_at.cmp(&a.received_at));
        messages
    }

    pub fn count_folder(&self, folder: &str) -> usize {
        let folder_path = self.base_path.join(folder);
        if let Ok(entries) = fs::read_dir(&folder_path) {
            entries.flatten().filter(|e| {
                e.path().extension().and_then(|s| s.to_str()) == Some("json")
            }).count()
        } else {
            0
        }
    }

    pub fn get_status(
        &self,
        operator_email: &str,
        smtp_port: u16,
        api_port: u16,
        cortex_ok: bool,
        bridge_ok: bool,
    ) -> MailboxStatus {
        MailboxStatus {
            inbox_count: self.count_folder("inbox"),
            sent_count: self.count_folder("sent"),
            archive_count: self.count_folder("archive"),
            drafts_count: self.count_folder("drafts"),
            storage_path: self.base_path.to_string_lossy().to_string(),
            smtp_port,
            api_port,
            cortex_connected: cortex_ok,
            proton_bridge_detected: bridge_ok,
            operator_email: operator_email.to_string(),
        }
    }
}

fn dirs_or_home() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home)
    } else {
        PathBuf::from("/home/drakestapleton")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::EmailMessage;
    use std::collections::HashMap;

    #[test]
    fn test_mail_store_roundtrip() {
        let temp_dir = std::env::temp_dir().join(format!("test_mail_{}", uuid::Uuid::new_v4()));
        let store = MailStore::new(Some(temp_dir.clone()));

        let msg = EmailMessage {
            id: "test-msg-1".to_string(),
            from: "alice@test.local".to_string(),
            to: vec!["bob@test.local".to_string()],
            subject: "Unit Test Subject".to_string(),
            body: "Unit test body content.".to_string(),
            headers: HashMap::new(),
            received_at: "2026-09-18T17:00:00Z".to_string(),
            folder: "inbox".to_string(),
            cortex_indexed: false,
        };

        store.save_message(&msg).expect("save should succeed");
        let retrieved = store.get_message("test-msg-1").expect("message should be found");
        assert_eq!(retrieved.subject, "Unit Test Subject");
        assert_eq!(retrieved.from, "alice@test.local");

        let list = store.list_folder("inbox");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "test-msg-1");

        let status = store.get_status("bob@test.local", 2525, 18092, true, false);
        assert_eq!(status.inbox_count, 1);
        assert_eq!(status.sent_count, 0);

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
