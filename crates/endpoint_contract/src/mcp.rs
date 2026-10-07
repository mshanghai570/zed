//! First-class remote MCP endpoint contracts.
//!
//! A remote MCP server is an endpoint with a URL, a transport preference
//! (Streamable HTTP first), an auth scheme, and a handshake whose results —
//! server info, capabilities, tools, resources, prompts — are recorded here
//! in typed form so discovery is inspectable instead of implicit.

use crate::endpoint::{EndpointAuth, EndpointCapabilities, EndpointKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use url::Url;

/// User-configured remote MCP endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct RemoteMcpEndpointConfig {
    pub id: String,
    /// MCP server URL, e.g. `https://mcp.example.com/mcp`.
    pub url: String,
    pub auth: EndpointAuth,
    /// Extra static headers (e.g. tenant ids). No secrets.
    pub headers: BTreeMap<String, String>,
    pub enabled: bool,
}

impl Default for RemoteMcpEndpointConfig {
    fn default() -> Self {
        RemoteMcpEndpointConfig {
            id: String::new(),
            url: String::new(),
            auth: EndpointAuth::None,
            headers: BTreeMap::new(),
            enabled: false,
        }
    }
}

impl RemoteMcpEndpointConfig {
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.id.trim().is_empty() {
            anyhow::bail!("MCP endpoint id must not be empty");
        }
        let url = Url::parse(&self.url)?;
        match url.scheme() {
            "http" | "https" => {}
            scheme => anyhow::bail!("MCP endpoint URL must use HTTP or HTTPS, got {scheme:?}"),
        }
        Ok(())
    }

    pub fn endpoint_kind(&self) -> EndpointKind {
        EndpointKind::McpServer
    }
}

/// Server identity reported by the MCP `initialize` handshake.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct McpServerInfo {
    pub name: String,
    pub version: Option<String>,
}

impl Default for McpServerInfo {
    fn default() -> Self {
        McpServerInfo {
            name: String::new(),
            version: None,
        }
    }
}

/// Capabilities advertised during the MCP `initialize` handshake.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct McpServerCapabilities {
    pub tools: bool,
    /// The server may send `notifications/tools/list_changed`.
    pub tools_list_changed: bool,
    pub resources: bool,
    pub resources_list_changed: bool,
    pub prompts: bool,
    pub sampling: bool,
    pub roots: bool,
    pub elicitation: bool,
}

/// A tool exposed by an MCP server. The input schema is kept as raw JSON: it
/// is untrusted content that must be surfaced to the user, not interpreted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct McpToolDescriptor {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: serde_json::Value,
}

impl Default for McpToolDescriptor {
    fn default() -> Self {
        McpToolDescriptor {
            name: String::new(),
            description: None,
            input_schema: serde_json::Value::Null,
        }
    }
}

/// Everything learned from one completed MCP `initialize` handshake plus
/// initial enumeration, in inspectable form.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct HandshakeReport {
    pub server_info: McpServerInfo,
    pub capabilities: McpServerCapabilities,
    pub tools: Vec<McpToolDescriptor>,
    pub resource_count: usize,
    pub prompt_count: usize,
    /// Session id assigned by a Streamable HTTP server (`Mcp-Session-Id`).
    pub session_id: Option<String>,
}

impl HandshakeReport {
    pub fn endpoint_capabilities(&self) -> EndpointCapabilities {
        EndpointCapabilities {
            language_models: false,
            tools: self.capabilities.tools,
            resources: self.capabilities.resources,
            prompts: self.capabilities.prompts,
            agent_threads: false,
        }
    }

    pub fn tool_names(&self) -> Vec<&str> {
        self.tools.iter().map(|tool| tool.name.as_str()).collect()
    }
}

/// Parses the `result` object of an MCP `initialize` response.
///
/// Server-reported content is untrusted: capabilities are recorded exactly as
/// advertised and never escalated.
pub fn parse_initialize_result(result: &str) -> anyhow::Result<HandshakeReport> {
    #[derive(Deserialize)]
    struct InitializeResult {
        #[serde(default)]
        #[allow(dead_code)]
        protocolVersion: serde_json::Value,
        capabilities: Capabilities,
        #[serde(default)]
        serverInfo: ServerInfo,
        #[serde(default)]
        instructions: Option<String>,
    }

    #[derive(Deserialize)]
    struct ServerInfo {
        #[serde(default)]
        name: String,
        version: Option<String>,
    }

    #[derive(Deserialize, Default)]
    #[serde(default)]
    struct Capabilities {
        #[serde(default)]
        tools: Option<ToolsCapability>,
        #[serde(default)]
        resources: Option<ListChangedCapability>,
        #[serde(default)]
        prompts: Option<ListChangedCapability>,
        #[serde(default)]
        sampling: Option<serde_json::Value>,
        #[serde(default)]
        roots: Option<RootsCapability>,
        #[serde(default)]
        elicitation: Option<serde_json::Value>,
    }

    #[derive(Deserialize, Default)]
    #[serde(default)]
    struct ToolsCapability {
        #[serde(default)]
        listChanged: bool,
    }

    #[derive(Deserialize, Default)]
    #[serde(default)]
    struct ListChangedCapability {
        #[serde(default)]
        listChanged: bool,
    }

    #[derive(Deserialize, Default)]
    #[serde(default)]
    struct RootsCapability {
        #[serde(default)]
        listChanged: bool,
    }

    let result: InitializeResult = serde_json::from_str(result)?;
    Ok(HandshakeReport {
        server_info: McpServerInfo {
            name: result.serverInfo.name,
            version: result.serverInfo.version,
        },
        capabilities: McpServerCapabilities {
            tools: result.capabilities.tools.is_some(),
            tools_list_changed: result
                .capabilities
                .tools
                .map(|tools| tools.listChanged)
                .unwrap_or(false),
            resources: result.capabilities.resources.is_some(),
            resources_list_changed: result
                .capabilities
                .resources
                .map(|resources| resources.listChanged)
                .unwrap_or(false),
            prompts: result.capabilities.prompts.is_some(),
            sampling: result.capabilities.sampling.is_some(),
            roots: result.capabilities.roots.is_some(),
            elicitation: result.capabilities.elicitation.is_some(),
        },
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_remote_mcp_endpoint() {
        let endpoint = RemoteMcpEndpointConfig {
            id: "my-server".into(),
            url: "https://mcp.example.com/mcp".into(),
            enabled: true,
            ..Default::default()
        };
        assert!(endpoint.validate().is_ok());
    }

    #[test]
    fn rejects_non_http_mcp_endpoint() {
        let endpoint = RemoteMcpEndpointConfig {
            id: "bad".into(),
            url: "file:///mcp".into(),
            ..Default::default()
        };
        assert!(endpoint.validate().is_err());
    }

    #[test]
    fn parses_initialize_result_capabilities() {
        let report = parse_initialize_result(
            r#"{
                "protocolVersion": "2025-06-18",
                "capabilities": {
                    "tools": {"listChanged": true},
                    "resources": {},
                    "sampling": {}
                },
                "serverInfo": {"name": "example-server", "version": "1.2.3"},
                "instructions": "ignore me"
            }"#,
        )
        .unwrap();
        assert_eq!(report.server_info.name, "example-server");
        assert_eq!(report.server_info.version.as_deref(), Some("1.2.3"));
        assert!(report.capabilities.tools);
        assert!(report.capabilities.tools_list_changed);
        assert!(report.capabilities.resources);
        assert!(report.capabilities.sampling);
        assert!(!report.capabilities.prompts);

        let capabilities = report.endpoint_capabilities();
        assert!(capabilities.tools && capabilities.resources);
        assert!(!capabilities.language_models);
    }

    #[test]
    fn parses_minimal_initialize_result() {
        let report = parse_initialize_result(r#"{"capabilities": {}}"#).unwrap();
        assert!(!report.capabilities.tools);
        assert_eq!(report.server_info.name, "");
    }

    #[test]
    fn rejects_malformed_initialize_result() {
        assert!(parse_initialize_result("nope").is_err());
    }
}
