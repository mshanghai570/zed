//! Custom provider configuration, wire API selection, and model discovery.
//!
//! This is the `provider-discovery` vocabulary: a provider is an
//! [`EndpointDescriptor`] with [`EndpointKind::ModelProvider`], plus the
//! request shape needed to talk to it and the models it serves.

use crate::endpoint::{EndpointCapabilities, EndpointKind};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use url::Url;

/// The request shape a custom provider speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WireApi {
    /// OpenAI-compatible `/v1/chat/completions`.
    OpenAiChatCompletions,
    /// OpenAI Responses API.
    OpenAiResponses,
    /// Anthropic-compatible `/v1/messages`.
    AnthropicMessages,
    /// Ollama-style API.
    OllamaApi,
    /// A provider-specific shape handled by a dedicated adapter.
    Custom,
}

/// User-configured custom provider.
///
/// Mirrors the existing `language_models` custom-provider settings shape so it
/// can be adopted without changing user settings files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct CustomProviderConfig {
    pub id: String,
    pub name: String,
    /// Base URL of the provider, e.g. `http://127.0.0.1:8080/v1`.
    pub base_url: String,
    /// Environment variable holding the API key, if the provider needs one.
    pub api_key_env: Option<String>,
    /// Additional static headers. Must not contain secrets.
    pub headers: BTreeMap<String, String>,
    pub wire_api: WireApi,
}

impl Default for CustomProviderConfig {
    fn default() -> Self {
        CustomProviderConfig {
            id: String::new(),
            name: String::new(),
            base_url: String::new(),
            api_key_env: None,
            headers: BTreeMap::new(),
            wire_api: WireApi::OpenAiChatCompletions,
        }
    }
}

impl CustomProviderConfig {
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.id.trim().is_empty() {
            anyhow::bail!("provider id must not be empty");
        }
        let base = Url::parse(&self.base_url)?;
        match base.scheme() {
            "http" | "https" => {}
            scheme => anyhow::bail!("provider base URL must use HTTP or HTTPS, got {scheme:?}"),
        }
        if matches!(self.wire_api, WireApi::Custom) {
            // A custom wire API needs a dedicated adapter registered elsewhere;
            // configuration alone is not enough to serve requests.
            anyhow::bail!(
                "provider {} uses a custom wire API; no custom adapter is registered",
                self.id
            );
        }
        Ok(())
    }

    /// URL used for OpenAI-compatible model discovery (`/models` relative to
    /// the base URL).
    pub fn models_url(&self) -> anyhow::Result<Url> {
        let base = Url::parse(&self.base_url)?;
        base.join("models")
            .map_err(|error| anyhow::anyhow!("building models URL for provider {}: {error}", self.id))
    }

    /// The endpoint capabilities this provider advertises.
    pub fn capabilities(&self) -> EndpointCapabilities {
        EndpointCapabilities::MODELS_ONLY
    }

    pub fn endpoint_kind(&self) -> EndpointKind {
        EndpointKind::ModelProvider
    }
}

/// A model returned by discovery, normalized across providers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct DiscoveredModel {
    pub id: String,
    pub display_name: String,
    pub context_window: Option<u64>,
    pub max_output_tokens: Option<u64>,
    pub supports_tools: bool,
    pub supports_vision: bool,
}

impl Default for DiscoveredModel {
    fn default() -> Self {
        DiscoveredModel {
            id: String::new(),
            display_name: String::new(),
            context_window: None,
            max_output_tokens: None,
            supports_tools: false,
            supports_vision: false,
        }
    }
}

/// Trait implemented by anything that can enumerate models from an endpoint.
#[async_trait]
pub trait ModelDiscovery {
    async fn discover_models(&self) -> anyhow::Result<Vec<DiscoveredModel>>;
}

/// Parses an OpenAI-compatible `/v1/models` response body into normalized
/// models. Model-list content is untrusted: only ids are taken at face value
/// and everything else is optional metadata.
pub fn parse_open_ai_models_response(body: &str) -> anyhow::Result<Vec<DiscoveredModel>> {
    #[derive(Deserialize)]
    struct ModelsResponse {
        #[serde(default)]
        data: Vec<ModelEntry>,
    }

    #[derive(Deserialize)]
    struct ModelEntry {
        id: String,
    }

    let response: ModelsResponse = serde_json::from_str(body)?;
    Ok(response
        .data
        .into_iter()
        .map(|entry| DiscoveredModel {
            id: entry.id.clone(),
            display_name: entry.id,
            ..Default::default()
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_provider_configuration() {
        let provider = CustomProviderConfig {
            id: "my-provider".into(),
            name: "My Provider".into(),
            base_url: "http://127.0.0.1:8080/v1".into(),
            ..Default::default()
        };
        assert!(provider.validate().is_ok());
        assert_eq!(
            provider.models_url().unwrap().as_str(),
            "http://127.0.0.1:8080/v1/models"
        );
    }

    #[test]
    fn rejects_custom_wire_api_without_adapter() {
        let provider = CustomProviderConfig {
            id: "custom".into(),
            base_url: "https://api.example.com".into(),
            wire_api: WireApi::Custom,
            ..Default::default()
        };
        assert!(provider.validate().is_err());
    }

    #[test]
    fn rejects_non_http_base_url() {
        let provider = CustomProviderConfig {
            id: "bad".into(),
            base_url: "file:///etc".into(),
            ..Default::default()
        };
        assert!(provider.validate().is_err());
    }

    #[test]
    fn parses_open_ai_models_response() {
        let models = parse_open_ai_models_response(
            r#"{"data": [{"id": "gpt-test"}, {"id": "qwen-coder"}]}"#,
        )
        .unwrap();
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "gpt-test");
        assert_eq!(models[1].display_name, "qwen-coder");
    }

    #[test]
    fn parses_empty_models_response() {
        let models = parse_open_ai_models_response("{}").unwrap();
        assert!(models.is_empty());
    }

    #[test]
    fn rejects_malformed_models_response() {
        assert!(parse_open_ai_models_response("not json").is_err());
    }
}
