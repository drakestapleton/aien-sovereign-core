use reqwest::Client;

use crate::models::{AdapterSpec, ProviderType};
use crate::vault::{is_secret_present, resolve_secret};

pub struct AdapterRouter {
    pub client: Client,
    pub catalog: Vec<AdapterSpec>,
}

impl AdapterRouter {
    pub fn new() -> Self {
        Self {
            client: Client::builder().build().unwrap_or_default(),
            catalog: Self::default_catalog(),
        }
    }

    pub fn default_catalog() -> Vec<AdapterSpec> {
        vec![
            // 1. Local Resident Seats
            AdapterSpec {
                id: "atlas-lightning-omni".to_string(),
                name: "Atlas Nemotron-3.5 30B (Resident)".to_string(),
                provider: ProviderType::LocalMax,
                endpoint: ProviderType::LocalMax.default_endpoint().to_string(),
                model_id: "atlas-lightning-omni".to_string(),
                context_length: 32768,
                is_available: true,
                requires_key: false,
                is_resident: true,
            },
            // 2. OpenAI
            AdapterSpec {
                id: "openai/gpt-4o".to_string(),
                name: "OpenAI GPT-4o".to_string(),
                provider: ProviderType::OpenAI,
                endpoint: ProviderType::OpenAI.default_endpoint().to_string(),
                model_id: "gpt-4o".to_string(),
                context_length: 128000,
                is_available: is_secret_present("OPENAI_API_KEY"),
                requires_key: true,
                is_resident: false,
            },
            AdapterSpec {
                id: "openai/o3-mini".to_string(),
                name: "OpenAI o3-mini (Reasoning)".to_string(),
                provider: ProviderType::OpenAI,
                endpoint: ProviderType::OpenAI.default_endpoint().to_string(),
                model_id: "o3-mini".to_string(),
                context_length: 200000,
                is_available: is_secret_present("OPENAI_API_KEY"),
                requires_key: true,
                is_resident: false,
            },
            // 3. Anthropic
            AdapterSpec {
                id: "anthropic/claude-3-7-sonnet".to_string(),
                name: "Claude 3.7 Sonnet (Hybrid Thinking)".to_string(),
                provider: ProviderType::Anthropic,
                endpoint: ProviderType::Anthropic.default_endpoint().to_string(),
                model_id: "claude-3-7-sonnet-20250219".to_string(),
                context_length: 200000,
                is_available: is_secret_present("ANTHROPIC_API_KEY"),
                requires_key: true,
                is_resident: false,
            },
            // 4. OpenRouter Gateway
            AdapterSpec {
                id: "openrouter/auto".to_string(),
                name: "OpenRouter Auto Gateway".to_string(),
                provider: ProviderType::OpenRouter,
                endpoint: ProviderType::OpenRouter.default_endpoint().to_string(),
                model_id: "openrouter/auto".to_string(),
                context_length: 128000,
                is_available: is_secret_present("OPENROUTER_API_KEY"),
                requires_key: true,
                is_resident: false,
            },
            // 5. Google Gemini
            AdapterSpec {
                id: "gemini/gemini-2.5-flash".to_string(),
                name: "Google Gemini 2.5 Flash".to_string(),
                provider: ProviderType::Gemini,
                endpoint: ProviderType::Gemini.default_endpoint().to_string(),
                model_id: "gemini-2.5-flash".to_string(),
                context_length: 1000000,
                is_available: is_secret_present("GEMINI_API_KEY"),
                requires_key: true,
                is_resident: false,
            },
            // 6. Groq Fast LPU
            AdapterSpec {
                id: "groq/llama-3.3-70b-versatile".to_string(),
                name: "Groq Llama 3.3 70B (Sub-100ms LPU)".to_string(),
                provider: ProviderType::Groq,
                endpoint: ProviderType::Groq.default_endpoint().to_string(),
                model_id: "llama-3.3-70b-versatile".to_string(),
                context_length: 128000,
                is_available: is_secret_present("GROQ_API_KEY"),
                requires_key: true,
                is_resident: false,
            },
            // 7. Ollama Local
            AdapterSpec {
                id: "ollama/qwen2.5-coder".to_string(),
                name: "Ollama Local (Qwen 2.5 Coder)".to_string(),
                provider: ProviderType::Ollama,
                endpoint: ProviderType::Ollama.default_endpoint().to_string(),
                model_id: "qwen2.5-coder:7b".to_string(),
                context_length: 32768,
                is_available: true,
                requires_key: false,
                is_resident: false,
            },
            // 8. DeepSeek Direct
            AdapterSpec {
                id: "deepseek/deepseek-chat".to_string(),
                name: "DeepSeek V3".to_string(),
                provider: ProviderType::DeepSeek,
                endpoint: ProviderType::DeepSeek.default_endpoint().to_string(),
                model_id: "deepseek-chat".to_string(),
                context_length: 64000,
                is_available: is_secret_present("DEEPSEEK_API_KEY"),
                requires_key: true,
                is_resident: false,
            },
        ]
    }

    pub fn discover_adapters(&self) -> Vec<AdapterSpec> {
        let mut list = self.catalog.clone();
        for spec in &mut list {
            if let Some(key_name) = spec.provider.key_name() {
                spec.is_available = is_secret_present(key_name);
            }
        }
        list
    }

    pub fn resolve_route(&self, model_query: &str) -> (ProviderType, String, String, Option<String>) {
        let trimmed = model_query.trim();

        // 1. Direct match in catalog
        for spec in &self.catalog {
            if spec.id == trimmed || spec.model_id == trimmed {
                let key = spec.provider.key_name().and_then(resolve_secret);
                return (spec.provider, spec.endpoint.clone(), spec.model_id.clone(), key);
            }
        }

        // 2. Prefix based matching
        if let Some(rest) = trimmed.strip_prefix("openai/") {
            let key = resolve_secret("OPENAI_API_KEY");
            return (ProviderType::OpenAI, ProviderType::OpenAI.default_endpoint().to_string(), rest.to_string(), key);
        }
        if let Some(rest) = trimmed.strip_prefix("anthropic/") {
            let key = resolve_secret("ANTHROPIC_API_KEY");
            return (ProviderType::Anthropic, ProviderType::Anthropic.default_endpoint().to_string(), rest.to_string(), key);
        }
        if let Some(rest) = trimmed.strip_prefix("openrouter/") {
            let key = resolve_secret("OPENROUTER_API_KEY");
            return (ProviderType::OpenRouter, ProviderType::OpenRouter.default_endpoint().to_string(), rest.to_string(), key);
        }
        if let Some(rest) = trimmed.strip_prefix("gemini/") {
            let key = resolve_secret("GEMINI_API_KEY");
            return (ProviderType::Gemini, ProviderType::Gemini.default_endpoint().to_string(), rest.to_string(), key);
        }
        if let Some(rest) = trimmed.strip_prefix("groq/") {
            let key = resolve_secret("GROQ_API_KEY");
            return (ProviderType::Groq, ProviderType::Groq.default_endpoint().to_string(), rest.to_string(), key);
        }
        if let Some(rest) = trimmed.strip_prefix("ollama/") {
            return (ProviderType::Ollama, ProviderType::Ollama.default_endpoint().to_string(), rest.to_string(), None);
        }
        if let Some(rest) = trimmed.strip_prefix("deepseek/") {
            let key = resolve_secret("DEEPSEEK_API_KEY");
            return (ProviderType::DeepSeek, ProviderType::DeepSeek.default_endpoint().to_string(), rest.to_string(), key);
        }

        // Default fallback to local MAX seat
        (
            ProviderType::LocalMax,
            ProviderType::LocalMax.default_endpoint().to_string(),
            "atlas-lightning-omni".to_string(),
            None,
        )
    }
}
