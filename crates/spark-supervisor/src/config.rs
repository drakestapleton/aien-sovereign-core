use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct ServiceConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default = "default_restart")]
    pub restart: String,
    pub health_url: Option<String>,
}

fn default_restart() -> String {
    "always".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct SupervisorConfig {
    #[serde(default)]
    pub services: HashMap<String, ServiceConfig>,
}

impl SupervisorConfig {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let cfg: SupervisorConfig = toml::from_str(&content)?;
        Ok(cfg)
    }
}
