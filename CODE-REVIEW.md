# Code Review — Zed Fork Delta

**Scope:** `origin/main...feature/open-vsx-source` (574 added lines), focused on correctness, security, performance, and test coverage. This is not a line-by-line review of upstream Zed.

## Verdict

**Request changes before calling Open VSX production-ready.** The code compiles and focused tests pass, but installation is intentionally incomplete and the default UI adds an external network request.

## Findings

### Major — Open VSX is queried by default on every marketplace refresh

`ExtensionsPage::fetch_extensions` starts `fetch_open_vsx_extensions` for every search, while `ExtensionSettings::open_vsx_source()` returns the official registry when the setting is absent.

Impact: slower marketplace loads, extra rate-limit/network traffic, and a third-party registry becomes part of the default request path.

**Recommendation:** make the source opt-in (`open_vsx_registry: Some(...)`) or add an explicit marketplace source toggle. Preserve the official URL as a documented example/default setting, not an implicit network behavior.

### Major — Open VSX results are visible but not installable

External cards intentionally have no handlers. This is safe, but users may interpret marketplace results as supported installable extensions.

**Recommendation:** label cards clearly as `External / Open VSX — preview only` until VSIX manifest parsing, capability review, compatibility mapping, and a source-aware installer exist. Do not route these IDs into `install_latest_extension`, which only addresses Zed’s API.

### Major — No VSIX trust boundary exists yet

The adapter validates metadata URLs but does not inspect the downloaded VSIX, publisher identity, extension manifest, permissions, or embedded code before execution.

**Recommendation:** before enabling install: download with size limits, inspect ZIP paths for traversal, parse `extension/package.json`, show requested capabilities, verify publisher/source, and install only after explicit approval.

### Minor — External metadata URL is presented as a repository link

`for_open_vsx` uses the registry metadata URL as `repository_url`, so the card’s repository icon/tooltip says “Visit Extension Repository” but opens registry metadata.

**Recommendation:** add a distinct source/details link or use the manifest repository field after fetching full metadata.

### Minor — Search result validation is stricter than custom-source configuration

Custom registries may use HTTP for metadata, but search entries require HTTPS URLs. This is a reasonable security default, but the error should be surfaced in source diagnostics rather than silently converting all external failures to an empty result.

## Positive findings

- Open VSX responses are size-bounded before JSON parsing.
- Namespace/name path injection is rejected.
- Download and metadata URLs are HTTPS-validated before use.
- External cards cannot invoke Zed’s installer.
- Focused tests cover URL validation, malformed transport behavior, pagination, search parsing, and metadata retrieval.
- Changes are split into small atomic commits and the working tree was clean before this review.

## Verification evidence

- `cargo check -p extensions_ui` — passed.
- `cargo test -p extension_host open_vsx` — **7 passed, 0 failed**.
- `git diff --check` — passed before the last commits.

## Recommended next order

1. Make Open VSX opt-in or add a visible source toggle.
2. Add source attribution/preview-only UI.
3. Implement VSIX inspection and capability manifest review.
4. Add source-aware installation with compatibility checks.
5. Add integration tests for settings, UI source selection, and failed external requests.
