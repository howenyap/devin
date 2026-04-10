use std::path::Path;
use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::fetcher::Fetcher;
use crate::parser;
use crate::storage::{CrawlRecord, Storage};

/// Shared crawler state, protected by a mutex for concurrent access.
pub struct CrawlerState {
    pub frontier: crate::frontier::Frontier,
    pub storage: Storage,
    pub pages_crawled: usize,
    pub max_pages: usize,
    pub running: bool,
}

impl CrawlerState {
    pub fn new(output_path: &Path, max_pages: usize) -> std::io::Result<Self> {
        Ok(Self {
            frontier: crate::frontier::Frontier::new(),
            storage: Storage::new(output_path)?,
            pages_crawled: 0,
            max_pages,
            running: false,
        })
    }
}

/// Run the crawl loop: pop URLs from the frontier, fetch, parse, store results,
/// and add discovered URLs back to the frontier.
pub async fn crawl_loop(state: Arc<Mutex<CrawlerState>>) {
    // The fetcher lives outside the mutex — reqwest::Client is cheap to clone
    // and we don't want to hold the lock across network I/O.
    let fetcher = Fetcher::new();

    loop {
        // Grab the next URL while holding the lock briefly.
        let (url, pages_crawled, max_pages) = {
            let mut s = state.lock().await;
            if !s.running {
                info!("crawler stopped");
                break;
            }
            if s.pages_crawled >= s.max_pages {
                info!("reached max pages limit ({}), stopping", s.max_pages);
                s.running = false;
                break;
            }
            match s.frontier.pop() {
                Some(url) => (url, s.pages_crawled, s.max_pages),
                None => {
                    info!("frontier empty, stopping");
                    s.running = false;
                    break;
                }
            }
        };

        info!("[{}/{}] crawling: {}", pages_crawled + 1, max_pages, url);

        // Fetch the page — no lock held during network I/O.
        let html = match fetcher.fetch(&url).await {
            Ok(html) => html,
            Err(e) => {
                warn!("failed to fetch {}: {}", url, e);
                let mut s = state.lock().await;
                s.pages_crawled += 1;
                continue;
            }
        };

        // Parse the page — also outside the lock.
        let result = parser::parse(&html, &url);

        // Store the result and enqueue new URLs.
        {
            let mut s = state.lock().await;
            s.pages_crawled += 1;

            let record = CrawlRecord {
                url: url.to_string(),
                title: result.title,
                links_found: result.links.len(),
                timestamp: now_unix(),
            };

            if let Err(e) = s.storage.write_record(&record) {
                warn!("failed to write record for {}: {}", url, e);
            }

            let mut added = 0;
            for link in result.links {
                if s.frontier.push(link) {
                    added += 1;
                }
            }

            info!(
                "parsed {}: found {} links, {} new",
                url, record.links_found, added
            );
        }
    }
}

fn now_unix() -> String {
    use std::time::SystemTime;
    let d = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}s", d.as_secs())
}
