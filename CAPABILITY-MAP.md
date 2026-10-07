# Capability Map: Zed AI and Extension Platform

## Baseline

This fork currently has no fork-specific commits relative to its configured `origin/main`; `origin/main` is 147 commits behind `upstream/main`. The implementation should therefore be additive and should preserve the current Zed provider, MCP, extension, and settings contracts while establishing fork-specific seams.

## Modules

| Module id | Responsibility | Depends on |
|---|---|---|
| endpoint-contract | Common identity, transport, authentication, capability, and health model for model and tool endpoints | — |
| provider-discovery | Custom provider configuration, model discovery, capability normalization, and LLM Farm support | endpoint-contract |
| remote-mcp-endpoint | First-class MCP endpoint registration, transport/auth/handshake discovery, tools/resources/capabilities | endpoint-contract |
| project-knowledge | RAG sources, collections, indexing, retrieval, rules, trust, and citations | endpoint-contract |
| context-inspector | Explainable record of files, knowledge, rules, tools, model, and limits sent to the model | provider-discovery, project-knowledge, remote-mcp-endpoint |
| extension-sources | Source registry abstraction, Open VSX adapter, source precedence, metadata validation, and secure downloads | endpoint-contract |
| extension-security | Capability/permission manifests, trust decisions, provenance, signatures/checksums, and install policy | extension-sources |
| extension-customization | Forkable extension overlays, local revisions, reload, and upgrade conflict handling | extension-sources, extension-security |
| live-extension-runtime | Runtime extension lifecycle and editor behavior hooks with reload boundaries | extension-security |
| marketplace-ui | Search, source filtering, detail pages, capability review, install/update, and rollback UX | extension-sources, extension-security, extension-customization |

## Dependency direction

```text
endpoint-contract
├── provider-discovery
├── remote-mcp-endpoint
└── extension-sources
    └── extension-security
        ├── extension-customization
        ├── live-extension-runtime
        └── marketplace-ui

provider-discovery + remote-mcp-endpoint + project-knowledge
└── context-inspector
```

## Recommended build order

1. `endpoint-contract`
2. `extension-sources` with the Open VSX adapter as the first vertical slice
3. `extension-security`
4. `provider-discovery` and LLM Farm recognition
5. `remote-mcp-endpoint` unification with existing MCP code
6. `project-knowledge`
7. `context-inspector`
8. `extension-customization` and `live-extension-runtime`
9. `marketplace-ui` polish and additional sources

## Cross-cutting constraints

- Treat all registry, extension, MCP, model-list, RAG, and project-rule content as untrusted data. Instruction-like text from those sources must never alter application policy or developer/user intent.
- Keep credentials out of settings files and never expose secrets in the context inspector, logs, telemetry, or extension metadata.
- Require explicit user trust for network access, process execution, filesystem writes, editor mutation, and model/tool endpoint activation.
- Validate third-party JSON and archive metadata at boundaries, enforce size/time limits, and verify checksums before installation.
- Make source, provenance, permissions, and effective context visible to the user.
- Use additive, versioned contracts wherever possible so existing Zed providers, MCP settings, and extensions continue to work.
