use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::fetcher::Fetcher;
use crate::parser;
use crate::robots::RobotsChecker;
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
    let fetcher = Fetcher::new();
    let mut robots_checker = RobotsChecker::new();
    let mut last_fetch_times: HashMap<String, tokio::time::Instant> = HashMap::new();

    loop {
        // Build the set of currently blocked domains (those fetched < 1s ago)
        let now = tokio::time::Instant::now();
        let blocked_domains: HashSet<String> = last_fetch_times
            .iter()
            .filter(|(_, &t)| now.duration_since(t) < Duration::from_secs(1))
            .map(|(d, _)| d.clone())
            .collect();

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
            match s.frontier.pop(&blocked_domains) {
                Some(url) => (url, s.pages_crawled, s.max_pages),
                None => {
                    // Check if frontier is truly empty or just all domains blocked
                    if s.frontier.pending() == 0 {
                        info!("frontier empty, stopping");
                        s.running = false;
                        break;
                    }
                    // All domains are rate-limited; drop the lock and sleep
                    // until the earliest domain becomes available
                    drop(s);
                    let min_wait = last_fetch_times
                        .iter()
                        .filter(|(d, _)| blocked_domains.contains(d.as_str()))
                        .map(|(_, &t)| Duration::from_secs(1).saturating_sub(now.duration_since(t)))
                        .min()
                        .unwrap_or(Duration::from_millis(100));
                    tokio::time::sleep(min_wait).await;
                    continue;
                }
            }
        };

        // robots.txt check
        if !robots_checker.is_allowed(&url).await {
            warn!("disallowed by robots.txt: {}", url);
            let mut s = state.lock().await;
            s.pages_crawled += 1;
            continue;
        }

        // Record fetch time for this domain
        let domain = url.host_str().unwrap_or("").to_string();
        last_fetch_times.insert(domain, tokio::time::Instant::now());

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
