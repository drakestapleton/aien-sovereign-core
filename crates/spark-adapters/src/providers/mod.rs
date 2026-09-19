pub mod anthropic;
pub mod openai;

pub use anthropic::*;
pub use openai::*;

use reqwest::Client;
use crate::models::{ChatMessage, ProviderType};

pub async fn call_provider_unary(
    client: &Client,
    provider: ProviderType,
    endpoint: &str,
    model: &str,
    api_key: Option<&str>,
    messages: &[ChatMessage],
    temperature: Option<f32>,
) -> Result<(String, Option<String>, usize), String> {
    match provider {
        ProviderType::Anthropic => {
            let key = api_key.ok_or_else(|| "Anthropic requires an API key".to_string())?;
            call_anthropic_unary(client, endpoint, model, key, messages, temperature).await
        }
        ProviderType::OpenAI
        | ProviderType::OpenRouter
        | ProviderType::Gemini
        | ProviderType::Groq
        | ProviderType::DeepSeek
        | ProviderType::Together
        | ProviderType::Mistral
        | ProviderType::Ollama
        | ProviderType::LocalMax => {
            call_openai_unary(client, endpoint, model, api_key, messages, temperature).await
        }
    }
}
