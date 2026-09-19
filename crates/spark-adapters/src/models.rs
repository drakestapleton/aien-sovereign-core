use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderType {
    OpenAI,
    Anthropic,
    OpenRouter,
    Gemini,
    Groq,
    Ollama,
    DeepSeek,
    Together,
    Mistral,
    LocalMax,
}

impl ProviderType {
    pub fn slug(&self) -> &'static str {
        match self {
            ProviderType::OpenAI => "openai",
            ProviderType::Anthropic => "anthropic",
            ProviderType::OpenRouter => "openrouter",
            ProviderType::Gemini => "gemini",
            ProviderType::Groq => "groq",
            ProviderType::Ollama => "ollama",
            ProviderType::DeepSeek => "deepseek",
            ProviderType::Together => "together",
            ProviderType::Mistral => "mistral",
            ProviderType::LocalMax => "local_max",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            ProviderType::OpenAI => "OpenAI",
            ProviderType::Anthropic => "Anthropic",
            ProviderType::OpenRouter => "OpenRouter (Multi-Model Gateway)",
            ProviderType::Gemini => "Google Gemini",
            ProviderType::Groq => "Groq LPU Engine",
            ProviderType::Ollama => "Ollama Local Engine",
            ProviderType::DeepSeek => "DeepSeek Cloud",
            ProviderType::Together => "Together AI",
            ProviderType::Mistral => "Mistral AI",
            ProviderType::LocalMax => "Modular MAX (GB10 Native)",
        }
    }

    pub fn default_endpoint(&self) -> &'static str {
        match self {
            ProviderType::OpenAI => "https://api.openai.com/v1/chat/completions",
            ProviderType::Anthropic => "https://api.anthropic.com/v1/messages",
            ProviderType::OpenRouter => "https://openrouter.ai/api/v1/chat/completions",
            ProviderType::Gemini => "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
            ProviderType::Groq => "https://api.groq.com/openai/v1/chat/completions",
            ProviderType::Ollama => "http://127.0.0.1:11434/v1/chat/completions",
            ProviderType::DeepSeek => "https://api.deepseek.com/v1/chat/completions",
            ProviderType::Together => "https://api.together.xyz/v1/chat/completions",
            ProviderType::Mistral => "https://api.mistral.ai/v1/chat/completions",
            ProviderType::LocalMax => "http://127.0.0.1:18006/v1/chat/completions",
        }
    }

    pub fn key_name(&self) -> Option<&'static str> {
        match self {
            ProviderType::OpenAI => Some("OPENAI_API_KEY"),
            ProviderType::Anthropic => Some("ANTHROPIC_API_KEY"),
            ProviderType::OpenRouter => Some("OPENROUTER_API_KEY"),
            ProviderType::Gemini => Some("GEMINI_API_KEY"),
            ProviderType::Groq => Some("GROQ_API_KEY"),
            ProviderType::Ollama => None,
            ProviderType::DeepSeek => Some("DEEPSEEK_API_KEY"),
            ProviderType::Together => Some("TOGETHER_API_KEY"),
            ProviderType::Mistral => Some("MISTRAL_API_KEY"),
            ProviderType::LocalMax => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterSpec {
    pub id: String,
    pub name: String,
    pub provider: ProviderType,
    pub endpoint: String,
    pub model_id: String,
    pub context_length: usize,
    pub is_available: bool,
    pub requires_key: bool,
    pub is_resident: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedChunk {
    pub content: String,
    pub reasoning: String,
    pub is_done: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    CodeSynthesis,
    SystemArchitecture,
    ReasoningTrace,
    RefactorLogic,
    VerificationHarness,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStrategy {
    CompilerCheck,
    JsonSchema,
    UnslopStrict,
    DualConsensus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationOutcome {
    pub passed: bool,
    pub score: f32,
    pub rule_violations: Vec<String>,
    pub compiler_output: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rollout {
    pub provider: ProviderType,
    pub model_id: String,
    pub content: String,
    pub reasoning: Option<String>,
    pub duration_ms: u64,
    pub token_count: usize,
    pub verification: VerificationOutcome,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistillTask {
    pub id: String,
    pub task_type: TaskType,
    pub prompt: String,
    pub system_prompt: Option<String>,
    pub teacher_model: String,
    pub student_model: Option<String>,
    pub verification_strategy: VerificationStrategy,
    pub commit_to_cortex: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistillationRecord {
    pub task_id: String,
    pub task_type: TaskType,
    pub prompt: String,
    pub system_prompt: Option<String>,
    pub teacher_rollout: Rollout,
    pub student_rollout: Option<Rollout>,
    pub chosen: String,
    pub chosen_reasoning: Option<String>,
    pub rejected: Option<String>,
    pub rejected_reasoning: Option<String>,
    pub preference_delta: f32,
    pub durable_memory_committed: bool,
    pub cortex_entity_id: Option<String>,
    pub created_at: String,
}
