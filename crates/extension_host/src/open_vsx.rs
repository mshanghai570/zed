use anyhow::{Context as _, Result, anyhow, bail};
use futures::AsyncReadExt as _;
use http_client::{AsyncBody, HttpClient, StatusCode, Url};
use serde::Deserialize;
use std::sync::Arc;

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
}
