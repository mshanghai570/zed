
## User-ranked implementation backlog

The marketplace roadmap is now product-led, not article-led. Start with GitLens-class Git intelligence, then GitHub PR/issues, AI-agent UX, Continue-style provider/context workflows, editor signal/navigation, API and development-environment tooling, language/tooling support, workspace identity, and guided code understanding.

The fork should implement these natively where its existing editor, provider, MCP, ACP, and context systems already provide the necessary primitives. Port an external extension only when native implementation is not smaller, safer, or more maintainable.

Create a `Zed Agent Extensions` category covering Agent Knowledge, Agent Skills, MCP Servers, ACP Agents, Model Router, Local Model Runner, Ollama, llama.cpp, LM Studio, LLM Farm, AI Context Inspector, Prompt Library, Agent Memory, Agent Trace Viewer, Tool Permission Manager, Agent Cost/Token Monitor, and Agent Session Replay. Keep the category as a catalog and capability taxonomy. Do not create separate subsystems for every label.

Use these simplifying boundaries:

- One shared endpoint/provider contract.
- One typed permission model.
- One context snapshot/event model that can later power inspection, tracing, replay, and cost views.
- One external-source adapter contract for Open VSX, ACP Registry, and MCP Registry.
- Native Zed behavior before VS Code compatibility wrappers.
- No speculative Agent Memory, Trace Viewer, Cost Monitor, or Session Replay implementation until the shared event model has a concrete consumer.
