use anyhow::{Context as _, Result, anyhow, bail};
use futures::AsyncReadExt as _;
use http_client::{AsyncBody, HttpClient, StatusCode, Url};
use serde::Deserialize;
use std::{collections::BTreeMap, sync::Arc};

const MAX_METADATA_BYTES: usize = 2 * 1024 * 1024;

/// Read-only client for the public Open VSX registry API.
///
/// This slice only fetches and validates metadata. It does not install or execute
/// an extension. Installation must remain behind the existing permission review.
pub struct OpenVsxSource {
    base_url: Url,
}

impl OpenVsxSource {
    pub fn new(base_url: Url) -> Result<Self> {
        match base_url.scheme() {
            "https" | "http" => {}
            scheme => bail!("Open VSX source must use HTTP or HTTPS, got {scheme:?}"),
        }

        Ok(Self { base_url })
    }

    pub fn official() -> Self {
        Self {
            base_url: Url::parse("https://open-vsx.org/").expect("official Open VSX URL is valid"),
        }
    }

    pub fn extension_url(&self, namespace: &str, name: &str) -> Result<Url> {
        validate_segment(namespace, "namespace")?;
        validate_segment(name, "extension name")?;

        self.base_url
            .join(&format!("api/{namespace}/{name}"))
            .context("building Open VSX extension URL")
    }

    pub async fn fetch_extension(
        &self,
        client: Arc<dyn HttpClient>,
        namespace: &str,
        name: &str,
    ) -> Result<OpenVsxExtension> {
        let url = self.extension_url(namespace, name)?;
        let mut response = client
            .get(url.as_str(), AsyncBody::empty(), true)
            .await
            .context("requesting Open VSX extension metadata")?;

        if response.status() != StatusCode::OK {
            bail!("Open VSX returned HTTP {}", response.status().as_u16());
        }

        let mut body = Vec::new();
        response
            .body_mut()
            .take((MAX_METADATA_BYTES + 1) as u64)
            .read_to_end(&mut body)
            .await
            .context("reading Open VSX extension metadata")?;

        if body.len() > MAX_METADATA_BYTES {
            bail!("Open VSX metadata exceeds {MAX_METADATA_BYTES} bytes");
        }

        let metadata: OpenVsxExtension =
            serde_json::from_slice(&body).context("parsing Open VSX extension metadata")?;
        metadata.validate()
    }

    pub async fn search(
        &self,
        client: Arc<dyn HttpClient>,
        query: Option<&str>,
        size: u32,
        offset: u32,
    ) -> Result<OpenVsxSearchResult> {
        if size > 1000 {
            bail!("Open VSX search size must not exceed 1000");
        }

        let mut url = self.base_url.join("api/-/search")?;
        {
            let mut params = url.query_pairs_mut();
            if let Some(query) = query.filter(|query| !query.trim().is_empty()) {
                params.append_pair("query", query);
            }
            params
                .append_pair("size", &size.to_string())
                .append_pair("offset", &offset.to_string());
        }

        let mut response = client
            .get(url.as_str(), AsyncBody::empty(), true)
            .await
            .context("requesting Open VSX search results")?;
        if response.status() != StatusCode::OK {
            bail!("Open VSX returned HTTP {}", response.status().as_u16());
        }

        let mut body = Vec::new();
        response
            .body_mut()
            .take((MAX_METADATA_BYTES + 1) as u64)
            .read_to_end(&mut body)
            .await
            .context("reading Open VSX search results")?;
        if body.len() > MAX_METADATA_BYTES {
            bail!("Open VSX search results exceed {MAX_METADATA_BYTES} bytes");
        }

        let result: OpenVsxSearchResult =
            serde_json::from_slice(&body).context("parsing Open VSX search results")?;
        result.validate()
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct OpenVsxExtension {
    pub namespace: String,
    pub name: String,
    pub version: String,
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub engines: Option<serde_json::Value>,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub download_count: Option<u64>,
    #[serde(default)]
    pub files: OpenVsxFiles,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct OpenVsxFiles {
    pub download: Option<String>,
    pub readme: Option<String>,
    pub manifest: Option<String>,
}

impl OpenVsxExtension {
    fn validate(self) -> Result<Self> {
        validate_segment(&self.namespace, "namespace")?;
        validate_segment(&self.name, "extension name")?;
        if self.version.trim().is_empty() {
            return Err(anyhow!("Open VSX metadata has an empty version"));
        }

        let download = self
            .files
            .download
            .as_deref()
            .ok_or_else(|| anyhow!("Open VSX metadata has no download URL"))?;
        let download_url = Url::parse(download).context("parsing Open VSX download URL")?;
        if download_url.scheme() != "https" {
            return Err(anyhow!("Open VSX download URL must use HTTPS"));
        }

        Ok(self)
    }

    pub fn id(&self) -> String {
        format!("{}.{}", self.namespace, self.name)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct OpenVsxSearchResult {
    pub offset: u32,
    #[serde(rename = "totalSize")]
    pub total_size: u32,
    pub extensions: Vec<OpenVsxSearchEntry>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct OpenVsxSearchEntry {
    pub url: String,
    pub files: BTreeMap<String, String>,
    pub name: String,
    pub namespace: String,
    pub version: String,
    pub timestamp: String,
    pub verified: bool,
    #[serde(default, rename = "downloadCount")]
    pub download_count: Option<u32>,
    #[serde(default, rename = "displayName")]
    pub display_name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub deprecated: bool,
}

impl OpenVsxSearchResult {
    fn validate(self) -> Result<Self> {
        for extension in &self.extensions {
            validate_segment(&extension.namespace, "namespace")?;
            validate_segment(&extension.name, "extension name")?;
            if extension.version.trim().is_empty() || extension.timestamp.trim().is_empty() {
                bail!("Open VSX search result has incomplete version metadata");
            }
            validate_https_url(&extension.url, "extension metadata URL")?;
            if let Some(download) = extension.files.get("download") {
                validate_https_url(download, "extension download URL")?;
            }
        }
        Ok(self)
    }
}

fn validate_https_url(value: &str, label: &str) -> Result<()> {
    let url = Url::parse(value).with_context(|| format!("parsing Open VSX {label}"))?;
    if url.scheme() != "https" {
        bail!("Open VSX {label} must use HTTPS");
    }
    Ok(())
}

fn validate_segment(value: &str, label: &str) -> Result<()> {
    if value.is_empty()
        || value.contains('/')
        || value.contains('\\')
        || value.contains('?')
        || value.contains('#')
    {
        bail!("invalid Open VSX {label}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_client::{FakeHttpClient, Response};

    const METADATA: &str = r#"{
        "namespace": "Anthropic",
        "name": "claude-code",
        "version": "2.1.292",
        "displayName": "Claude Code for VS Code",
        "description": "AI coding agent",
        "license": "proprietary",
        "files": {
            "download": "https://open-vsx.org/api/Anthropic/claude-code/2.1.292/file/Anthropic.claude-code-2.1.292.vsix"
        }
    }"#;

    #[test]
    fn builds_extension_url_without_accepting_path_injection() {
        let source = OpenVsxSource::official();
        assert_eq!(
            source.extension_url("Anthropic", "claude-code").unwrap(),
            "https://open-vsx.org/api/Anthropic/claude-code"
                .parse::<Url>()
                .unwrap()
        );
        assert!(
            source
                .extension_url("Anthropic/other", "claude-code")
                .is_err()
        );
    }

    #[test]
    fn validates_metadata_and_exposes_stable_id() {
        let metadata: OpenVsxExtension = serde_json::from_str(METADATA).unwrap();
        let metadata = metadata.validate().unwrap();
        assert_eq!(metadata.id(), "Anthropic.claude-code");
    }

    #[test]
    fn rejects_http_downloads() {
        let mut metadata: OpenVsxExtension = serde_json::from_str(METADATA).unwrap();
        metadata.files.download = Some("http://example.com/file.vsix".into());
        assert!(metadata.validate().is_err());
    }

    const SEARCH: &str = r#"{
        "offset": 18,
        "totalSize": 42,
        "extensions": [{
            "url": "https://open-vsx.org/api/Anthropic/claude-code",
            "files": {"download": "https://open-vsx.org/api/Anthropic/claude-code/2.1.292/file/Anthropic.claude-code-2.1.292.vsix"},
            "name": "claude-code",
            "namespace": "Anthropic",
            "version": "2.1.292",
            "timestamp": "2026-10-06T00:00:00Z",
            "verified": true,
            "downloadCount": 1234,
            "displayName": "Claude Code"
        }]
    }"#;

    #[test]
    fn validates_search_results() {
        let result: OpenVsxSearchResult = serde_json::from_str(SEARCH).unwrap();
        let result = result.validate().unwrap();
        assert_eq!(result.offset, 18);
        assert_eq!(result.total_size, 42);
        assert_eq!(result.extensions[0].namespace, "Anthropic");
    }

    #[gpui::test]
    async fn fetches_metadata_with_fake_http(executor: gpui::BackgroundExecutor) {
        let body = METADATA.as_bytes().to_vec();
        let client = FakeHttpClient::create(move |request| {
            assert_eq!(request.uri().path(), "/api/Anthropic/claude-code");
            let body = body.clone();
            async move { Ok(Response::builder().status(200).body(body.into()).unwrap()) }
        });

        let metadata = OpenVsxSource::official()
            .fetch_extension(client, "Anthropic", "claude-code")
            .await
            .unwrap();
        assert_eq!(metadata.id(), "Anthropic.claude-code");
        drop(executor);
    }

    #[gpui::test]
    async fn searches_with_query_and_pagination(executor: gpui::BackgroundExecutor) {
        let body = SEARCH.as_bytes().to_vec();
        let client = FakeHttpClient::create(move |request| {
            assert_eq!(request.uri().path(), "/api/-/search");
            assert_eq!(
                request.uri().query(),
                Some("query=claude+code&size=18&offset=18")
            );
            let body = body.clone();
            async move { Ok(Response::builder().status(200).body(body.into()).unwrap()) }
        });

        let result = OpenVsxSource::official()
            .search(client, Some("claude code"), 18, 18)
            .await
            .unwrap();
        assert_eq!(result.extensions[0].name, "claude-code");
        drop(executor);
    }

    #[gpui::test]
    async fn rejects_oversized_search_page(executor: gpui::BackgroundExecutor) {
        let client = FakeHttpClient::with_200_response();
        let error = OpenVsxSource::official()
            .search(client, None, 1001, 0)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("must not exceed 1000"));
        drop(executor);
    }
}
