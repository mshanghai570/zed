
## User-ranked backlog

The user's priority order supersedes the generic article order. Build the smallest useful slices in this order:

1. **GitLens-class Git intelligence:** enhanced blame, file history, commit visualization, line authorship, and code lens. Prefer native Zed Git data plus focused UI. Do not port the whole VS Code extension if native primitives cover it.
2. **GitHub Pull Requests & Issues:** browse, review, comment, and manage PRs/issues in the editor. Keep GitHub auth and network permissions explicit.
3. **AI-agent UX:** inline chat, explain, fix, refactor, test generation, plan mode, tool calls, diffs, approvals, and command execution. Reuse the fork's provider, MCP, ACP, context-inspector, and permission systems.
4. **Continue-style provider UX:** local/custom providers, model switching, context selection, and agent workflows. This should become a native Zed feature where possible, not a dependency on the Continue extension.
5. **Editor signal and navigation:** Error Lens, Todo Tree, Bookmarks, TODO Highlight, Path Intellisense, Better Comments, Regex Previewer, and Project Manager behavior.
6. **API and project tooling:** Thunder Client, REST Client, SQLite Viewer, YAML/JSON schema tooling, Docker, Makefile Tools, CMake Tools, Dev Containers, Remote SSH, and Live Share.
7. **Formatting and language tooling:** Prettier-like workflows mapped to Zed's native formatter architecture, Code Spell Checker, Python, Swift, YAML/JSON, CMake, C/C++, Dart, Ruby, Svelte, and other language tooling.
8. **Workspace identity:** Peacock-style per-project colors and visual identity. Keep this lightweight and native.
9. **Guided code understanding:** CodeTour and related project walkthrough/context features.

## Zed Agent Extensions category

Add a first-class `Zed Agent Extensions` category. Keep these as typed capabilities, not a flat tag list:

- Agent Knowledge
- Agent Skills
- MCP Servers
- ACP Agents
- Model Router
- Local Model Runner
- Ollama
- llama.cpp
- LM Studio
- LLM Farm
- AI Context Inspector
- Prompt Library
- Agent Memory
- Agent Trace Viewer
- Tool Permission Manager
- Agent Cost / Token Monitor
- Agent Session Replay

Use one shared agent-platform substrate for identity, configuration, health, permissions, provenance, and telemetry. Add individual features only when a concrete workflow needs them. Prefer existing provider/MCP/ACP/context data over parallel storage or duplicate abstractions.

## Simplification rules for implementation

- Build native behavior before wrapping a VS Code extension.
- Add one endpoint/provider abstraction, not separate abstractions for every vendor.
- Add one permission model with typed capabilities, not per-feature approval code.
- Add one context event/snapshot model, then derive inspector, trace, replay, and cost views from it.
- Add one source adapter contract, then plug in Open VSX, ACP Registry, and MCP Registry.
- Do not build Agent Memory, Trace Viewer, Cost Monitor, or Session Replay as separate systems until the shared context/event stream exists.
- Do not port extensions whose core behavior is already native or easy to reproduce safely.
