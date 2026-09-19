use clap::{Parser, Subcommand};
use serde::Deserialize;
use spark_mail::api::{create_router, ApiState};
use spark_mail::cortex_sync::CortexSync;
use spark_mail::models::SendEmailRequest;
use spark_mail::relay::MailRelay;
use spark_mail::smtp_server::run_smtp_server;
use spark_mail::store::MailStore;
use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser, Debug)]
#[command(name = "spark-mail", about = "Sovereign Native Mail Engine & Atlas Cortex Bridge")]
struct Cli {
    #[arg(long, default_value = "127.0.0.1:2525")]
    smtp_addr: String,

    #[arg(long, default_value = "127.0.0.1:18092")]
    api_addr: String,

    #[arg(long)]
    mail_dir: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Run,
    Status,
    List {
        #[arg(default_value = "inbox")]
        folder: String,
    },
    Send {
        #[arg(long)]
        to: String,
        #[arg(long)]
        subject: String,
        #[arg(long)]
        body: String,
        #[arg(long)]
        from: Option<String>,
    },
    IngestTest,
}

#[derive(Deserialize, Default)]
struct OperatorConfig {
    operator: Option<OperatorSection>,
}

#[derive(Deserialize, Default)]
struct OperatorSection {
    email: Option<String>,
}

fn load_operator_email() -> String {
    let p = dirs_or_home().join(".config/sovereign/operator.toml");
    if p.exists() {
        if let Ok(content) = fs::read_to_string(&p) {
            if let Ok(cfg) = toml::from_str::<OperatorConfig>(&content) {
                if let Some(op) = cfg.operator {
                    if let Some(email) = op.email {
                        return email;
                    }
                }
            }
        }
    }
    "aien.atlas@proton.me".to_string()
}

fn dirs_or_home() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home)
    } else {
        PathBuf::from("/home/drakestapleton")
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cli = Cli::parse();
    let store = Arc::new(MailStore::new(cli.mail_dir));
    let cortex = Arc::new(CortexSync::new(None));
    let operator_email = load_operator_email();

    let relay = Arc::new(MailRelay::new(
        "127.0.0.1",
        1025,
        Arc::clone(&store),
        Arc::clone(&cortex),
        &operator_email,
    ));

    match cli.command.unwrap_or(Commands::Run) {
        Commands::Run => {
            println!("📬 AIEN Sovereign Mail Engine starting...");
            println!("   Local NVMe Storage: {}", store.base_path.display());
            println!("   SMTP Listener:      {}", cli.smtp_addr);
            println!("   API Gateway:        http://{}", cli.api_addr);
            println!("   Operator Identity:  {}", operator_email);

            let store_smtp = Arc::clone(&store);
            let cortex_smtp = Arc::clone(&cortex);
            let smtp_addr = cli.smtp_addr.clone();

            let smtp_handle = tokio::spawn(async move {
                if let Err(e) = run_smtp_server(&smtp_addr, store_smtp, cortex_smtp).await {
                    eprintln!("SMTP server error: {}", e);
                }
            });

            let api_port: u16 = cli.api_addr.split(':').last().and_then(|p| p.parse().ok()).unwrap_or(18092);
            let smtp_port: u16 = cli.smtp_addr.split(':').last().and_then(|p| p.parse().ok()).unwrap_or(2525);

            let api_state = ApiState {
                store: Arc::clone(&store),
                cortex: Arc::clone(&cortex),
                relay: Arc::clone(&relay),
                smtp_port,
                api_port,
                operator_email,
            };

            let app = create_router(api_state);
            let api_socket: SocketAddr = cli.api_addr.parse()?;
            let listener = tokio::net::TcpListener::bind(api_socket).await?;

            let api_handle = tokio::spawn(async move {
                let _ = axum::serve(listener, app).await;
            });

            tokio::select! {
                _ = smtp_handle => {},
                _ = api_handle => {},
            }
        }
        Commands::Status => {
            let cortex_ok = cortex.check_health().await;
            let bridge_ok = relay.check_bridge().await;
            let status = store.get_status(&operator_email, 2525, 18092, cortex_ok, bridge_ok);
            println!("{:#?}", status);
        }
        Commands::List { folder } => {
            let messages = store.list_folder(&folder);
            println!("Found {} messages in [{}]:", messages.len(), folder);
            for m in messages {
                println!("- [{}] {} | From: {} | Subject: {}", &m.id[..8], m.received_at, m.from, m.subject);
            }
        }
        Commands::Send { to, subject, body, from } => {
            let req = SendEmailRequest {
                to: vec![to],
                subject,
                body,
                from,
            };
            let sent = relay.send_email(req).await.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            println!("Email recorded and sent: {} (Cortex indexed: {})", sent.id, sent.cortex_indexed);
        }
        Commands::IngestTest => {
            let req = SendEmailRequest {
                to: vec![operator_email.clone()],
                subject: "Sovereign Mail Node Online".to_string(),
                body: "Your sovereign mail server is running directly on Spark hardware. Messages are stored on local NVMe disk with zero cloud telemetry and synced to Atlas Cortex memory.".to_string(),
                from: Some("atlas@sovereign.spark".to_string()),
            };
            let sent = relay.send_email(req).await.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            println!("Test message saved and indexed into Cortex: {}", sent.id);
        }
    }

    Ok(())
}
