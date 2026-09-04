use crate::core::i18n::tr;
use reqwest::header::{CACHE_CONTROL, PRAGMA};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::LazyLock;
use std::time::{SystemTime, UNIX_EPOCH};
use windows::Win32::UI::WindowsAndMessaging::{
    IDOK, IDYES, MB_ICONINFORMATION, MB_OKCANCEL, MB_SETFOREGROUND, MB_TOPMOST, MessageBoxW,
};
use windows::core::PCWSTR;

static HTTP_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("WinIsland-Updater")
        .build()
        .unwrap()
});

#[derive(Deserialize)]
struct NightlyVersionInfo {
    build_number: u64,
    built_at: String,
    installer_sha256: String,
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubReleaseAsset>,
}

#[derive(Deserialize)]
struct GithubReleaseAsset {
    name: String,
    browser_download_url: String,
    digest: Option<String>,
}

const UPDATE_URL_JSON: &str =
    "https://github.com/WinIslandProject/WinIsland/releases/download/nightly/version_info.json";
const UPDATE_URL_NIGHTLY_INSTALLER: &str = "https://github.com/WinIslandProject/WinIsland/releases/download/nightly/WinIsland-Nightly-Setup.exe";
const UPDATE_URL_STABLE_RELEASE: &str =
    "https://api.github.com/repos/WinIslandProject/WinIsland/releases/latest";
const MAX_INSTALLER_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Clone, Copy)]
enum InstallerChannel {
    Stable,
    Nightly,
}

struct UpdatePackage {
    channel: InstallerChannel,
    download_url: String,
    expected_sha256: String,
}

impl InstallerChannel {
    fn installer_name(self) -> &'static str {
        match self {
            Self::Stable => "WinIsland-Setup.exe",
            Self::Nightly => "WinIsland-Nightly-Setup.exe",
        }
    }

    fn installed_executable() -> PathBuf {
        let mut path = dirs::data_local_dir().unwrap_or_else(get_app_dir);
        path.push("WinIsland");
        path.push("WinIsland.exe");
        path
    }
}

fn nightly_build_number() -> u64 {
    env!("WINISLAND_BUILD_NUMBER").parse().unwrap_or_default()
}

fn cache_buster() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

async fn fetch_json<T: DeserializeOwned>(url: &str) -> Result<T, String> {
    let response = HTTP_CLIENT
        .get(url)
        .header(CACHE_CONTROL, "no-cache, no-store")
        .header(PRAGMA, "no-cache")
        .send()
        .await
        .map_err(|error| error.to_string())?;

    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }

    response.json().await.map_err(|error| error.to_string())
}

fn validate_sha256(value: &str) -> Option<String> {
    let value = value.trim();
    (value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| value.to_ascii_lowercase())
}

fn parse_release_digest(value: Option<&str>) -> Option<String> {
    value
        .and_then(|value| value.strip_prefix("sha256:"))
        .and_then(validate_sha256)
}

fn hex_bytes(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn is_version_newer(current: &str, remote: &str) -> bool {
    let current_parts: Vec<&str> = current.split('.').collect();
    let remote_parts: Vec<&str> = remote.split('.').collect();

    for i in 0..std::cmp::max(current_parts.len(), remote_parts.len()) {
        let current_val = current_parts
            .get(i)
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);
        let remote_val = remote_parts
            .get(i)
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);
        if remote_val > current_val {
            return true;
        } else if remote_val < current_val {
            return false;
        }
    }
    false
}

pub fn get_app_dir() -> PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".winisland");
    if !path.exists() {
        let _ = fs::create_dir_all(&path);
    }
    path
}

pub fn start_update_checker() {
    tokio::spawn(async move {
        let app_dir = get_app_dir();
        let mut last_check = tokio::time::Instant::now();

        // Initial check
        if crate::core::persistence::load_config().check_for_updates {
            log::info!("Update checker started");
            do_check(&app_dir, false).await;
        } else {
            log::info!("Update checker: disabled in config");
        }

        loop {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            let config = crate::core::persistence::load_config();
            if !config.check_for_updates {
                continue;
            }

            let interval_secs = config.update_check_interval * 3600.0;
            if last_check.elapsed().as_secs_f32() >= interval_secs {
                do_check(&app_dir, false).await;
                last_check = tokio::time::Instant::now();
            }
        }
    });
}

pub fn check_updates_manually() {
    std::thread::spawn(|| {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(r) => r,
            Err(e) => {
                log::error!("Failed to build runtime for manual update check: {e:?}");
                return;
            }
        };
        rt.block_on(async {
            let app_dir = get_app_dir();
            do_check(&app_dir, true).await;
        });
    });
}

async fn do_check(app_dir: &Path, manual: bool) {
    let config = crate::core::persistence::load_config();
    let channel = config.update_channel.as_str();

    if channel == "beta" {
        do_beta_check(app_dir, manual).await;
    } else {
        do_stable_check(app_dir, manual).await;
    }
}

async fn notify_check_failed(manual: bool) {
    if manual {
        show_error_box(tr("update_failed_title"), tr("update_failed_desc")).await;
    }
}

async fn notify_up_to_date(manual: bool) {
    if manual {
        show_error_box(tr("update_no_update_title"), tr("update_no_update_desc")).await;
    }
}

async fn prompt_update(
    channel_name: &str,
    version_display: &str,
    package: UpdatePackage,
    app_dir: &Path,
) {
    let title_w: Vec<u16> = format!("{} ({})\0", tr("update_available_title"), channel_name)
        .encode_utf16()
        .collect();
    let description = match package.channel {
        InstallerChannel::Stable => tr("update_available_desc"),
        InstallerChannel::Nightly => {
            tr("update_available_nightly_desc").replace("{}", version_display)
        }
    };
    let text_w: Vec<u16> = description.add_null().encode_utf16().collect();

    let result = tokio::task::spawn_blocking(move || unsafe {
        MessageBoxW(
            None,
            PCWSTR(text_w.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            MB_OKCANCEL | MB_ICONINFORMATION | MB_TOPMOST | MB_SETFOREGROUND,
        )
    })
    .await;

    if let Ok(r) = result
        && (r == IDOK || r == IDYES)
    {
        perform_update(package, app_dir.to_path_buf()).await;
    }
}

async fn do_beta_check(app_dir: &Path, manual: bool) {
    let manifest_url = format!("{UPDATE_URL_JSON}?check={}", cache_buster());
    let remote_info: NightlyVersionInfo = match fetch_json(&manifest_url).await {
        Ok(info) => info,
        Err(error) => {
            log::warn!("Update check (Beta): failed to fetch version info: {error}");
            notify_check_failed(manual).await;
            return;
        }
    };

    let expected_sha256 = match validate_sha256(&remote_info.installer_sha256) {
        Some(hash) => hash,
        None => {
            log::warn!("Update check (Beta): version info contains an invalid installer hash");
            notify_check_failed(manual).await;
            return;
        }
    };

    let local_build = nightly_build_number();
    let remote_build = remote_info.build_number;

    if remote_build > local_build {
        log::info!("Update available (Beta): build {local_build} -> {remote_build}");
        prompt_update(
            &tr("channel_beta"),
            &remote_info.built_at,
            UpdatePackage {
                channel: InstallerChannel::Nightly,
                download_url: format!("{UPDATE_URL_NIGHTLY_INSTALLER}?build={remote_build}"),
                expected_sha256,
            },
            app_dir,
        )
        .await;
    } else {
        log::info!(
            "Update check (Beta): current build is up-to-date (local: {local_build}, remote: {remote_build})"
        );
        notify_up_to_date(manual).await;
    }
}

async fn do_stable_check(app_dir: &Path, manual: bool) {
    let release: GithubRelease = match fetch_json(UPDATE_URL_STABLE_RELEASE).await {
        Ok(release) => release,
        Err(error) => {
            log::warn!("Update check (Stable): failed to fetch latest release: {error}");
            notify_check_failed(manual).await;
            return;
        }
    };

    let remote_version = release
        .tag_name
        .trim_start_matches('v')
        .trim_start_matches('V');
    let needs_update = is_version_newer(crate::core::config::APP_VERSION, remote_version);

    if needs_update {
        log::info!(
            "Update available (Stable): {} -> {}",
            crate::core::config::APP_VERSION,
            remote_version
        );

        let Some(asset) = release
            .assets
            .into_iter()
            .find(|asset| asset.name == InstallerChannel::Stable.installer_name())
        else {
            log::warn!("Update check (Stable): release does not contain WinIsland-Setup.exe");
            notify_check_failed(manual).await;
            return;
        };

        let Some(expected_sha256) = parse_release_digest(asset.digest.as_deref()) else {
            log::warn!("Update check (Stable): installer has no valid SHA-256 digest");
            notify_check_failed(manual).await;
            return;
        };
        prompt_update(
            &tr("channel_stable"),
            remote_version,
            UpdatePackage {
                channel: InstallerChannel::Stable,
                download_url: asset.browser_download_url,
                expected_sha256,
            },
            app_dir,
        )
        .await;
    } else {
        log::info!(
            "Update check (Stable): current version is up-to-date ({})",
            crate::core::config::APP_VERSION
        );
        notify_up_to_date(manual).await;
    }
}

async fn download_installer(package: &UpdatePackage, destination: &Path) -> Result<u64, String> {
    let response = HTTP_CLIENT
        .get(&package.download_url)
        .header(CACHE_CONTROL, "no-cache, no-store")
        .header(PRAGMA, "no-cache")
        .send()
        .await
        .map_err(|error| format!("download request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("download returned HTTP {}", response.status()));
    }
    if response
        .content_length()
        .is_some_and(|length| length == 0 || length > MAX_INSTALLER_BYTES)
    {
        return Err("installer size is invalid".into());
    }

    let temporary = destination.with_extension("download");
    let result = async {
        let mut response = response;
        let mut file = fs::File::create(&temporary)
            .map_err(|error| format!("failed to create installer file: {error}"))?;
        let mut hasher = Sha256::new();
        let mut total = 0u64;
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| format!("failed to read installer response: {error}"))?
        {
            total = total
                .checked_add(chunk.len() as u64)
                .ok_or_else(|| "installer size overflow".to_string())?;
            if total > MAX_INSTALLER_BYTES {
                return Err("installer exceeds the size limit".into());
            }
            hasher.update(&chunk);
            file.write_all(&chunk)
                .map_err(|error| format!("failed to write installer: {error}"))?;
        }
        if total == 0 {
            return Err("installer is empty".into());
        }
        let actual_hash = hex_bytes(&hasher.finalize());
        if actual_hash != package.expected_sha256 {
            return Err(format!(
                "installer hash mismatch (expected {}, got {})",
                package.expected_sha256, actual_hash
            ));
        }
        file.sync_all()
            .map_err(|error| format!("failed to flush installer: {error}"))?;
        if destination.exists() {
            fs::remove_file(destination)
                .map_err(|error| format!("failed to replace installer: {error}"))?;
        }
        fs::rename(&temporary, destination)
            .map_err(|error| format!("failed to activate installer: {error}"))?;
        Ok(total)
    }
    .await;
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

async fn perform_update(package: UpdatePackage, app_dir: PathBuf) {
    log::info!(
        "Update: downloading installer from {}",
        package.download_url
    );

    let installer_directory = app_dir.join("updates");
    if fs::create_dir_all(&installer_directory).is_err() {
        log::error!(
            "Update: failed to create installer directory {}",
            installer_directory.display()
        );
        show_error_box(tr("update_failed_title"), tr("update_failed_save")).await;
        return;
    }
    let installer_path = installer_directory.join(package.channel.installer_name());
    let downloaded = match download_installer(&package, &installer_path).await {
        Ok(bytes) => bytes,
        Err(error) => {
            log::error!("Update: {error}");
            show_error_box(tr("update_failed_title"), tr("update_failed_dl")).await;
            return;
        }
    };

    log::info!(
        "Update: downloaded {downloaded} bytes to {}, scheduling update",
        installer_path.display()
    );

    let installer_path = installer_path.to_string_lossy().into_owned();
    let installed_executable = InstallerChannel::installed_executable()
        .to_string_lossy()
        .into_owned();

    let ps_escape = |s: &str| s.replace('\'', "''");

    let pid = std::process::id();
    let script = format!(
        "while (Get-Process -Id {} -ErrorAction SilentlyContinue) {{ Start-Sleep -Milliseconds 100 }}; \
         $installer = Start-Process -FilePath '{}' -ArgumentList @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/CLOSEAPPLICATIONS') -PassThru -Wait; \
         if ($installer.ExitCode -eq 0) {{ Start-Process -FilePath '{}' }}",
        pid,
        ps_escape(&installer_path),
        ps_escape(&installed_executable)
    );

    let _ = Command::new("powershell")
        .args(["-WindowStyle", "Hidden", "-Command", &script])
        .spawn();

    std::process::exit(0);
}

async fn show_error_box(title: String, text: String) {
    let title_w: Vec<u16> = title.add_null().encode_utf16().collect();
    let text_w: Vec<u16> = text.add_null().encode_utf16().collect();
    // SAFETY: MessageBoxW displays a modal error dialog with the provided
    // null-terminated UTF-16 strings. All pointers are valid for the call duration.
    tokio::task::spawn_blocking(move || unsafe {
        MessageBoxW(
            None,
            PCWSTR(text_w.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            MB_ICONINFORMATION | MB_TOPMOST,
        );
    })
    .await
    .ok();
}

trait AddNull {
    fn add_null(&self) -> String;
}
impl AddNull for String {
    fn add_null(&self) -> String {
        format!("{self}\0")
    }
}
