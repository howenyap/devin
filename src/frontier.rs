use std::collections::{HashSet, VecDeque};

use url::Url;

/// The URL frontier manages which URLs to crawl next using a FIFO deque.
/// It also tracks visited URLs to avoid re-crawling.
pub struct Frontier {
    queue: VecDeque<Url>,
    visited: HashSet<String>,
}

impl Frontier {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            visited: HashSet::new(),
        }
    }

    /// Add a URL to the back of the frontier if it hasn't been visited yet.
    /// Returns `true` if the URL was added, `false` if it was already seen.
    pub fn push(&mut self, url: Url) -> bool {
        if !self.visited.insert(url.as_str().to_string()) {
            return false;
        }
        self.queue.push_back(url);
        true
    }

    /// Pop the next URL from the front of the frontier.
    pub fn pop(&mut self) -> Option<Url> {
        self.queue.pop_front()
    }

    /// Number of URLs waiting in the queue.
    pub fn pending(&self) -> usize {
        self.queue.len()
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
        assert_eq!(f.pop(), Some(u));
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
}
