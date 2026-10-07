# Endpoint Contract

Implementation of the `endpoint-contract` module from `CAPABILITY-MAP.md`, as a
self-contained crate: `crates/endpoint_contract`.

## Why this exists

The fork's AI layer needs one shared vocabulary before the rest of the roadmap
can be built without duplicating itself:

- Custom providers, remote MCP servers, LLM Farm, ACP agents, and extension
  sources are all *endpoints*. An endpoint can provide models, tools, or both;
  it should not be forced into one narrow category (MCP-as-provider
  recognition).
- Extension installation needs a typed permission model so capability review
  is an explicit trust decision, not a blind install.
- The agent needs user-defined project knowledge (RAG collections + rules).
- The user needs to see exactly what was sent to the model (context inspector).

This crate defines those contracts only. It changes no existing Zed behavior,
registers no settings, and performs no network I/O. Everything is additive and
serde-versioned so existing providers, MCP settings, and extensions keep
working.

## Modules

| Module | Contents |
|---|---|
| `endpoint` | `EndpointKind`, `EndpointTransport` (Streamable HTTP first, legacy SSE/stdio), `EndpointAuth` (secrets by env-var reference only), `EndpointCapabilities`, `EndpointState`, `EndpointDescriptor` with validation |
| `provider` | `CustomProviderConfig`, `WireApi`, `DiscoveredModel`, `ModelDiscovery` trait, OpenAI-compatible `/v1/models` parsing |
| `llm_farm` | `LlmFarmConfig` (default port 9090), `GgufModelMetadata` (quantization, parameter count, file size, context length, source repo), model-list parsing that never invents capabilities |
| `mcp` | `RemoteMcpEndpointConfig`, `parse_initialize_result` for the MCP handshake, `HandshakeReport` with server info, capabilities, tools, and session id |
| `permission` | `ExtensionPermission`, `PermissionManifest`, and `unmet_permissions` with scoped grant matching (wildcard hosts, directory globs) |
| `knowledge` | `KnowledgeCollection`, `KnowledgeSource`, `KnowledgeRule`, `ProjectKnowledgeConfig`, and `ContextSnapshot`/`ContextSnapshotEntry` for the context inspector |

## Integration steps (pending)

The crate is currently standalone so it can be reviewed and tested before it
joins the workspace build:

1. Add `crates/endpoint_contract` to `[workspace] members` in the root
   `Cargo.toml`.
2. Add `endpoint_contract = { path = "crates/endpoint_contract" }` to
   `[workspace.dependencies]` in the root `Cargo.toml`.
3. Delete the `[workspace]` table at the bottom of
   `crates/endpoint_contract/Cargo.toml` (it exists only so the crate builds
   standalone in the meantime).
4. Switch the crate's dependencies to `.workspace = true` once it is a
   member.

Build and test standalone:

```
cargo test --manifest-path crates/endpoint_contract/Cargo.toml
```

## Downstream consumers (next slices)

- `provider-discovery`: adopt `CustomProviderConfig` + `ModelDiscovery` in the
  custom provider settings path; add an LLM Farm provider using
  `LlmFarmConfig`.
- `remote-mcp-endpoint`: back remote MCP settings with
  `RemoteMcpEndpointConfig`; store `HandshakeReport` per server so tool
  discovery is visible and cheap to re-read.
- `extension-security`: validate extension manifests against
  `PermissionManifest` before install; show `describe()` output in the review
  UI; gate execution on `unmet_permissions(...).is_empty()`.
- `project-knowledge` / `context-inspector`: map `ProjectKnowledgeConfig` to
  indexed collections; emit a `ContextSnapshot` per request and render it in
  the inspector.

## Security constraints

- All registry, extension, MCP, model-list, RAG, and rule content is
  untrusted. Nothing in this crate executes based on such content.
- Credentials are referenced by environment variable name; they never appear
  in settings files, manifests, snapshots, or logs.
- Capability and permission matching is conservative: a grant covers a
  requirement only on exact match or a strictly broader scope.
