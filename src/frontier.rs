use std::collections::{HashMap, HashSet, VecDeque};

use url::Url;

/// The URL frontier manages which URLs to crawl next using per-domain FIFO queues.
/// It tracks visited URLs to avoid re-crawling and uses round-robin scheduling
/// across domains for fairness.
pub struct Frontier {
    /// Per-domain FIFO queues
    domain_queues: HashMap<String, VecDeque<Url>>,
    /// Round-robin order of domains (to be fair across domains)
    domain_order: VecDeque<String>,
    /// Dedup set (unchanged)
    visited: HashSet<String>,
}

impl Frontier {
    pub fn new() -> Self {
        Self {
            domain_queues: HashMap::new(),
            domain_order: VecDeque::new(),
            visited: HashSet::new(),
        }
    }

    /// Add a URL to the back of the frontier if it hasn't been visited yet.
    /// Returns `true` if the URL was added, `false` if it was already seen.
    pub fn push(&mut self, url: Url) -> bool {
        if !self.visited.insert(url.as_str().to_string()) {
            return false;
        }

        let domain = url.host_str().unwrap_or("").to_string();

        if !self.domain_queues.contains_key(&domain) {
            self.domain_queues.insert(domain.clone(), VecDeque::new());
            self.domain_order.push_back(domain.clone());
        }

        self.domain_queues
            .get_mut(&domain)
            .unwrap()
            .push_back(url);
        true
    }

    /// Pop the next URL from a domain that is NOT in `blocked_domains`.
    /// Returns None if no eligible URL exists (either frontier is empty,
    /// or all remaining domains are blocked).
    pub fn pop(&mut self, blocked_domains: &HashSet<String>) -> Option<Url> {
        let len = self.domain_order.len();
        if len == 0 {
            return None;
        }

        // Try each domain in round-robin order, skipping blocked ones.
        for _ in 0..len {
            let domain = match self.domain_order.front() {
                Some(d) => d.clone(),
                None => return None,
            };

            if blocked_domains.contains(&domain) {
                // Rotate past this blocked domain so we check the next one.
                self.domain_order.rotate_left(1);
                continue;
            }

            // Found a non-blocked domain — pop from its queue.
            let url = self
                .domain_queues
                .get_mut(&domain)
                .and_then(|q| q.pop_front());

            match url {
                Some(u) => {
                    // If the queue is now empty, remove the domain entirely.
                    if self.domain_queues.get(&domain).is_none_or(|q| q.is_empty()) {
                        self.domain_queues.remove(&domain);
                        self.domain_order.pop_front();
                    } else {
                        // Rotate so the next call starts from the next domain.
                        self.domain_order.rotate_left(1);
                    }
                    return Some(u);
                }
                None => {
                    // Queue was unexpectedly empty — clean up and continue.
                    self.domain_queues.remove(&domain);
                    self.domain_order.pop_front();
                }
            }
        }

        None
    }

    /// Returns true if the frontier has any URLs from non-blocked domains.
    pub fn has_eligible(&self, blocked_domains: &HashSet<String>) -> bool {
        self.domain_order
            .iter()
            .any(|d| !blocked_domains.contains(d))
    }

    /// Number of URLs waiting in the queue.
    pub fn pending(&self) -> usize {
        self.domain_queues.values().map(|q| q.len()).sum()
    }

    /// Number of unique URLs that have been seen (visited + pending).
    pub fn total_seen(&self) -> usize {
        self.visited.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_and_pop() {
        let mut f = Frontier::new();
        let u = Url::parse("https://example.com").unwrap();
        assert!(f.push(u.clone()));
        assert_eq!(f.pending(), 1);
        assert_eq!(f.pop(&HashSet::new()), Some(u));
        assert_eq!(f.pending(), 0);
    }

    #[test]
    fn deduplicates() {
        let mut f = Frontier::new();
        let u = Url::parse("https://example.com").unwrap();
        assert!(f.push(u.clone()));
        assert!(!f.push(u));
        assert_eq!(f.pending(), 1);
    }

    #[test]
    fn blocked_domain_is_skipped() {
        let mut f = Frontier::new();
        let a = Url::parse("https://a.com/page1").unwrap();
        let b = Url::parse("https://b.com/page1").unwrap();
        f.push(a);
        f.push(b.clone());

        let mut blocked = HashSet::new();
        blocked.insert("a.com".to_string());

        // Should skip a.com and return b.com's URL.
        assert_eq!(f.pop(&blocked), Some(b));
    }

    #[test]
    fn all_domains_blocked_returns_none() {
        let mut f = Frontier::new();
        let a = Url::parse("https://a.com/page1").unwrap();
        let b = Url::parse("https://b.com/page1").unwrap();
        f.push(a);
        f.push(b);

        let mut blocked = HashSet::new();
        blocked.insert("a.com".to_string());
        blocked.insert("b.com".to_string());

        assert_eq!(f.pop(&blocked), None);
        // pending() should still show URLs are queued.
        assert_eq!(f.pending(), 2);
    }

    #[test]
    fn round_robin_fairness() {
        let mut f = Frontier::new();
        f.push(Url::parse("https://a.com/1").unwrap());
        f.push(Url::parse("https://a.com/2").unwrap());
        f.push(Url::parse("https://b.com/1").unwrap());
        f.push(Url::parse("https://b.com/2").unwrap());

        let empty = HashSet::new();
        let first = f.pop(&empty).unwrap();
        let second = f.pop(&empty).unwrap();

        // Should alternate between domains.
        assert_ne!(
            first.host_str().unwrap(),
            second.host_str().unwrap(),
            "round-robin should alternate domains"
        );
    }

    #[test]
    fn has_eligible_with_blocked() {
        let mut f = Frontier::new();
        f.push(Url::parse("https://a.com/1").unwrap());
        f.push(Url::parse("https://b.com/1").unwrap());

        let mut blocked = HashSet::new();
        blocked.insert("a.com".to_string());

        assert!(f.has_eligible(&blocked));

        blocked.insert("b.com".to_string());
        assert!(!f.has_eligible(&blocked));
    }
}
