//! Common identity, transport, authentication, capability, and health model
//! for model and tool endpoints.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use url::Url;

/// What an endpoint is, independent of transport or capabilities.
///
/// This is what lets Zed understand that a single endpoint can provide AI
/// capabilities (models), tools, or both, instead of forcing every endpoint
/// into one narrow category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndpointKind {
    /// A language-model provider (OpenAI-compatible, Anthropic-compatible, etc.).
    ModelProvider,
    /// A Model Context Protocol server.
    McpServer,
    /// An LLM Farm instance (iPhone/local GGUF inference backend).
    LlmFarm,
    /// An Agent Client Protocol agent.
    AcpAgent,
    /// An extension registry/marketplace source (e.g. Open VSX).
    ExtensionSource,
}

impl EndpointKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            EndpointKind::ModelProvider => "model_provider",
            EndpointKind::McpServer => "mcp_server",
            EndpointKind::LlmFarm => "llm_farm",
            EndpointKind::AcpAgent => "acp_agent",
            EndpointKind::ExtensionSource => "extension_source",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            EndpointKind::ModelProvider => "Model provider",
            EndpointKind::McpServer => "MCP server",
            EndpointKind::LlmFarm => "LLM Farm",
            EndpointKind::AcpAgent => "ACP agent",
            EndpointKind::ExtensionSource => "Extension source",
        }
    }
}

/// How an endpoint is reached.
///
/// Streamable HTTP is the preferred modern transport for remote MCP; legacy
/// SSE and stdio exist for servers that require them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum EndpointTransport {
    /// MCP Streamable HTTP, the preferred remote transport.
    StreamableHttp { url: String },
    /// Legacy HTTP+SSE transport, only for servers without Streamable HTTP.
    HttpSse { url: String },
    /// Local stdio process.
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: BTreeMap<String, String>,
    },
}

impl EndpointTransport {
    /// Returns the endpoint URL, if this transport is URL-based.
    ///
    /// Only `http` and `https` schemes are accepted.
    pub fn url(&self) -> anyhow::Result<Url> {
        let raw = match self {
            EndpointTransport::StreamableHttp { url } | EndpointTransport::HttpSse { url } => url,
            EndpointTransport::Stdio { .. } => anyhow::bail!("stdio transport has no URL"),
        };
        let url = Url::parse(raw)?;
        match url.scheme() {
            "http" | "https" => Ok(url),
            scheme => anyhow::bail!("endpoint URL must use HTTP or HTTPS, got {scheme:?}"),
        }
    }
}

/// How an endpoint authenticates.
///
/// Secrets are referenced by environment variable name so credentials never
/// live in settings files, manifests, logs, or the context inspector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum EndpointAuth {
    None,
    /// `Authorization: Bearer <value of env var>`.
    BearerToken { token_env: String },
    /// An arbitrary header whose value comes from an environment variable.
    Header { name: String, value_env: String },
}

impl Default for EndpointAuth {
    fn default() -> Self {
        EndpointAuth::None
    }
}

/// The capability surface an endpoint advertises or is configured with.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct EndpointCapabilities {
    /// The endpoint can serve language models for chat/agent use.
    pub language_models: bool,
    /// The endpoint exposes callable tools.
    pub tools: bool,
    /// The endpoint exposes resources (context material).
    pub resources: bool,
    /// The endpoint exposes prompts/templates.
    pub prompts: bool,
    /// The endpoint can host agent threads (ACP-style).
    pub agent_threads: bool,
}

impl EndpointCapabilities {
    pub const NONE: EndpointCapabilities = EndpointCapabilities {
        language_models: false,
        tools: false,
        resources: false,
        prompts: false,
        agent_threads: false,
    };

    pub const MODELS_ONLY: EndpointCapabilities = EndpointCapabilities {
        language_models: true,
        ..EndpointCapabilities::NONE
    };

    pub const MCP_TOOLS_ONLY: EndpointCapabilities = EndpointCapabilities {
        tools: true,
        ..EndpointCapabilities::NONE
    };

    pub fn is_empty(&self) -> bool {
        *self == Self::NONE
    }

    /// Union of two capability sets, e.g. after discovery.
    pub fn union(&self, other: &EndpointCapabilities) -> EndpointCapabilities {
        EndpointCapabilities {
            language_models: self.language_models || other.language_models,
            tools: self.tools || other.tools,
            resources: self.resources || other.resources,
            prompts: self.prompts || other.prompts,
            agent_threads: self.agent_threads || other.agent_threads,
        }
    }
}

/// Last-known health of an endpoint, for status surfaces and diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndpointState {
    Unknown,
    Reachable,
    Unreachable { detail: Option<String> },
    Unauthorized,
}

/// A configured endpoint: everything the platform needs to identify, reach,
/// authenticate with, and reason about an endpoint, before any client code is
/// attached.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct EndpointDescriptor {
    pub id: String,
    pub kind: EndpointKind,
    pub transport: EndpointTransport,
    pub auth: EndpointAuth,
    pub capabilities: EndpointCapabilities,
    /// Extra static headers, e.g. custom provider headers. Values must not
    /// contain secrets; use [`EndpointAuth`] for credentials.
    pub headers: BTreeMap<String, String>,
    pub state: EndpointState,
}

impl Default for EndpointDescriptor {
    fn default() -> Self {
        EndpointDescriptor {
            id: String::new(),
            kind: EndpointKind::ModelProvider,
            transport: EndpointTransport::Stdio {
                command: String::new(),
                args: Vec::new(),
                env: BTreeMap::new(),
            },
            auth: EndpointAuth::None,
            capabilities: EndpointCapabilities::NONE,
            headers: BTreeMap::new(),
            state: EndpointState::Unknown,
        }
    }
}

impl EndpointDescriptor {
    /// Validates the descriptor so failures surface at configuration time,
    /// not mid-request.
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.id.trim().is_empty() {
            anyhow::bail!("endpoint id must not be empty");
        }
        // URL-based transports must parse and use HTTP(S). Stdio must name a
        // command. This intentionally does not forbid stdio: local MCP servers
        // and ACP agents are legitimate endpoints.
        match &self.transport {
            EndpointTransport::Stdio { command, .. } => {
                if command.trim().is_empty() {
                    anyhow::bail!("endpoint {} has empty stdio command", self.id);
                }
            }
            _ => {
                self.transport.url()?;
            }
        }
        Ok(())
    }

    pub fn supports_language_models(&self) -> bool {
        self.capabilities.language_models
    }

    pub fn supports_tools(&self) -> bool {
        self.capabilities.tools
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn http_endpoint(url: &str) -> EndpointDescriptor {
        EndpointDescriptor {
            id: "test".into(),
            kind: EndpointKind::McpServer,
            transport: EndpointTransport::StreamableHttp { url: url.into() },
            ..Default::default()
        }
    }

    #[test]
    fn rejects_non_http_endpoint_urls() {
        let descriptor = http_endpoint("ftp://example.com/mcp");
        assert!(descriptor.validate().is_err());
    }

    #[test]
    fn accepts_https_streamable_http_endpoint() {
        let descriptor = http_endpoint("https://example.com/mcp");
        assert!(descriptor.validate().is_ok());
    }

    #[test]
    fn rejects_empty_id() {
        let descriptor = http_endpoint("https://example.com/mcp");
        let mut descriptor = descriptor;
        descriptor.id = "  ".into();
        assert!(descriptor.validate().is_err());
    }

    #[test]
    fn stdio_endpoint_requires_command() {
        let descriptor = EndpointDescriptor {
            id: "local".into(),
            kind: EndpointKind::AcpAgent,
            ..Default::default()
        };
        assert!(descriptor.validate().is_err());
    }

    #[test]
    fn capability_union_is_additive() {
        let a = EndpointCapabilities::MODELS_ONLY;
        let b = EndpointCapabilities::MCP_TOOLS_ONLY;
        let union = a.union(&b);
        assert!(union.language_models && union.tools);
        assert!(!union.resources);
    }

    #[test]
    fn endpoint_kind_roundtrips_through_serde() {
        for kind in [
            EndpointKind::ModelProvider,
            EndpointKind::McpServer,
            EndpointKind::LlmFarm,
            EndpointKind::AcpAgent,
            EndpointKind::ExtensionSource,
        ] {
            let json = serde_json::to_string(&kind).unwrap();
            let back: EndpointKind = serde_json::from_str(&json).unwrap();
            assert_eq!(kind, back);
        }
    }
}
