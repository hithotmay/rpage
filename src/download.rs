//! Download management - track and manage file downloads
//!
//! Provides a simple download tracker that records download metadata.
//! Actual download implementation leverages Chromium's CDP download events
//! or reqwest for session mode.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tracing::{debug, info};

use crate::error::{Error, Result};

/// Status of a download
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadStatus {
    /// Download is in progress
    InProgress,
    /// Download completed successfully
    Completed,
    /// Download was cancelled
    Cancelled,
    /// Download failed with an error
    Failed(String),
}

/// Metadata for a tracked download
#[derive(Debug, Clone)]
pub struct DownloadInfo {
    /// Unique download ID
    pub id: String,
    /// Original URL
    pub url: String,
    /// Suggested filename
    pub filename: String,
    /// Local save path (if completed)
    pub save_path: Option<PathBuf>,
    /// Total size in bytes (if known)
    pub total_size: Option<u64>,
    /// Downloaded bytes so far
    pub downloaded: u64,
    /// Current status
    pub status: DownloadStatus,
}

/// Manages file downloads across both modes
#[derive(Debug, Default)]
pub struct DownloadManager {
    downloads: Mutex<Vec<DownloadInfo>>,
    default_dir: PathBuf,
}

impl DownloadManager {
    /// Create a new download manager with default temp directory
    pub fn new() -> Self {
        let default_dir = std::env::temp_dir().join("rpage_downloads");
        Self {
            downloads: Mutex::new(Vec::new()),
            default_dir,
        }
    }

    /// Create with a custom download directory
    pub fn with_dir(dir: impl Into<PathBuf>) -> Self {
        Self {
            downloads: Mutex::new(Vec::new()),
            default_dir: dir.into(),
        }
    }

    /// Get the default download directory
    pub fn default_dir(&self) -> &Path {
        &self.default_dir
    }

    /// Register a new download
    pub fn register(&self, url: &str, filename: &str) -> String {
        let id = format!("dl_{}", self.downloads.lock().map(|l| l.len()).unwrap_or(0));
        let info = DownloadInfo {
            id: id.clone(),
            url: url.to_string(),
            filename: filename.to_string(),
            save_path: None,
            total_size: None,
            downloaded: 0,
            status: DownloadStatus::InProgress,
        };
        if let Ok(mut list) = self.downloads.lock() {
            list.push(info);
        }
        debug!("Registered download: {id} -> {filename}");
        id
    }

    /// Update download progress
    pub fn update_progress(&self, id: &str, downloaded: u64) {
        if let Ok(mut list) = self.downloads.lock() {
            if let Some(dl) = list.iter_mut().find(|d| d.id == id) {
                dl.downloaded = downloaded;
            }
        }
    }

    /// Mark a download as completed
    pub fn complete(&self, id: &str, save_path: &Path) {
        if let Ok(mut list) = self.downloads.lock() {
            if let Some(dl) = list.iter_mut().find(|d| d.id == id) {
                dl.status = DownloadStatus::Completed;
                dl.save_path = Some(save_path.to_path_buf());
                dl.downloaded = dl.total_size.unwrap_or(dl.downloaded);
                info!("Download completed: {id} -> {}", save_path.display());
            }
        }
    }

    /// Mark a download as failed
    pub fn fail(&self, id: &str, error: &str) {
        if let Ok(mut list) = self.downloads.lock() {
            if let Some(dl) = list.iter_mut().find(|d| d.id == id) {
                dl.status = DownloadStatus::Failed(error.to_string());
            }
        }
    }

    /// Cancel a download
    pub fn cancel(&self, id: &str) {
        if let Ok(mut list) = self.downloads.lock() {
            if let Some(dl) = list.iter_mut().find(|d| d.id == id) {
                dl.status = DownloadStatus::Cancelled;
            }
        }
    }

    /// Get all downloads
    pub fn list(&self) -> Vec<DownloadInfo> {
        self.downloads.lock().map(|l| l.clone()).unwrap_or_default()
    }

    /// Get a specific download by ID
    pub fn get(&self, id: &str) -> Option<DownloadInfo> {
        self.downloads
            .lock()
            .ok()
            .and_then(|l| l.iter().find(|d| d.id == id).cloned())
    }

    /// Get all completed downloads
    pub fn completed(&self) -> Vec<DownloadInfo> {
        self.list()
            .into_iter()
            .filter(|d| d.status == DownloadStatus::Completed)
            .collect()
    }

    /// Clear all download records
    pub fn clear(&self) {
        if let Ok(mut l) = self.downloads.lock() {
            l.clear();
        }
    }

    /// Download a file via HTTP (session mode)
    pub async fn download_file(
        client: &reqwest::Client,
        url: &str,
        save_path: &Path,
    ) -> Result<PathBuf> {
        debug!("Downloading {url} to {}", save_path.display());
        let resp = client.get(url).send().await.map_err(Error::Reqwest)?;

        let _total_size = resp.content_length();
        let bytes = resp.bytes().await.map_err(Error::Reqwest)?;

        // Create parent directory if needed
        if let Some(parent) = save_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(save_path, &bytes)?;
        info!(
            "Downloaded {} bytes to {}",
            bytes.len(),
            save_path.display()
        );

        Ok(save_path.to_path_buf())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_download_manager_register_and_complete() {
        let dm = DownloadManager::new();
        let id = dm.register("https://example.com/file.zip", "file.zip");

        let dl = dm.get(&id).unwrap();
        assert_eq!(dl.status, DownloadStatus::InProgress);
        assert_eq!(dl.filename, "file.zip");

        dm.complete(&id, Path::new("/tmp/file.zip"));
        let dl = dm.get(&id).unwrap();
        assert_eq!(dl.status, DownloadStatus::Completed);
        assert_eq!(dl.save_path, Some(PathBuf::from("/tmp/file.zip")));

        assert_eq!(dm.completed().len(), 1);
    }

    #[test]
    fn test_download_manager_fail() {
        let dm = DownloadManager::new();
        let id = dm.register("https://example.com/file.zip", "file.zip");
        dm.fail(&id, "connection reset");

        let dl = dm.get(&id).unwrap();
        assert!(matches!(dl.status, DownloadStatus::Failed(_)));
    }

    #[test]
    fn test_download_manager_clear() {
        let dm = DownloadManager::new();
        dm.register("https://example.com/a.zip", "a.zip");
        dm.register("https://example.com/b.zip", "b.zip");
        assert_eq!(dm.list().len(), 2);

        dm.clear();
        assert!(dm.list().is_empty());
    }
}
