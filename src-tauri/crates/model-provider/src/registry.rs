//! Provider 注册表。按 provider_id 查找实现。

use std::collections::HashMap;
use std::sync::Arc;

use crate::traits::ModelProvider;

pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn ModelProvider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    /// 注册内置 Provider 的默认集合
    pub fn with_builtins() -> Self {
        let mut r = Self::new();
        r.register(Arc::new(crate::providers::openai::OpenAiProvider::new(
            "openai",
            "OpenAI",
            "https://api.openai.com/v1",
        )));
        r.register(Arc::new(crate::providers::openai::OpenAiProvider::new(
            "deepseek",
            "DeepSeek",
            "https://api.deepseek.com/v1",
        )));
        r.register(Arc::new(crate::providers::openai::OpenAiProvider::new(
            "openrouter",
            "OpenRouter",
            "https://openrouter.ai/api/v1",
        )));
        r.register(Arc::new(crate::providers::openai::OpenAiProvider::new(
            "ollama",
            "Ollama",
            "http://localhost:11434/v1",
        )));
        r
    }

    pub fn register(&mut self, provider: Arc<dyn ModelProvider>) {
        self.providers
            .insert(provider.provider_id().to_string(), provider);
    }

    pub fn get(&self, provider_id: &str) -> Option<Arc<dyn ModelProvider>> {
        self.providers.get(provider_id).cloned()
    }

    pub fn list_ids(&self) -> Vec<&str> {
        self.providers.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}
