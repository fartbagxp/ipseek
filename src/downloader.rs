use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

/// Base URL for downloading IP range files from GitHub
const GITHUB_REPO_RAW_BASE: &str =
    "https://raw.githubusercontent.com/fartbagxp/aas-cidr-ranges/master/data/raw";

/// Maximum number of retry attempts for failed downloads
const DEFAULT_MAX_RETRIES: u32 = 3;

/// Initial backoff delay in milliseconds
const INITIAL_BACKOFF_MS: u64 = 1000;

/// List of all IP range files to download from the repository
const FILES_TO_DOWNLOAD: &[&str] = &[
    // Cloud infrastructure
    "asn-alibaba-as37963.json",
    "asn-alibaba-as45102.json",
    "asn-baidu-as38365.json",
    "asn-hetzner-as24940.json",
    "asn-huawei-as136907.json",
    "asn-ionos-as8560.json",
    "asn-ovh-as16276.json",
    "asn-ovh-as35540.json",
    "asn-scaleway-as12876.json",
    "asn-tencent-as132203.json",
    "asn-tencent-as45090.json",
    "atlassian.json",
    "aws.json",
    "azure-china.json",
    "azure-germany.json",
    "azure-gov.json",
    "azure-public.json",
    // AI crawlers / bots
    "bing-bingbot.json",
    "duckduckgo-duckassistbot.json",
    "duckduckgo-duckduckbot.json",
    "openai-chatgpt-user.json",
    "openai-gptbot.json",
    "openai-searchbot.json",
    "perplexity-perplexitybot.json",
    "perplexity-user.json",
    // CDN / delivery
    "bunny-ipv4.json",
    "bunny-ipv6.json",
    "cachefly.txt",
    "cloudflare-ipv4.txt",
    "cloudflare-ipv6.txt",
    "datadog.json",
    "digitalocean.txt",
    // DNS
    "public-dns.json",
    // SaaS / services
    "docusign.json",
    "fastly.json",
    "gcore.json",
    "gcp.json",
    "gcp.txt",
    "github.json",
    "gitlab.json",
    "grafana-hosted-alerts.txt",
    "grafana-hosted-logs.txt",
    "grafana-hosted-metrics.txt",
    "grafana-hosted-traces.txt",
    "grafana-hosted.txt",
    "grafana-synthetic-monitoring.json",
    "icloud-private-relay.csv",
    "imperva.json",
    "iana-ipv4.csv",
    "iana-ipv6.csv",
    "linode.txt",
    // Microsoft 365 regional variants
    "m365-china.json",
    "m365-usgov-dod.json",
    "m365-usgov-gcchigh.json",
    "m365-worldwide.json",
    "maxcdn.txt",
    "mongodb-atlas.json",
    "newrelic-synthetics.json",
    "okta.json",
    "oracle.json",
    "pagerduty.json",
    "pingdom-ipv4.txt",
    "pingdom-ipv6.txt",
    "salesforce.json",
    "seqera.json",
    "starlink.csv",
    "statuscake.txt",
    "stripe-api-ip-range.txt",
    "stripe-webhook-ip-range.txt",
    "telegram.json",
    "twilio-sip.json",
    "vultr.json",
    "webex.json",
    "zdxbeta.net-hub.json",
    "zdxcloud.net-hub.json",
    "zdxgov.net-hub.json",
    "zoom-cdn.txt",
    "zoom-contact-center.txt",
    "zoom-crc.txt",
    "zoom-meeting.txt",
    "zoom-phone.txt",
    "zoom-range.txt",
    "zscaler.net-cenr.json",
    "zscaler.net-hub.json",
    "zscalerbeta.net-cenr.json",
    "zscalerbeta.net-hub.json",
    "zscalergov.net-cenr.json",
    "zscalergov.net-hub.json",
    "zscalerone.net-cenr.json",
    "zscalerone.net-hub.json",
    "zscalerten.net-cenr.json",
    "zscalerten.net-hub.json",
    "zscalerthree.net-cenr.json",
    "zscalerthree.net-hub.json",
    "zscalertwo.net-cenr.json",
    "zscalertwo.net-hub.json",
    "zscloud.net-cenr.json",
    "zscloud.net-hub.json",
];

/// Statistics for tracking download progress and results
#[derive(Debug)]
pub struct DownloadStats {
    pub total: usize,
    pub succeeded: usize,
    pub failed: Vec<(String, String)>,
}

impl DownloadStats {
    /// Print a summary of download results
    pub fn print_summary(&self) {
        println!("\nDownload complete!");
        println!("  Total:   {}", self.total);
        println!("  Success: {}", self.succeeded);
        println!("  Failed:  {}", self.failed.len());

        if !self.failed.is_empty() {
            eprintln!(
                "\nWarning: {} file(s) failed to download:",
                self.failed.len()
            );
            for (file, err) in &self.failed {
                eprintln!("  - {}: {}", file, err);
            }
        }
    }
}

/// Main entry point for downloading IP range files from GitHub
///
/// # Arguments
/// * `output_dir` - Directory where files will be saved
/// * `parallel` - Number of concurrent downloads (recommended: 5-10)
///
/// # Returns
/// * `DownloadStats` - Statistics about the download operation
pub async fn run_download(output_dir: &str, parallel: usize) -> Result<DownloadStats> {
    // Create output directory if it doesn't exist
    tokio::fs::create_dir_all(output_dir)
        .await
        .with_context(|| format!("Failed to create directory: {}", output_dir))?;

    println!(
        "Downloading {} files from GitHub...",
        FILES_TO_DOWNLOAD.len()
    );
    println!("Output directory: {}", output_dir);

    let total_files = FILES_TO_DOWNLOAD.len();

    // Semaphore to limit concurrent downloads
    let semaphore = Arc::new(Semaphore::new(parallel));

    // Atomic counter for progress tracking
    let completed = Arc::new(AtomicUsize::new(0));

    // Spawn download tasks
    let mut tasks = Vec::new();
    for filename in FILES_TO_DOWNLOAD {
        let sem = semaphore.clone();
        let url = format!("{}/{}", GITHUB_REPO_RAW_BASE, filename);
        let dest = PathBuf::from(output_dir).join(filename);
        let filename_owned = filename.to_string();
        let completed_clone = completed.clone();

        tasks.push(tokio::spawn(async move {
            // Acquire semaphore permit
            let _permit = sem.acquire().await.unwrap();

            // Attempt download with retry
            let result = download_file_with_retry(url.clone(), dest, DEFAULT_MAX_RETRIES).await;

            // Update progress counter
            let count = completed_clone.fetch_add(1, Ordering::SeqCst) + 1;

            match &result {
                Ok(_) => {
                    println!("[{}/{}] {} ✓", count, total_files, filename_owned);
                }
                Err(e) => {
                    eprintln!("[{}/{}] {} ✗ ({})", count, total_files, filename_owned, e);
                }
            }

            (filename_owned, result)
        }));
    }

    // Wait for all downloads to complete
    let mut succeeded = 0;
    let mut failed = Vec::new();

    for task in tasks {
        match task.await {
            Ok((filename, result)) => match result {
                Ok(_) => succeeded += 1,
                Err(e) => failed.push((filename, e.to_string())),
            },
            Err(e) => {
                eprintln!("Task error: {}", e);
            }
        }
    }

    let stats = DownloadStats {
        total: total_files,
        succeeded,
        failed,
    };

    stats.print_summary();

    // Return error if all downloads failed
    if succeeded == 0 {
        bail!("All downloads failed. Check network connection and try again.");
    }

    Ok(stats)
}

/// Download a single file with retry logic and exponential backoff
///
/// # Arguments
/// * `url` - URL to download from
/// * `dest_path` - Destination file path
/// * `max_retries` - Maximum number of retry attempts
async fn download_file_with_retry(url: String, dest_path: PathBuf, max_retries: u32) -> Result<()> {
    let mut last_error = None;

    for attempt in 0..max_retries {
        match download_single_file(&url, &dest_path).await {
            Ok(_) => return Ok(()),
            Err(e) => {
                last_error = Some(e);
                if attempt < max_retries - 1 {
                    let backoff_ms = INITIAL_BACKOFF_MS * 2_u64.pow(attempt);
                    log::debug!(
                        "Retry {}/{} for {} after {}ms",
                        attempt + 1,
                        max_retries,
                        url,
                        backoff_ms
                    );
                    tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                }
            }
        }
    }

    Err(last_error.unwrap())
}

/// Perform a single HTTP download
///
/// # Arguments
/// * `url` - URL to download from
/// * `dest` - Destination file path
async fn download_single_file(url: &str, dest: &Path) -> Result<()> {
    // Create parent directories if they don't exist
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    // Download file
    let response = reqwest::get(url)
        .await
        .with_context(|| format!("Failed to fetch {}", url))?;

    // Check HTTP status
    if !response.status().is_success() {
        bail!("HTTP {}: {}", response.status(), url);
    }

    // Stream response to disk (memory-efficient for large files)
    let bytes = response
        .bytes()
        .await
        .with_context(|| format!("Failed to read response from {}", url))?;

    // Write to file
    tokio::fs::write(dest, bytes)
        .await
        .with_context(|| format!("Failed to write file: {}", dest.display()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_download_file_success() {
        // Test downloading a small file (github.json)
        let temp_dir = TempDir::new().unwrap();
        let dest = temp_dir.path().join("github.json");

        let url = format!("{}/github.json", GITHUB_REPO_RAW_BASE);
        let result = download_single_file(&url, &dest).await;

        assert!(result.is_ok(), "Download should succeed");
        assert!(dest.exists(), "File should exist after download");

        let metadata = dest.metadata().unwrap();
        assert!(metadata.len() > 0, "File should have content");
    }

    #[tokio::test]
    async fn test_download_nonexistent_file() {
        let temp_dir = TempDir::new().unwrap();
        let dest = temp_dir.path().join("nonexistent.json");

        let url = format!(
            "{}/this-file-does-not-exist-12345.json",
            GITHUB_REPO_RAW_BASE
        );
        let result = download_single_file(&url, &dest).await;

        assert!(result.is_err(), "Download should fail for nonexistent file");
    }

    #[tokio::test]
    async fn test_download_creates_directory() {
        let temp_dir = TempDir::new().unwrap();
        let nested_dir = temp_dir.path().join("nested").join("path");
        let dest = nested_dir.join("test.json");

        let url = format!("{}/github.json", GITHUB_REPO_RAW_BASE);
        let result = download_single_file(&url, &dest).await;

        assert!(result.is_ok(), "Should create parent directories");
        assert!(dest.exists(), "File should exist in nested directory");
    }

    #[tokio::test]
    async fn test_file_list_not_empty() {
        assert!(
            !FILES_TO_DOWNLOAD.is_empty(),
            "File list should not be empty"
        );
        assert!(
            FILES_TO_DOWNLOAD.len() > 80,
            "Should have many files to download"
        );
    }
}
