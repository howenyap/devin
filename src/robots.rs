use std::collections::HashMap;

use reqwest::Client;
use texting_robots::Robot;
use tracing::warn;
use url::Url;

/// Caches parsed robots.txt per domain and checks whether a URL is allowed.
pub struct RobotsChecker {
    cache: HashMap<String, Option<Robot>>,
    client: Client,
}

impl RobotsChecker {
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent("web-crawler/0.1.0")
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("failed to build HTTP client for robots checker");
        Self {
            cache: HashMap::new(),
            client,
        }
    }

    /// Check whether the given URL is allowed by the domain's robots.txt.
    /// Fetches and caches robots.txt on first access per origin.
    /// On failure (404, network error, parse error), assumes allowed and caches `None`.
    pub async fn is_allowed(&mut self, url: &Url) -> bool {
        let origin = url.origin().ascii_serialization();

        if !self.cache.contains_key(&origin) {
            let robots_url = format!("{}/robots.txt", origin);
            let robot = match self.client.get(&robots_url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    match resp.bytes().await {
                        Ok(body) => match Robot::new("web-crawler", &body) {
                            Ok(r) => Some(r),
                            Err(e) => {
                                warn!("failed to parse robots.txt for {}: {}", origin, e);
                                None
                            }
                        },
                        Err(e) => {
                            warn!("failed to read robots.txt body for {}: {}", origin, e);
                            None
                        }
                    }
                }
                Ok(_) => {
                    // Non-success status (e.g. 404) — assume allowed.
                    None
                }
                Err(e) => {
                    warn!("failed to fetch robots.txt for {}: {}", origin, e);
                    None
                }
            };
            self.cache.insert(origin.clone(), robot);
        }

        match self.cache.get(&origin) {
            Some(Some(robot)) => robot.allowed(url.as_str()),
            _ => true, // No robots.txt or parse failure — assume allowed.
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn robots_txt_parsing_and_check() {
        let txt = b"User-Agent: web-crawler\nDisallow: /secret\nAllow: /public\n";
        let robot = Robot::new("web-crawler", txt).unwrap();

        assert!(robot.allowed("https://example.com/public"));
        assert!(!robot.allowed("https://example.com/secret"));
        assert!(robot.allowed("https://example.com/other"));
    }

    #[test]
    fn robots_txt_wildcard_disallow_all() {
        let txt = b"User-Agent: *\nDisallow: /\n";
        let robot = Robot::new("web-crawler", txt).unwrap();

        assert!(!robot.allowed("https://example.com/anything"));
        assert!(!robot.allowed("https://example.com/"));
    }

    #[test]
    fn robots_txt_empty_allows_all() {
        let txt = b"";
        let robot = Robot::new("web-crawler", txt).unwrap();

        assert!(robot.allowed("https://example.com/anything"));
    }
}
