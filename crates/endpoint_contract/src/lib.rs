//! Shared contracts for the fork's AI extension platform.
//!
//! This crate is the `endpoint-contract` foundation described in
//! `CAPABILITY-MAP.md`. It defines the typed vocabulary that the rest of the
//! platform builds on, without touching any existing Zed behavior:
//!
//! - [`endpoint`]: a common identity, transport, authentication, capability,
//!   and health model for anything that can provide models, tools, resources,
//!   or agent threads (custom providers, remote MCP servers, LLM Farm, ACP
//!   agents, extension sources).
//! - [`provider`]: custom provider configuration, wire API selection, and
//!   model discovery for OpenAI-compatible endpoints.
//! - [`llm_farm`]: LLM Farm endpoint recognition and model discovery, treating
//!   it as a model backend rather than only an MCP server.
//! - [`mcp`]: first-class remote MCP endpoint configuration, initialize
//!   handshake parsing, and capability/tool discovery.
//! - [`permission`]: the typed permission model used by extension capability
//!   manifests, so installation is an explicit trust decision instead of a
//!   blind install.
//! - [`knowledge`]: project knowledge (RAG) collections and rules, plus the
//!   context snapshot record that powers the AI context inspector.
//!
//! Everything here is additive: types are versioned via serde and none of the
//! existing Zed provider, MCP, extension, or settings contracts are changed.
//!
//! # Trust rules
//!
//! All registry, extension, MCP, model-list, RAG, and project-rule content is
//! untrusted data. Nothing in this crate executes anything based on such
//! content. Credentials are referenced by environment variable name, never
//! stored inline in settings or manifests.

pub mod endpoint;
pub mod knowledge;
pub mod llm_farm;
pub mod mcp;
pub mod permission;
pub mod provider;
