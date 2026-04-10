use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;

/// Record written to the output file for each crawled page.
#[derive(Debug, Serialize)]
pub struct CrawlRecord {
    pub url: String,
    pub title: Option<String>,
    pub links_found: usize,
    pub timestamp: String,
}

/// Writes crawl results to a JSONL (JSON Lines) file.
pub struct Storage {
    output_path: PathBuf,
}

impl Storage {
    /// Create a new storage that writes to the given file path.
    /// Creates parent directories if needed.
    pub fn new(output_path: &Path) -> std::io::Result<Self> {
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }
        Ok(Self {
            output_path: output_path.to_path_buf(),
        })
    }

    /// Append a crawl record as a JSON line.
    pub fn write_record(&self, record: &CrawlRecord) -> std::io::Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.output_path)?;

        let json = serde_json::to_string(record)
            .map_err(std::io::Error::other)?;
        writeln!(file, "{json}")?;
        Ok(())
    }

    /// Return the path to the output file.
    pub fn path(&self) -> &Path {
        &self.output_path
    }
}
