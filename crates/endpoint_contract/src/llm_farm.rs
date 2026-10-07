//! LLM Farm endpoint recognition.
//!
//! LLM Farm is treated as a model backend with its own endpoint kind, not
//! merely as an MCP server. Discovery goes through its OpenAI-compatible
//! model listing, plus optional GGUF metadata that a well-behaved LLM Farm
//! endpoint can attach per model.
//!
//! Typical deployment: LLM Farm runs on an iPhone (the inference machine);
//! the Mac only relays. Defaults therefore assume a LAN address that the
//! user overrides in settings.

use crate::endpoint::{EndpointCapabilities, EndpointKind};
use serde::{Deserialize, Serialize};
use url::Url;

/// Default LLM Farm HTTP port.
pub const DEFAULT_LLM_FARM_PORT: u16 = 9090;

/// User-configured LLM Farm endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct LlmFarmConfig {
    /// Base URL of the LLM Farm server, e.g. `http://192.168.1.20:9090/v1`.
    pub base_url: String,
    /// Environment variable holding an optional API key.
    pub api_key_env: Option<String>,
}

impl Default for LlmFarmConfig {
    fn default() -> Self {
        LlmFarmConfig {
            base_url: format!("http://127.0.0.1:{DEFAULT_LLM_FARM_PORT}/v1"),
            api_key_env: None,
        }
    }
}

impl LlmFarmConfig {
    pub fn validate(&self) -> anyhow::Result<()> {
        let base = Url::parse(&self.base_url)?;
        match base.scheme() {
            "http" | "https" => {}
            scheme => anyhow::bail!("LLM Farm base URL must use HTTP or HTTPS, got {scheme:?}"),
        }
        Ok(())
    }

    pub fn models_url(&self) -> anyhow::Result<Url> {
        let base = Url::parse(&self.base_url)?;
        base.join("models")
            .map_err(|error| anyhow::anyhow!("building LLM Farm models URL: {error}"))
    }

    pub fn endpoint_kind(&self) -> EndpointKind {
        EndpointKind::LlmFarm
    }

    pub fn capabilities(&self) -> EndpointCapabilities {
        EndpointCapabilities::MODELS_ONLY
    }
}

/// GGUF-specific metadata a model listing can carry. All fields optional:
/// model-list content is untrusted and missing metadata must not be invented.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct GgufModelMetadata {
    /// Quantization label, e.g. `Q4_K_M`. Reported as-is, never guessed.
    pub quantization: Option<String>,
    pub parameter_count: Option<u64>,
    pub file_size_bytes: Option<u64>,
    pub context_length: Option<u64>,
    /// Source repository, e.g. a Hugging Face repo id.
    pub source_repo: Option<String>,
}

/// A model served by an LLM Farm instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct LlmFarmModel {
    pub id: String,
    pub display_name: String,
    #[serde(flatten)]
    pub gguf: GgufModelMetadata,
    pub supports_tools: bool,
}

impl Default for LlmFarmModel {
    fn default() -> Self {
        LlmFarmModel {
            id: String::new(),
            display_name: String::new(),
            gguf: GgufModelMetadata::default(),
            supports_tools: false,
        }
    }
}

/// Parses an OpenAI-compatible models response with optional per-model
/// metadata fields into [`LlmFarmModel`]s.
pub fn parse_llm_farm_models_response(body: &str) -> anyhow::Result<Vec<LlmFarmModel>> {
    #[derive(Deserialize)]
    struct ModelsResponse {
        #[serde(default)]
        data: Vec<ModelEntry>,
    }

    #[derive(Deserialize)]
    struct ModelEntry {
        id: String,
        #[serde(default)]
        display_name: Option<String>,
        #[serde(default)]
        quantization: Option<String>,
        #[serde(default)]
        parameter_count: Option<u64>,
        #[serde(default)]
        file_size_bytes: Option<u64>,
        #[serde(default)]
        context_length: Option<u64>,
        #[serde(default)]
        source_repo: Option<String>,
    }

    let response: ModelsResponse = serde_json::from_str(body)?;
    Ok(response
        .data
        .into_iter()
        .map(|entry| LlmFarmModel {
            id: entry.id.clone(),
            display_name: entry.display_name.unwrap_or(entry.id),
            gguf: GgufModelMetadata {
                quantization: entry.quantization,
                parameter_count: entry.parameter_count,
                file_size_bytes: entry.file_size_bytes,
                context_length: entry.context_length,
                source_repo: entry.source_repo,
            },
            supports_tools: false,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_points_at_default_port() {
        let config = LlmFarmConfig::default();
        assert!(config.validate().is_ok());
        assert_eq!(
            config.models_url().unwrap().as_str(),
            format!("http://127.0.0.1:{DEFAULT_LLM_FARM_PORT}/v1/models")
        );
    }

    #[test]
    fn rejects_non_http_llm_farm_url() {
        let config = LlmFarmConfig {
            base_url: "file:///models".into(),
            api_key_env: None,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn parses_llm_farm_models_with_metadata() {
        let models = parse_llm_farm_models_response(
            r#"{
                "data": [
                    {
                        "id": "qwen2.5-coder",
                        "quantization": "Q4_K_M",
                        "parameter_count": 7700000000,
                        "context_length": 32768,
                        "source_repo": "Qwen/Qwen2.5-Coder-7B-Instruct-GGUF"
                    }
                ]
            }"#,
        )
        .unwrap();
        assert_eq!(models.len(), 1);
        let model = &models[0];
        assert_eq!(model.id, "qwen2.5-coder");
        assert_eq!(model.display_name, "qwen2.5-coder");
        assert_eq!(model.gguf.quantization.as_deref(), Some("Q4_K_M"));
        assert_eq!(model.gguf.context_length, Some(32768));
        // Never invent capabilities the endpoint did not report.
        assert!(!model.supports_tools);
    }

    #[test]
    fn parses_plain_open_ai_style_list() {
        let models =
            parse_llm_farm_models_response(r#"{"data": [{"id": "spark"}]}"#).unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].gguf.quantization, None);
    }
}
