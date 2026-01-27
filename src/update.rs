use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

use flate2::read::GzDecoder;
use serde::Deserialize;
use tar::Archive;

use crate::constants::VERSION;

const GITHUB_RELEASES_URL: &str =
    "https://api.github.com/repos/only-using-ai/rustxl/releases/latest";

#[derive(Debug, Deserialize)]
pub struct GithubRelease {
    pub tag_name: String,
    pub assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
pub struct GithubAsset {
    pub name: String,
    pub browser_download_url: String,
}

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub download_url: String,
    pub asset_name: String,
}

#[derive(Debug)]
pub enum UpdateMessage {
    Available(UpdateInfo),
    NotAvailable,
    #[allow(dead_code)]
    Error(String),
}

/// Compares two version strings and returns true if `latest` is newer than `current`
fn is_newer_version(current: &str, latest: &str) -> bool {
    let current = current.trim_start_matches('v');
    let latest = latest.trim_start_matches('v');

    match (
        semver::Version::parse(current),
        semver::Version::parse(latest),
    ) {
        (Ok(curr), Ok(lat)) => lat > curr,
        _ => {
            // Fallback to string comparison if semver parsing fails
            latest != current
        }
    }
}

/// Gets the appropriate asset name for the current platform
fn get_platform_asset_name() -> Option<String> {
    let os = env::consts::OS;
    let arch = env::consts::ARCH;

    match (os, arch) {
        ("macos", "aarch64") => Some("xl-darwin-arm64.tar.gz".to_string()),
        ("macos", "x86_64") => Some("xl-darwin-x86_64.tar.gz".to_string()),
        ("linux", "x86_64") => Some("xl-linux-x86_64.tar.gz".to_string()),
        ("linux", "aarch64") => Some("xl-linux-arm64.tar.gz".to_string()),
        _ => None,
    }
}

/// Checks GitHub for the latest release
pub fn check_for_update() -> Result<Option<UpdateInfo>, String> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("xl-spreadsheet")
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let response = client
        .get(GITHUB_RELEASES_URL)
        .send()
        .map_err(|e| format!("Failed to fetch releases: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("GitHub API returned status: {}", response.status()));
    }

    let release: GithubRelease = response
        .json()
        .map_err(|e| format!("Failed to parse release info: {}", e))?;

    let latest_version = release.tag_name.clone();
    let current_version = VERSION.to_string();

    if !is_newer_version(&current_version, &latest_version) {
        return Ok(None);
    }

    // Find the appropriate asset for this platform
    let platform_asset = get_platform_asset_name();
    if platform_asset.is_none() {
        return Err("No compatible binary available for this platform".to_string());
    }
    let platform_asset = platform_asset.unwrap();

    let asset = release
        .assets
        .iter()
        .find(|a| a.name == platform_asset)
        .ok_or_else(|| format!("No asset found for platform: {}", platform_asset))?;

    Ok(Some(UpdateInfo {
        current_version,
        latest_version,
        download_url: asset.browser_download_url.clone(),
        asset_name: asset.name.clone(),
    }))
}

/// Spawns a background thread to check for updates
pub fn spawn_update_checker() -> mpsc::Receiver<UpdateMessage> {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        // Small delay to let the app start up
        thread::sleep(std::time::Duration::from_secs(2));

        let message = match check_for_update() {
            Ok(Some(info)) => UpdateMessage::Available(info),
            Ok(None) => UpdateMessage::NotAvailable,
            Err(e) => UpdateMessage::Error(e),
        };

        let _ = tx.send(message);
    });

    rx
}

/// Downloads and installs the update
pub fn download_and_install(update_info: &UpdateInfo) -> Result<(), String> {
    // Get the current executable path
    let current_exe = env::current_exe()
        .map_err(|e| format!("Failed to get current executable path: {}", e))?;

    // Create a temporary directory for download
    let temp_dir = env::temp_dir().join("xl-update");
    fs::create_dir_all(&temp_dir)
        .map_err(|e| format!("Failed to create temp directory: {}", e))?;

    let archive_path = temp_dir.join(&update_info.asset_name);

    // Download the archive
    download_file(&update_info.download_url, &archive_path)?;

    // Extract the archive
    let extracted_binary = extract_archive(&archive_path, &temp_dir)?;

    // Replace the current executable
    replace_executable(&extracted_binary, &current_exe)?;

    // Clean up
    let _ = fs::remove_dir_all(&temp_dir);

    Ok(())
}

fn download_file(url: &str, dest: &PathBuf) -> Result<(), String> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("xl-spreadsheet")
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let response = client
        .get(url)
        .send()
        .map_err(|e| format!("Failed to download file: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Download failed with status: {}", response.status()));
    }

    let bytes = response
        .bytes()
        .map_err(|e| format!("Failed to read response: {}", e))?;

    let mut file =
        File::create(dest).map_err(|e| format!("Failed to create file: {}", e))?;

    file.write_all(&bytes)
        .map_err(|e| format!("Failed to write file: {}", e))?;

    Ok(())
}

fn extract_archive(archive_path: &PathBuf, dest_dir: &PathBuf) -> Result<PathBuf, String> {
    let file =
        File::open(archive_path).map_err(|e| format!("Failed to open archive: {}", e))?;

    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);

    archive
        .unpack(dest_dir)
        .map_err(|e| format!("Failed to extract archive: {}", e))?;

    // Look for the xl binary in the extracted files
    let binary_name = if cfg!(windows) { "xl.exe" } else { "xl" };

    // Try common locations
    let possible_paths = [
        dest_dir.join(binary_name),
        dest_dir.join("xl").join(binary_name),
    ];

    for path in &possible_paths {
        if path.exists() {
            return Ok(path.clone());
        }
    }

    // Search recursively if not found in common locations
    find_binary_recursive(dest_dir, binary_name)
        .ok_or_else(|| "Could not find xl binary in archive".to_string())
}

fn find_binary_recursive(dir: &PathBuf, binary_name: &str) -> Option<PathBuf> {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.file_name().map_or(false, |n| n == binary_name) {
                return Some(path);
            } else if path.is_dir() {
                if let Some(found) = find_binary_recursive(&path, binary_name) {
                    return Some(found);
                }
            }
        }
    }
    None
}

fn replace_executable(new_exe: &PathBuf, current_exe: &PathBuf) -> Result<(), String> {
    // Create backup of current executable
    let backup_path = current_exe.with_extension("old");

    // On Unix, we need to handle this carefully
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        // Make the new binary executable
        let mut perms = fs::metadata(new_exe)
            .map_err(|e| format!("Failed to get new binary metadata: {}", e))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(new_exe, perms)
            .map_err(|e| format!("Failed to set executable permissions: {}", e))?;

        // Rename current to backup
        if current_exe.exists() {
            fs::rename(current_exe, &backup_path)
                .map_err(|e| format!("Failed to backup current executable: {}", e))?;
        }

        // Copy new binary to current location
        fs::copy(new_exe, current_exe)
            .map_err(|e| format!("Failed to install new binary: {}", e))?;

        // Remove backup
        let _ = fs::remove_file(&backup_path);
    }

    #[cfg(windows)]
    {
        // On Windows, rename current to .old, copy new, then delete old on restart
        if current_exe.exists() {
            let _ = fs::remove_file(&backup_path);
            fs::rename(current_exe, &backup_path)
                .map_err(|e| format!("Failed to backup current executable: {}", e))?;
        }

        fs::copy(new_exe, current_exe)
            .map_err(|e| format!("Failed to install new binary: {}", e))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        assert!(is_newer_version("0.1.0", "0.1.1"));
        assert!(is_newer_version("0.1.0", "0.2.0"));
        assert!(is_newer_version("0.1.0", "1.0.0"));
        assert!(is_newer_version("v0.1.0", "v0.1.1"));
        assert!(!is_newer_version("0.1.1", "0.1.0"));
        assert!(!is_newer_version("0.1.0", "0.1.0"));
    }

    #[test]
    fn test_platform_asset_name() {
        // This test will vary based on the platform it runs on
        let asset = get_platform_asset_name();
        // We just verify it returns Some on supported platforms or None on unsupported
        if cfg!(target_os = "macos") || cfg!(target_os = "linux") {
            assert!(asset.is_some());
        }
    }
}
