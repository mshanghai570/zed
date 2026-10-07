//! Typed permission model for extension capability manifests.
//!
//! Extensions declare the permissions they need; the user grants a subset;
//! nothing runs until the unmet set is empty. This makes installation an
//! explicit trust decision instead of a blind install, and gives the
//! marketplace something concrete to display before install.

use serde::{Deserialize, Serialize};

/// One capability an extension asks for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "capability")]
pub enum ExtensionPermission {
    /// Outbound network access, scoped to hosts (exact names or `*.suffix`).
    Network { hosts: Vec<String> },
    /// Filesystem writes, scoped to paths (globs relative to the project).
    FileSystemWrite { paths: Vec<String> },
    /// Execute a specific command with fixed arguments.
    ProcessExec {
        command: String,
        #[serde(default)]
        args: Vec<String>,
    },
    /// Install a specific npm package.
    NpmInstall { package: String },
    /// Download a file from a host/path.
    DownloadFile { host: String, path: String },
    /// Call a tool exposed by a named MCP server.
    McpTool { server: String, tool: String },
    /// Act as / connect to a named model provider.
    ModelProvider { provider: String },
    /// Mutate editor behavior at runtime (live extensions).
    EditorMutation,
}

impl ExtensionPermission {
    /// Human-readable summary for capability review UI.
    pub fn describe(&self) -> String {
        match self {
            ExtensionPermission::Network { hosts } => {
                format!("network access to {}", hosts.join(", "))
            }
            ExtensionPermission::FileSystemWrite { paths } => {
                format!("write files matching {}", paths.join(", "))
            }
            ExtensionPermission::ProcessExec { command, args } => {
                if args.is_empty() {
                    format!("run `{command}`")
                } else {
                    format!("run `{command} {}`", args.join(" "))
                }
            }
            ExtensionPermission::NpmInstall { package } => format!("npm install {package}"),
            ExtensionPermission::DownloadFile { host, path } => {
                format!("download from {host}/{path}")
            }
            ExtensionPermission::McpTool { server, tool } => {
                format!("call MCP tool {server}/{tool}")
            }
            ExtensionPermission::ModelProvider { provider } => {
                format!("use model provider {provider}")
            }
            ExtensionPermission::EditorMutation => "modify editor behavior".to_string(),
        }
    }
}

/// An extension's declared capability manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct PermissionManifest {
    pub manifest_version: u32,
    pub permissions: Vec<ExtensionPermission>,
}

impl Default for PermissionManifest {
    fn default() -> Self {
        PermissionManifest {
            manifest_version: 1,
            permissions: Vec::new(),
        }
    }
}

/// Whether `granted` covers `required`.
///
/// Exact permissions always match. Scoped permissions also match when the
/// grant is broader: a network grant covers any required host it subsumes,
/// and a filesystem grant covers any required path it subsumes.
fn covers(granted: &ExtensionPermission, required: &ExtensionPermission) -> bool {
    match (granted, required) {
        (
            ExtensionPermission::Network { hosts: granted_hosts },
            ExtensionPermission::Network { hosts: required_hosts },
        ) => required_hosts
            .iter()
            .all(|required| granted_hosts.iter().any(|granted| host_matches(granted, required))),
        (
            ExtensionPermission::FileSystemWrite {
                paths: granted_paths,
            },
            ExtensionPermission::FileSystemWrite {
                paths: required_paths,
            },
        ) => required_paths
            .iter()
            .all(|required| granted_paths.iter().any(|granted| path_covers(granted, required))),
        _ => granted == required,
    }
}

/// `*.example.com` covers `api.example.com`; exact strings match themselves.
fn host_matches(granted: &str, required: &str) -> bool {
    if let Some(suffix) = granted.strip_prefix("*.") {
        required.ends_with(suffix) || required == suffix
    } else {
        granted == required
    }
}

/// A grant glob covers a required path if the grant is a prefix (directory
/// scope) or an exact match.
fn path_covers(granted: &str, required: &str) -> bool {
    granted == required
        || granted
            .strip_suffix("/**")
            .is_some_and(|prefix| required.starts_with(prefix))
        || granted.ends_with('*') && required.starts_with(granted.trim_end_matches('*'))
}

/// The permissions in `required` that are not covered by `granted`.
///
/// If this is non-empty, the extension must not run with only `granted`.
pub fn unmet_permissions(
    required: &[ExtensionPermission],
    granted: &[ExtensionPermission],
) -> Vec<ExtensionPermission> {
    required
        .iter()
        .filter(|requirement| !granted.iter().any(|grant| covers(grant, requirement)))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_grant_covers_exact_requirement() {
        let required = vec![ExtensionPermission::ProcessExec {
            command: "git".into(),
            args: vec!["status".into()],
        }];
        let granted = required.clone();
        assert!(unmet_permissions(&required, &granted).is_empty());
    }

    #[test]
    fn reports_unmet_permissions() {
        let required = vec![
            ExtensionPermission::ProcessExec {
                command: "git".into(),
                args: vec![],
            },
            ExtensionPermission::EditorMutation,
        ];
        let granted = vec![ExtensionPermission::ProcessExec {
            command: "git".into(),
            args: vec![],
        }];
        let unmet = unmet_permissions(&required, &granted);
        assert_eq!(unmet, vec![ExtensionPermission::EditorMutation]);
    }

    #[test]
    fn wildcard_network_grant_covers_subdomain() {
        let required = vec![ExtensionPermission::Network {
            hosts: vec!["api.example.com".into()],
        }];
        let granted = vec![ExtensionPermission::Network {
            hosts: vec!["*.example.com".into()],
        }];
        assert!(unmet_permissions(&required, &granted).is_empty());
    }

    #[test]
    fn narrower_network_grant_does_not_cover_broader_requirement() {
        let required = vec![ExtensionPermission::Network {
            hosts: vec!["*.example.com".into()],
        }];
        let granted = vec![ExtensionPermission::Network {
            hosts: vec!["api.example.com".into()],
        }];
        assert!(!unmet_permissions(&required, &granted).is_empty());
    }

    #[test]
    fn directory_filesystem_grant_covers_nested_path() {
        let required = vec![ExtensionPermission::FileSystemWrite {
            paths: vec!["build/output/bin".into()],
        }];
        let granted = vec![ExtensionPermission::FileSystemWrite {
            paths: vec!["build/**".into()],
        }];
        assert!(unmet_permissions(&required, &granted).is_empty());
    }

    #[test]
    fn filesystem_grant_outside_scope_does_not_cover() {
        let required = vec![ExtensionPermission::FileSystemWrite {
            paths: vec!["build/output".into()],
        }];
        let granted = vec![ExtensionPermission::FileSystemWrite {
            paths: vec!["dist/**".into()],
        }];
        assert!(!unmet_permissions(&required, &granted).is_empty());
    }

    #[test]
    fn manifest_roundtrips_through_serde() {
        let manifest = PermissionManifest {
            manifest_version: 1,
            permissions: vec![
                ExtensionPermission::Network {
                    hosts: vec!["registry.npmjs.org".into()],
                },
                ExtensionPermission::NpmInstall {
                    package: "prettier".into(),
                },
                ExtensionPermission::McpTool {
                    server: "llm-farm".into(),
                    tool: "list_models".into(),
                },
                ExtensionPermission::EditorMutation,
            ],
        };
        let json = serde_json::to_string(&manifest).unwrap();
        let back: PermissionManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(manifest, back);
    }
}
