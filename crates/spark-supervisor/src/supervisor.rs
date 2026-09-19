use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, Mutex};
use tokio::time::sleep;
use tracing::{error, info, warn};

use crate::config::{ServiceConfig, SupervisorConfig};
use crate::proc_telemetry::get_process_stats;

#[derive(Debug, Clone)]
pub struct ManagedServiceState {
    pub name: String,
    pub config: ServiceConfig,
    pub pid: Option<u32>,
    pub restart_count: u32,
    pub is_healthy: bool,
    pub uptime_secs: u64,
    pub rss_mb: f64,
}

pub struct Supervisor {
    config: SupervisorConfig,
    states: Arc<Mutex<HashMap<String, ManagedServiceState>>>,
}

impl Supervisor {
    pub fn new(config: SupervisorConfig) -> Self {
        let mut states = HashMap::new();
        for (name, s_cfg) in &config.services {
            states.insert(
                name.clone(),
                ManagedServiceState {
                    name: name.clone(),
                    config: s_cfg.clone(),
                    pid: None,
                    restart_count: 0,
                    is_healthy: false,
                    uptime_secs: 0,
                    rss_mb: 0.0,
                },
            );
        }
        Self {
            config,
            states: Arc::new(Mutex::new(states)),
        }
    }

    pub async fn run_daemon(&self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Starting SparkOS Sovereign Supervisor with {} service(s)...", self.config.services.len());

        let (shutdown_tx, _) = broadcast::channel::<()>(1);

        for (name, s_cfg) in self.config.services.clone() {
            let states = self.states.clone();
            let mut shutdown_rx = shutdown_tx.subscribe();
            tokio::spawn(async move {
                Self::supervise_loop(name, s_cfg, states, &mut shutdown_rx).await;
            });
        }

        let states = self.states.clone();
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_millis(1500))
            .build()
            .unwrap_or_default();

        let mut shutdown_rx = shutdown_tx.subscribe();

        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    info!("Received termination signal. Shutting down all managed services gracefully...");
                    let _ = shutdown_tx.send(());
                    // Allow child processes brief grace period to terminate
                    sleep(Duration::from_millis(1000)).await;
                    break;
                }
                _ = shutdown_rx.recv() => {
                    break;
                }
                _ = sleep(Duration::from_secs(3)) => {
                    let mut map = states.lock().await;
                    for (_name, state) in map.iter_mut() {
                        if let Some(pid) = state.pid {
                            let p_stats = get_process_stats(pid);
                            if p_stats.is_alive {
                                state.rss_mb = p_stats.rss_bytes as f64 / (1024.0 * 1024.0);
                                state.uptime_secs += 3;
                            } else {
                                state.is_healthy = false;
                            }

                            if let Some(ref h_url) = state.config.health_url {
                                if let Ok(res) = http_client.get(h_url).send().await {
                                    state.is_healthy = res.status().is_success();
                                } else {
                                    state.is_healthy = false;
                                }
                            } else {
                                state.is_healthy = p_stats.is_alive;
                            }
                        }
                    }
                }
            }
        }

        info!("SparkOS Sovereign Supervisor halted cleanly.");
        Ok(())
    }

    async fn supervise_loop(
        name: String,
        config: ServiceConfig,
        states: Arc<Mutex<HashMap<String, ManagedServiceState>>>,
        shutdown_rx: &mut broadcast::Receiver<()>,
    ) {
        loop {
            info!("[{}] Spawning process: {} {:?}...", name, config.command, config.args);
            let mut cmd = tokio::process::Command::new(&config.command);
            cmd.args(&config.args);
            for (k, v) in &config.env {
                cmd.env(k, v);
            }

            match cmd.spawn() {
                Ok(mut child) => {
                    let pid = child.id();
                    {
                        let mut map = states.lock().await;
                        if let Some(st) = map.get_mut(&name) {
                            st.pid = pid;
                            st.uptime_secs = 0;
                            st.is_healthy = true;
                        }
                    }
                    info!("[{}] Running with PID {:?}", name, pid);

                    tokio::select! {
                        _ = shutdown_rx.recv() => {
                            info!("[{}] Terminating child PID {:?} for supervisor shutdown...", name, pid);
                            let _ = child.kill().await;
                            let _ = child.wait().await;
                            let mut map = states.lock().await;
                            if let Some(st) = map.get_mut(&name) {
                                st.pid = None;
                                st.is_healthy = false;
                            }
                            return;
                        }
                        status_res = child.wait() => {
                            match status_res {
                                Ok(status) => {
                                    warn!("[{}] Process exited with status: {}", name, status);
                                }
                                Err(e) => {
                                    error!("[{}] Process error: {}", name, e);
                                }
                            }
                        }
                    }

                    {
                        let mut map = states.lock().await;
                        if let Some(st) = map.get_mut(&name) {
                            st.pid = None;
                            st.is_healthy = false;
                            st.restart_count += 1;
                        }
                    }

                    if config.restart == "no" {
                        info!("[{}] Configured restart is no. Halting supervision.", name);
                        break;
                    }

                    info!("[{}] Restarting in 2 seconds...", name);
                    sleep(Duration::from_secs(2)).await;
                }
                Err(e) => {
                    error!("[{}] Failed to spawn: {}. Retrying in 5 seconds...", name, e);
                    sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }

    pub async fn print_status(&self) {
        let map = self.states.lock().await;
        println!("");
        println!("=== SparkOS Sovereign Supervisor Services ===");
        println!("{:<18} {:<10} {:<10} {:<12} {:<10} {:<10}", "SERVICE", "STATUS", "PID", "MEMORY", "RESTARTS", "HEALTH");
        println!("{}", "-".repeat(74));

        for (name, st) in map.iter() {
            let pid_str = st.pid.map(|p| p.to_string()).unwrap_or_else(|| "-".into());
            let status_str = if st.pid.is_some() { "RUNNING" } else { "STOPPED" };
            let mem_str = format!("{:.1} MB", st.rss_mb);
            let health_str = if st.is_healthy { "OK" } else { "UNHEALTHY" };

            println!("{:<18} {:<10} {:<10} {:<12} {:<10} {:<10}",
                name, status_str, pid_str, mem_str, st.restart_count, health_str);
        }
        println!("{}", "-".repeat(74));
        println!("");
    }
}
