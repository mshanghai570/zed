//! Project knowledge (RAG) collections, rules, and the context snapshot.
//!
//! This is the vocabulary for two features:
//!
//! - **Project knowledge**: user-defined collections of files/globs that
//!   ground the agent in the project's own material, plus rule files.
//! - **Context inspector**: a [`ContextSnapshot`] records exactly what was
//!   sent to the model — files, knowledge, rules, tools, system prompt — so
//!   "what did the model actually see" is inspectable instead of implicit.
//!
//! Rule and knowledge content is untrusted text: it is context for the model,
//! never policy for the application.

use serde::{Deserialize, Serialize};

/// One source inside a knowledge collection: a glob relative to the project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct KnowledgeSource {
    /// Glob relative to the worktree root, e.g. `docs/**/*.md`.
    pub path: String,
    pub description: Option<String>,
}

impl Default for KnowledgeSource {
    fn default() -> Self {
        KnowledgeSource {
            path: String::new(),
            description: None,
        }
    }
}

/// A named, user-defined collection of knowledge sources.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct KnowledgeCollection {
    pub id: String,
    pub display_name: String,
    pub sources: Vec<KnowledgeSource>,
}

impl Default for KnowledgeCollection {
    fn default() -> Self {
        KnowledgeCollection {
            id: String::new(),
            display_name: String::new(),
            sources: Vec::new(),
        }
    }
}

/// A rules file whose contents are injected as agent rules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct KnowledgeRule {
    /// Path relative to the worktree root, e.g. `.rules/architecture.md`.
    pub path: String,
    pub title: Option<String>,
}

impl Default for KnowledgeRule {
    fn default() -> Self {
        KnowledgeRule {
            path: String::new(),
            title: None,
        }
    }
}

/// Project-level knowledge configuration.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct ProjectKnowledgeConfig {
    pub collections: Vec<KnowledgeCollection>,
    pub rules: Vec<KnowledgeRule>,
}

impl ProjectKnowledgeConfig {
    pub fn collection(&self, id: &str) -> Option<&KnowledgeCollection> {
        self.collections.iter().find(|collection| collection.id == id)
    }

    pub fn total_sources(&self) -> usize {
        self.collections.iter().map(|c| c.sources.len()).sum()
    }
}

/// What kind of material an entry in the model's context is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextEntryKind {
    File,
    Knowledge,
    Rule,
    Tool,
    SystemPrompt,
    UserMessage,
}

/// One entry that was (or will be) sent to the model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct ContextSnapshotEntry {
    pub kind: ContextEntryKind,
    pub label: String,
    pub detail: Option<String>,
    pub approximate_tokens: Option<u64>,
}

impl Default for ContextSnapshotEntry {
    fn default() -> Self {
        ContextSnapshotEntry {
            kind: ContextEntryKind::File,
            label: String::new(),
            detail: None,
            approximate_tokens: None,
        }
    }
}

/// The record of everything fed to the model for one request, powering the
/// AI context inspector.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct ContextSnapshot {
    pub provider_id: String,
    pub model_id: String,
    pub entries: Vec<ContextSnapshotEntry>,
    pub context_window: Option<u64>,
    pub reserved_for_output: Option<u64>,
}

impl ContextSnapshot {
    pub fn total_entries(&self) -> usize {
        self.entries.len()
    }

    pub fn approximate_tokens(&self) -> u64 {
        self.entries.iter().filter_map(|e| e.approximate_tokens).sum()
    }

    /// Count of entries per kind, for a summary view.
    pub fn entry_counts(&self) -> Vec<(ContextEntryKind, usize)> {
        let mut counts: Vec<(ContextEntryKind, usize)> = Vec::new();
        for entry in &self.entries {
            match counts.iter_mut().find(|(kind, _)| *kind == entry.kind) {
                Some((_, count)) => *count += 1,
                None => counts.push((entry.kind, 1)),
            }
        }
        counts
    }

    /// How much of the context window the entries approximately consume.
    pub fn context_usage(&self) -> Option<f64> {
        self.context_window.map(|window| {
            let tokens = self.approximate_tokens() as f64;
            let effective = self
                .reserved_for_output
                .map(|reserved| window.saturating_sub(reserved))
                .filter(|effective| *effective > 0)
                .unwrap_or(window) as f64;
            if effective <= 0.0 {
                0.0
            } else {
                (tokens / effective).clamp(0.0, 1.0)
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> ProjectKnowledgeConfig {
        ProjectKnowledgeConfig {
            collections: vec![KnowledgeCollection {
                id: "architecture".into(),
                display_name: "Architecture".into(),
                sources: vec![
                    KnowledgeSource {
                        path: "docs/architecture.md".into(),
                        description: Some("system overview".into()),
                    },
                    KnowledgeSource {
                        path: "docs/decisions/*.md".into(),
                        description: None,
                    },
                ],
            }],
            rules: vec![KnowledgeRule {
                path: ".rules/swift.md".into(),
                title: Some("Swift conventions".into()),
            }],
        }
    }

    #[test]
    fn finds_collection_by_id() {
        let config = config();
        assert_eq!(config.collection("architecture").unwrap().sources.len(), 2);
        assert!(config.collection("missing").is_none());
        assert_eq!(config.total_sources(), 2);
    }

    #[test]
    fn snapshot_counts_entries_and_tokens() {
        let snapshot = ContextSnapshot {
            provider_id: "llm-farm".into(),
            model_id: "qwen2.5-coder".into(),
            entries: vec![
                ContextSnapshotEntry {
                    kind: ContextEntryKind::File,
                    label: "src/main.rs".into(),
                    detail: None,
                    approximate_tokens: Some(400),
                },
                ContextSnapshotEntry {
                    kind: ContextEntryKind::Rule,
                    label: ".rules/swift.md".into(),
                    detail: None,
                    approximate_tokens: Some(100),
                },
                ContextSnapshotEntry {
                    kind: ContextEntryKind::Tool,
                    label: "mcp_llm_farm_list_models".into(),
                    detail: None,
                    approximate_tokens: None,
                },
            ],
            context_window: Some(32_768),
            reserved_for_output: Some(4_096),
        };
        assert_eq!(snapshot.total_entries(), 3);
        assert_eq!(snapshot.approximate_tokens(), 500);
        let counts = snapshot.entry_counts();
        assert_eq!(counts.len(), 3);
        let usage = snapshot.context_usage().unwrap();
        assert!(usage > 0.0 && usage < 0.05);
    }

    #[test]
    fn snapshot_usage_clamps_when_over_window() {
        let snapshot = ContextSnapshot {
            provider_id: "p".into(),
            model_id: "m".into(),
            entries: vec![ContextSnapshotEntry {
                kind: ContextEntryKind::File,
                label: "huge.txt".into(),
                detail: None,
                approximate_tokens: Some(1_000_000),
            }],
            context_window: Some(1_000),
            reserved_for_output: None,
        };
        assert_eq!(snapshot.context_usage().unwrap(), 1.0);
    }
}
