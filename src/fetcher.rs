use reqwest::Client;
use url::Url;

/// Fetches raw HTML content from a URL.
pub struct Fetcher {
    client: Client,
}

impl Fetcher {
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent("web-crawler/0.1.0")
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("failed to build HTTP client");
        Self { client }
    }

    /// Fetch the HTML body from the given URL.
    /// Returns `Ok(html_string)` on success.
    pub async fn fetch(&self, url: &Url) -> Result<String, FetchError> {
        let response = self
            .client
            .get(url.as_str())
            .send()
            .await
            .map_err(FetchError::Network)?;

        let status = response.status();
        if !status.is_success() {
            return Err(FetchError::HttpStatus(status.as_u16()));
        }

        let body = response.text().await.map_err(FetchError::Network)?;
        Ok(body)
    }
}

#[derive(Debug)]
pub enum FetchError {
    Network(reqwest::Error),
    HttpStatus(u16),
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FetchError::Network(e) => write!(f, "network error: {e}"),
            FetchError::HttpStatus(code) => write!(f, "HTTP {code}"),
        }
    }
}

impl std::error::Error for FetchError {}
