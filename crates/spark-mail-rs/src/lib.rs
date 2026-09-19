pub mod api;
pub mod cortex_sync;
pub mod models;
pub mod relay;
pub mod smtp_server;
pub mod store;

pub use models::{EmailMessage, MailboxStatus, SendEmailRequest};
pub use store::MailStore;
pub use cortex_sync::CortexSync;
pub use relay::MailRelay;
