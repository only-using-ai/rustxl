use std::env;
use std::fs::{self, DirBuilder, File};
#[cfg(unix)]
use std::fs::{OpenOptions, Permissions};
use std::io::{self, Write};
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;

use flate2::read::GzDecoder;
use serde::Deserialize;
use tar::Archive;

use crate::constants::VERSION;

const GITHUB_RELEASES_URL: &str =
    "https://api.github.com/repos/only-using-ai/rustxl/releases/latest";
static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

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

pub(crate) type InstallResult = Result<String, String>;

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
        ("macos", "aarch64") => Some("xl-macos-arm64.tar.gz".to_string()),
        ("macos", "x86_64") => Some("xl-macos-x86_64.tar.gz".to_string()),
        ("linux", "x86_64") => Some("xl-linux-x86_64.tar.gz".to_string()),
        ("linux", "aarch64") => Some("xl-linux-aarch64.tar.gz".to_string()),
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

pub(crate) fn spawn_update_install(
    info: UpdateInfo,
    install: impl FnOnce(&UpdateInfo) -> Result<(), String> + Send + 'static,
) -> io::Result<mpsc::Receiver<InstallResult>> {
    let (tx, rx) = mpsc::channel();
    thread::Builder::new()
        .name("xl-update".into())
        .spawn(move || {
            let result = install(&info).map(|()| info.latest_version);
            let _ = tx.send(result);
        })?;
    Ok(rx)
}

fn temporary_path(parent: &Path) -> PathBuf {
    let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
    parent.join(format!(".xl-update-{}-{id}", std::process::id()))
}

struct UpdateWorkspace(PathBuf);

impl UpdateWorkspace {
    fn create(parent: &Path) -> io::Result<Self> {
        let mut builder = DirBuilder::new();
        #[cfg(unix)]
        builder.mode(0o700);
        loop {
            let path = temporary_path(parent);
            match builder.create(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
    }
}

impl Drop for UpdateWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Downloads and installs the update
pub fn download_and_install(update_info: &UpdateInfo) -> Result<(), String> {
    if get_platform_asset_name().is_none() {
        return Err("Automatic updates are unavailable on this platform".to_string());
    }

    let current_exe =
        env::current_exe().map_err(|e| format!("Failed to get current executable path: {}", e))?;

    let workspace = UpdateWorkspace::create(&env::temp_dir())
        .map_err(|e| format!("Failed to create update workspace: {e}"))?;
    let archive_path = workspace.0.join(&update_info.asset_name);
    download_file(&update_info.download_url, &archive_path)?;
    let extracted_binary = extract_archive(&archive_path, &workspace.0)?;
    replace_executable(&extracted_binary, &current_exe)
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

#[cfg(unix)]
fn replace_executable(new_exe: &Path, current_exe: &Path) -> Result<(), String> {
    let mut replacement = File::open(new_exe)
        .map_err(|error| format!("Failed to open replacement executable: {error}"))?;
    let metadata = replacement
        .metadata()
        .map_err(|error| format!("Failed to read replacement metadata: {error}"))?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err("The update does not contain a nonempty executable file".to_string());
    }

    install_staged(current_exe, |staged| {
        let copied = io::copy(&mut replacement, staged)?;
        if copied != metadata.len() {
            return Err(io::Error::other("Replacement size changed during staging"));
        }
        Ok(())
    })
    .map_err(|error| format!("Failed to install new executable: {error}"))
}

#[cfg(not(unix))]
fn replace_executable(_: &Path, _: &Path) -> Result<(), String> {
    Err("Automatic updates are unavailable on this platform".to_string())
}

#[cfg(unix)]
struct StagedExecutable {
    path: PathBuf,
    installed: bool,
}

#[cfg(unix)]
impl Drop for StagedExecutable {
    fn drop(&mut self) {
        if !self.installed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[cfg(unix)]
fn install_staged(
    current_exe: &Path,
    write: impl FnOnce(&mut File) -> io::Result<()>,
) -> io::Result<()> {
    let parent = current_exe
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "No executable directory"))?;
    let (mut staging, mut file) = loop {
        let path = temporary_path(parent);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
        {
            Ok(file) => {
                break (
                    StagedExecutable {
                        path,
                        installed: false,
                    },
                    file,
                );
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    };

    write(&mut file)?;
    file.flush()?;
    file.set_permissions(Permissions::from_mode(0o755))?;
    file.sync_all()?;
    drop(file);

    fs::rename(&staging.path, current_exe)?;
    staging.installed = true;
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
        assert!(is_newer_version("0.1.2", "0.1.5"));
        assert!(is_newer_version("0.1.2", "v0.1.5"));
        assert!(!is_newer_version("0.1.1", "0.1.0"));
        assert!(!is_newer_version("0.1.0", "0.1.0"));
        assert!(!is_newer_version("0.1.5", "0.1.2"));
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

    #[test]
    fn update_workspaces_are_isolated_and_cleaned_up() {
        let parent = UpdateWorkspace::create(&env::temp_dir()).unwrap();
        let first = UpdateWorkspace::create(&parent.0).unwrap();
        let second = UpdateWorkspace::create(&parent.0).unwrap();
        let first_path = first.0.clone();
        assert_ne!(first_path, second.0);
        fs::write(first_path.join("partially-downloaded"), b"partial data").unwrap();

        drop(first);

        assert!(!first_path.exists());
        assert!(second.0.is_dir());
    }

    #[test]
    #[cfg(unix)]
    fn partial_stage_failure_keeps_the_live_executable_and_cleans_up() {
        let directory = UpdateWorkspace::create(&env::temp_dir()).unwrap();
        let current = directory.0.join("xl");
        fs::write(&current, b"original executable").unwrap();
        fs::set_permissions(&current, Permissions::from_mode(0o700)).unwrap();

        let error = install_staged(&current, |staged| {
            assert_eq!(staged.metadata()?.permissions().mode() & 0o077, 0);
            staged.write_all(b"partial replacement")?;
            assert_eq!(fs::read(&current)?, b"original executable");
            Err(io::Error::other("short copy"))
        })
        .unwrap_err();

        assert_eq!(error.to_string(), "short copy");
        assert_eq!(fs::read(&current).unwrap(), b"original executable");
        assert_eq!(
            fs::metadata(&current).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
    }

    #[test]
    #[cfg(unix)]
    fn installs_a_complete_executable_and_rejects_an_empty_replacement() {
        use std::io::Read;

        let directory = UpdateWorkspace::create(&env::temp_dir()).unwrap();
        let current = directory.0.join("xl");
        let replacement = directory.0.join("downloaded-xl");
        fs::write(&current, b"original executable").unwrap();
        fs::write(&replacement, b"").unwrap();

        assert!(replace_executable(&replacement, &current).is_err());
        assert_eq!(fs::read(&current).unwrap(), b"original executable");

        let contents = b"#!/bin/sh\nexit 0\n";
        fs::write(&replacement, contents).unwrap();
        let mut previous = File::open(&current).unwrap();
        replace_executable(&replacement, &current).unwrap();

        assert_eq!(fs::read(&current).unwrap(), contents);
        let mut previous_contents = Vec::new();
        previous.read_to_end(&mut previous_contents).unwrap();
        assert_eq!(previous_contents, b"original executable");
        assert_eq!(
            fs::metadata(&current).unwrap().permissions().mode() & 0o777,
            0o755
        );
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 2);
    }

    #[test]
    #[cfg(not(unix))]
    fn unsupported_updater_returns_before_download() {
        let info = UpdateInfo {
            current_version: "0.1.0".into(),
            latest_version: "0.2.0".into(),
            download_url: String::new(),
            asset_name: String::new(),
        };
        assert_eq!(
            download_and_install(&info).unwrap_err(),
            "Automatic updates are unavailable on this platform"
        );
    }
}
