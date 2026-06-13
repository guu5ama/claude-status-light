mod claude_settings;
mod profiles;
mod runtime_paths;

#[cfg(target_os = "windows")]
#[link(name = "dwmapi")]
extern "system" {
    fn DwmSetWindowAttribute(
        hwnd: *mut core::ffi::c_void,
        dw_attribute: u32,
        pv_attribute: *const core::ffi::c_void,
        cb_attribute: u32,
    ) -> i32;
}

use std::{
    env, fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use claude_settings::{
    format_hook_command, merge_hook_settings, normalize_display_path, strip_utf8_bom, ClaudeSetupStatus,
    ClaudeSetupStatusKind,
};
use tauri::{
    menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, State, Wry,
};

struct RuntimeState {
    claude_setup_status: Mutex<ClaudeSetupStatus>,
    active_config_dir: Mutex<PathBuf>,
}

struct ProfileMenuState {
    items: Mutex<Vec<(PathBuf, CheckMenuItem<Wry>)>>,
}

const PROFILE_MENU_ID_PREFIX: &str = "profile:";

fn home_dir() -> Result<PathBuf, String> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .ok_or_else(|| "Could not resolve the user home directory.".to_string())
}

fn profiles_config_path() -> PathBuf {
    let state_path = resolve_state_path();
    state_path
        .parent()
        .map(|parent| parent.join("profiles.json"))
        .unwrap_or_else(|| PathBuf::from("profiles.json"))
}

static ATOMIC_WRITE_NONCE: AtomicU64 = AtomicU64::new(0);

fn initial_setup_status() -> ClaudeSetupStatus {
    ClaudeSetupStatus {
        kind: ClaudeSetupStatusKind::Failed,
        message: "Claude hook configuration has not run yet.".into(),
        settings_path: String::new(),
        backup_path: None,
        active_bridge_path: None,
        wrote_changes: false,
        requires_claude_restart: false,
    }
}

fn resolve_state_path() -> PathBuf {
    runtime_paths::resolve_state_path().unwrap_or_else(|_| runtime_paths::dev_state_path())
}

fn override_claude_settings_path() -> Option<PathBuf> {
    env::var("CLAUDE_STATUS_LIGHT_CLAUDE_SETTINGS_PATH")
        .ok()
        .filter(|path| !path.trim().is_empty())
        .map(PathBuf::from)
}

fn validate_bridge_script_path(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err(format!(
            "Claude hook bridge script path does not exist: {}",
            path.display()
        ));
    }

    if !path.is_file() {
        return Err(format!(
            "Claude hook bridge script path is not a file: {}",
            path.display()
        ));
    }

    Ok(())
}

fn resolve_bridge_script_path(app: &AppHandle) -> Result<PathBuf, String> {
    if let Ok(path) = env::var("CLAUDE_STATUS_LIGHT_BRIDGE_SCRIPT_PATH") {
        if !path.trim().is_empty() {
            let path = PathBuf::from(path);
            validate_bridge_script_path(&path)?;
            return Ok(path);
        }
    }

    if let Ok(resource_dir) = app.path().resource_dir() {
        // Tauri maps the `../bridge/**/*` resource glob to `_up_/bridge/`
        // inside the bundle, so check both layouts.
        for bundled in [
            resource_dir.join("bridge").join("claude-hook.mjs"),
            resource_dir.join("_up_").join("bridge").join("claude-hook.mjs"),
        ] {
            if bundled.is_file() {
                return Ok(bundled);
            }
        }
    }

    if let Ok(current_exe) = env::current_exe() {
        for candidate in runtime_paths::portable_bridge_candidates(&current_exe) {
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }

    let dev_path = runtime_paths::dev_bridge_script_path();
    if dev_path.is_file() {
        return Ok(dev_path);
    }

    Err("Could not locate bridge/claude-hook.mjs for Claude hook configuration.".into())
}

fn current_unix_timestamp_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}

fn build_backup_path(settings_path: &Path) -> PathBuf {
    let file_name = settings_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("settings.json");
    let nonce = ATOMIC_WRITE_NONCE.fetch_add(1, Ordering::Relaxed);
    let unique_suffix = format!("{}-{nonce}", current_unix_timestamp_nanos());
    settings_path.with_file_name(format!("{file_name}.bak-{unique_suffix}"))
}

fn build_atomic_write_paths(target_path: &Path) -> (PathBuf, PathBuf) {
    let parent = target_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let file_name = target_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("settings.json");
    let nonce = ATOMIC_WRITE_NONCE.fetch_add(1, Ordering::Relaxed);
    let unique_suffix = format!("{}-{nonce}", current_unix_timestamp_nanos());

    (
        parent.join(format!(".{file_name}.tmp-{unique_suffix}")),
        parent.join(format!(".{file_name}.rollback-{unique_suffix}")),
    )
}

fn write_text_atomically(target_path: &Path, contents: &str) -> Result<(), String> {
    let parent = target_path
        .parent()
        .ok_or_else(|| "Settings path has no parent directory.".to_string())?;

    fs::create_dir_all(parent).map_err(|error| error.to_string())?;

    let (temp_path, rollback_path) = build_atomic_write_paths(target_path);

    fs::write(&temp_path, contents).map_err(|error| error.to_string())?;

    replace_file_with_rollback(target_path, &temp_path, &rollback_path, &mut |from, to| {
        fs::rename(from, to)
    })
}

fn replace_file_with_rollback<F>(
    target_path: &Path,
    temp_path: &Path,
    rollback_path: &Path,
    rename_file: &mut F,
) -> Result<(), String>
where
    F: FnMut(&Path, &Path) -> std::io::Result<()>,
{
    if !target_path.exists() {
        return rename_file(temp_path, target_path).map_err(|error| error.to_string());
    }

    rename_file(target_path, rollback_path).map_err(|error| error.to_string())?;

    if let Err(error) = rename_file(temp_path, target_path) {
        let restore_result = rename_file(rollback_path, target_path);
        return Err(match restore_result {
            Ok(()) => error.to_string(),
            Err(restore_error) => format!(
                "{error}; original settings restore also failed: {restore_error}"
            ),
        });
    }

    let _ = fs::remove_file(rollback_path);
    Ok(())
}

fn run_claude_setup(app: &AppHandle) -> ClaudeSetupStatus {
    let bridge_script_path = match resolve_bridge_script_path(app) {
        Ok(path) => path,
        Err(message) => {
            return ClaudeSetupStatus {
                kind: ClaudeSetupStatusKind::Failed,
                message,
                settings_path: String::new(),
                backup_path: None,
                active_bridge_path: None,
                wrote_changes: false,
                requires_claude_restart: false,
            }
        }
    };

    if let Some(settings_path) = override_claude_settings_path() {
        return run_claude_setup_for_paths(&settings_path, &bridge_script_path);
    }

    let home = match home_dir() {
        Ok(home) => home,
        Err(message) => {
            return ClaudeSetupStatus {
                kind: ClaudeSetupStatusKind::Failed,
                message,
                settings_path: String::new(),
                backup_path: None,
                active_bridge_path: None,
                wrote_changes: false,
                requires_claude_restart: false,
            }
        }
    };

    let stored = profiles::load_profiles_config(&profiles_config_path());
    let config_dirs = profiles::discover_profiles(&home, &stored.extra_config_dirs);

    let statuses = config_dirs
        .iter()
        .map(|config_dir| {
            (
                profiles::display_label(config_dir, &home),
                run_claude_setup_for_paths(&config_dir.join("settings.json"), &bridge_script_path),
            )
        })
        .collect();

    aggregate_setup_statuses(statuses)
}

fn aggregate_setup_statuses(mut statuses: Vec<(String, ClaudeSetupStatus)>) -> ClaudeSetupStatus {
    if statuses.is_empty() {
        return initial_setup_status();
    }
    if statuses.len() == 1 {
        return statuses.remove(0).1;
    }

    let total = statuses.len();
    let failures: Vec<String> = statuses
        .iter()
        .filter(|(_, status)| status.kind == ClaudeSetupStatusKind::Failed)
        .map(|(label, status)| format!("{label}: {}", status.message))
        .collect();
    let any_configured = statuses
        .iter()
        .any(|(_, status)| status.kind == ClaudeSetupStatusKind::Configured);

    let (kind, message) = if !failures.is_empty() {
        let mut message = failures.join(" ");
        if any_configured {
            message.push_str(" Other config paths were updated.");
        }
        (ClaudeSetupStatusKind::Failed, message)
    } else if any_configured {
        (
            ClaudeSetupStatusKind::Configured,
            format!("Claude hook bridge paths were updated for {total} config path(s)."),
        )
    } else {
        (
            ClaudeSetupStatusKind::AlreadyConfigured,
            format!("Claude hook bridge is already configured for {total} config path(s)."),
        )
    };

    ClaudeSetupStatus {
        kind,
        message,
        settings_path: statuses
            .iter()
            .map(|(_, status)| status.settings_path.clone())
            .filter(|path| !path.is_empty())
            .collect::<Vec<_>>()
            .join(", "),
        backup_path: statuses
            .iter()
            .find_map(|(_, status)| status.backup_path.clone()),
        active_bridge_path: statuses
            .iter()
            .find_map(|(_, status)| status.active_bridge_path.clone()),
        wrote_changes: statuses.iter().any(|(_, status)| status.wrote_changes),
        requires_claude_restart: statuses
            .iter()
            .any(|(_, status)| status.requires_claude_restart),
    }
}

fn run_claude_setup_for_paths(settings_path: &Path, bridge_script_path: &Path) -> ClaudeSetupStatus {
    let active_bridge_path = normalize_display_path(&bridge_script_path.to_string_lossy());
    if let Err(message) = validate_bridge_script_path(bridge_script_path) {
        return ClaudeSetupStatus {
            kind: ClaudeSetupStatusKind::Failed,
            message,
            settings_path: settings_path.display().to_string(),
            backup_path: None,
            active_bridge_path: Some(active_bridge_path),
            wrote_changes: false,
            requires_claude_restart: false,
        };
    }

    let hook_command = format_hook_command(bridge_script_path);

    let existing_settings = match fs::read_to_string(settings_path) {
        Ok(raw) => match serde_json::from_str::<serde_json::Value>(strip_utf8_bom(&raw)) {
            Ok(parsed) => parsed,
            Err(error) => {
                return ClaudeSetupStatus {
                    kind: ClaudeSetupStatusKind::Failed,
                    message: format!("Could not parse Claude settings.json: {error}"),
                    settings_path: settings_path.display().to_string(),
                    backup_path: None,
                    active_bridge_path: Some(active_bridge_path.clone()),
                    wrote_changes: false,
                    requires_claude_restart: false,
                }
            }
        },
        Err(error) if error.kind() == ErrorKind::NotFound => serde_json::json!({}),
        Err(error) => {
            return ClaudeSetupStatus {
                kind: ClaudeSetupStatusKind::Failed,
                message: format!("Could not read Claude settings.json: {error}"),
                settings_path: settings_path.display().to_string(),
                backup_path: None,
                active_bridge_path: Some(active_bridge_path.clone()),
                wrote_changes: false,
                requires_claude_restart: false,
            }
        }
    };

    let (merged_settings, changed) = match merge_hook_settings(existing_settings, &hook_command) {
        Ok(result) => result,
        Err(message) => {
            return ClaudeSetupStatus {
                kind: ClaudeSetupStatusKind::Failed,
                message,
                settings_path: settings_path.display().to_string(),
                backup_path: None,
                active_bridge_path: Some(active_bridge_path.clone()),
                wrote_changes: false,
                requires_claude_restart: false,
            }
        }
    };

    if !changed {
        return ClaudeSetupStatus {
            kind: ClaudeSetupStatusKind::AlreadyConfigured,
            message: "Claude hook bridge is already configured for this app location.".into(),
            settings_path: settings_path.display().to_string(),
            backup_path: None,
            active_bridge_path: Some(active_bridge_path.clone()),
            wrote_changes: false,
            requires_claude_restart: false,
        };
    }

    let backup_path = if settings_path.exists() {
        let backup = build_backup_path(settings_path);
        if let Some(parent) = backup.parent() {
            if let Err(error) = fs::create_dir_all(parent) {
                return ClaudeSetupStatus {
                    kind: ClaudeSetupStatusKind::Failed,
                    message: format!("Could not create Claude backup directory: {error}"),
                    settings_path: settings_path.display().to_string(),
                    backup_path: None,
                    active_bridge_path: Some(active_bridge_path.clone()),
                    wrote_changes: false,
                    requires_claude_restart: false,
                };
            }
        }

        if let Err(error) = fs::copy(settings_path, &backup) {
            return ClaudeSetupStatus {
                kind: ClaudeSetupStatusKind::Failed,
                message: format!("Could not back up Claude settings.json: {error}"),
                settings_path: settings_path.display().to_string(),
                backup_path: None,
                active_bridge_path: Some(active_bridge_path.clone()),
                wrote_changes: false,
                requires_claude_restart: false,
            };
        }

        Some(backup)
    } else {
        None
    };

    let serialized = match serde_json::to_string_pretty(&merged_settings) {
        Ok(raw) => format!("{raw}\n"),
        Err(error) => {
            return ClaudeSetupStatus {
                kind: ClaudeSetupStatusKind::Failed,
                message: format!("Could not serialize merged Claude settings: {error}"),
                settings_path: settings_path.display().to_string(),
                backup_path: backup_path.as_ref().map(|path| path.display().to_string()),
                active_bridge_path: Some(active_bridge_path.clone()),
                wrote_changes: false,
                requires_claude_restart: false,
            }
        }
    };

    if let Err(message) = write_text_atomically(settings_path, &serialized) {
        return ClaudeSetupStatus {
            kind: ClaudeSetupStatusKind::Failed,
            message: format!("Could not write Claude settings.json: {message}"),
            settings_path: settings_path.display().to_string(),
            backup_path: backup_path.as_ref().map(|path| path.display().to_string()),
            active_bridge_path: Some(active_bridge_path.clone()),
            wrote_changes: false,
            requires_claude_restart: false,
        };
    }

    ClaudeSetupStatus {
        kind: ClaudeSetupStatusKind::Configured,
        message: "Claude hook bridge paths were updated for this app location.".into(),
        settings_path: settings_path.display().to_string(),
        backup_path: backup_path.map(|path| path.display().to_string()),
        active_bridge_path: Some(active_bridge_path),
        wrote_changes: true,
        requires_claude_restart: true,
    }
}

fn store_setup_status(
    app: &AppHandle,
    runtime_state: &State<'_, RuntimeState>,
    status: ClaudeSetupStatus,
) -> Result<ClaudeSetupStatus, String> {
    {
        let mut guard = runtime_state
            .claude_setup_status
            .lock()
            .map_err(|_| "Could not lock Claude setup state.".to_string())?;
        *guard = status.clone();
    }

    app.emit("claude-setup-status", status.clone())
        .map_err(|error| error.to_string())?;

    Ok(status)
}

#[tauri::command]
fn read_state_file() -> Result<String, String> {
    fs::read_to_string(resolve_state_path()).map_err(|error| error.to_string())
}

#[tauri::command]
fn get_claude_setup_status(state: State<'_, RuntimeState>) -> Result<ClaudeSetupStatus, String> {
    state
        .claude_setup_status
        .lock()
        .map(|guard| guard.clone())
        .map_err(|_| "Could not read Claude setup state.".to_string())
}

#[tauri::command]
fn configure_claude_hooks(
    app: AppHandle,
    state: State<'_, RuntimeState>,
) -> Result<ClaudeSetupStatus, String> {
    let status = run_claude_setup(&app);
    store_setup_status(&app, &state, status)
}

#[tauri::command]
fn reset_session_binding() -> Result<(), String> {
    let state_path = resolve_state_path();
    let sound_enabled = fs::read_to_string(&state_path)
        .ok()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .and_then(|json| json.get("soundEnabled").and_then(|value| value.as_bool()))
        .unwrap_or(true);
    let next_state = serde_json::json!({
        "sessionId": null,
        "status": "idle_unbound",
        "updatedAt": "",
        "soundEnabled": sound_enabled,
        "lastEvent": null,
        "lastMessageText": "",
        "doneReason": "not_bound",
        "bridgeHealthy": false
    });

    if let Some(parent) = state_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    fs::write(
        state_path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&next_state).map_err(|error| error.to_string())?
        ),
    )
    .map_err(|error| error.to_string())
}

#[derive(serde::Serialize)]
struct UsageWindow {
    utilization: f64,
    resets_at: String,
}

#[derive(serde::Serialize)]
struct ClaudeUsage {
    five_hour: Option<UsageWindow>,
    seven_day: Option<UsageWindow>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct UsageErrorPayload {
    kind: &'static str,
    message: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct UsagePayload {
    config_dir_label: String,
    usage: Option<ClaudeUsage>,
    error: Option<UsageErrorPayload>,
}

enum UsageFetchError {
    /// The selected config path has no usable login: no credentials, an
    /// expired token, or the API rejected the token. The user must sign in
    /// again with whatever Claude app uses that path.
    NoActiveLogin,
    /// Network errors, rate limits, server errors — the last good value
    /// should be kept on the frontend.
    Transient(String),
}

struct OauthCredentials {
    access_token: String,
    expires_at_ms: Option<u64>,
}

fn extract_credentials(raw: &str) -> Option<OauthCredentials> {
    let json: serde_json::Value = serde_json::from_str(strip_utf8_bom(raw)).ok()?;
    let oauth = json.get("claudeAiOauth")?;
    let access_token = oauth
        .get("accessToken")
        .and_then(|token| token.as_str())
        .map(str::trim)
        .filter(|token| !token.is_empty())?
        .to_string();
    let expires_at_ms = oauth.get("expiresAt").and_then(|value| value.as_u64());

    Some(OauthCredentials {
        access_token,
        expires_at_ms,
    })
}

fn credentials_expired(credentials: &OauthCredentials) -> bool {
    let Some(expires_at_ms) = credentials.expires_at_ms else {
        return false;
    };
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);
    expires_at_ms <= now_ms
}

// On macOS, Claude apps store OAuth credentials in the login Keychain. The
// service name depends on the config dir, so each config path maps to its
// own Keychain entry regardless of which app performed the login.
#[cfg(target_os = "macos")]
fn read_keychain_credentials(service_name: &str) -> Option<OauthCredentials> {
    let output = std::process::Command::new("security")
        .args(["find-generic-password", "-s", service_name, "-w"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let raw = String::from_utf8(output.stdout).ok()?;
    extract_credentials(&raw)
}

fn read_oauth_credentials_for(config_dir: &Path, home: &Path) -> Option<OauthCredentials> {
    if let Ok(path) = env::var("CLAUDE_STATUS_LIGHT_CLAUDE_CREDENTIALS_PATH") {
        if !path.trim().is_empty() {
            return fs::read_to_string(path)
                .ok()
                .and_then(|raw| extract_credentials(&raw));
        }
    }

    if let Ok(raw) = fs::read_to_string(config_dir.join(".credentials.json")) {
        if let Some(credentials) = extract_credentials(&raw) {
            return Some(credentials);
        }
    }

    #[cfg(target_os = "macos")]
    {
        let service_name = profiles::keychain_service_name(config_dir, home);
        if let Some(credentials) = read_keychain_credentials(&service_name) {
            return Some(credentials);
        }
    }

    #[cfg(not(target_os = "macos"))]
    let _ = home;

    None
}

fn parse_usage_window(value: Option<&serde_json::Value>) -> Option<UsageWindow> {
    let window = value?;
    if window.is_null() {
        return None;
    }

    let utilization = window.get("utilization")?.as_f64()?;
    let resets_at = window.get("resets_at")?.as_str()?.to_string();

    Some(UsageWindow {
        utilization,
        resets_at,
    })
}

fn request_claude_usage(access_token: &str) -> Result<ClaudeUsage, UsageFetchError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|error| UsageFetchError::Transient(format!("Could not build HTTP client: {error}")))?;

    let response = client
        .get("https://api.anthropic.com/api/oauth/usage")
        .header("Authorization", format!("Bearer {access_token}"))
        .header("anthropic-beta", "oauth-2025-04-20")
        .header("anthropic-version", "2023-06-01")
        .send()
        .map_err(|error| UsageFetchError::Transient(format!("Usage request failed: {error}")))?;

    let status = response.status();
    if status.as_u16() == 401 || status.as_u16() == 403 {
        return Err(UsageFetchError::NoActiveLogin);
    }
    if !status.is_success() {
        return Err(UsageFetchError::Transient(format!(
            "Usage endpoint returned HTTP {}.",
            status.as_u16()
        )));
    }

    let body: serde_json::Value = response
        .json()
        .map_err(|error| UsageFetchError::Transient(format!("Could not parse usage response: {error}")))?;

    Ok(ClaudeUsage {
        five_hour: parse_usage_window(body.get("five_hour")),
        seven_day: parse_usage_window(body.get("seven_day")),
    })
}

fn no_active_login_message(config_dir_label: &str) -> String {
    format!(
        "No active login found for {config_dir_label}. Sign in with any Claude app that uses this config path, then switch back here."
    )
}

fn fetch_claude_usage_for(config_dir: PathBuf, home: PathBuf) -> UsagePayload {
    let config_dir_label = profiles::display_label(&config_dir, &home);

    let result = match read_oauth_credentials_for(&config_dir, &home) {
        None => Err(UsageFetchError::NoActiveLogin),
        Some(credentials) if credentials_expired(&credentials) => {
            Err(UsageFetchError::NoActiveLogin)
        }
        Some(credentials) => request_claude_usage(&credentials.access_token),
    };

    match result {
        Ok(usage) => UsagePayload {
            config_dir_label,
            usage: Some(usage),
            error: None,
        },
        Err(UsageFetchError::NoActiveLogin) => {
            let message = no_active_login_message(&config_dir_label);
            UsagePayload {
                config_dir_label,
                usage: None,
                error: Some(UsageErrorPayload {
                    kind: "no_active_login",
                    message,
                }),
            }
        }
        Err(UsageFetchError::Transient(message)) => UsagePayload {
            config_dir_label,
            usage: None,
            error: Some(UsageErrorPayload {
                kind: "transient",
                message,
            }),
        },
    }
}

#[tauri::command]
async fn get_claude_usage(state: State<'_, RuntimeState>) -> Result<UsagePayload, String> {
    let config_dir = state
        .active_config_dir
        .lock()
        .map(|guard| guard.clone())
        .map_err(|_| "Could not read the active config path.".to_string())?;
    let home = home_dir()?;

    tauri::async_runtime::spawn_blocking(move || fetch_claude_usage_for(config_dir, home))
        .await
        .map_err(|error| format!("Usage task failed: {error}"))
}

fn set_active_profile(app: &AppHandle, config_dir: PathBuf) {
    let menu_state = app.state::<ProfileMenuState>();
    if let Ok(items) = menu_state.items.lock() {
        for (dir, item) in items.iter() {
            let _ = item.set_checked(*dir == config_dir);
        }
    }

    let runtime_state = app.state::<RuntimeState>();
    if let Ok(mut active) = runtime_state.active_config_dir.lock() {
        *active = config_dir.clone();
    }

    let config_path = profiles_config_path();
    let mut stored = profiles::load_profiles_config(&config_path);
    stored.active_config_dir = Some(config_dir.to_string_lossy().to_string());
    let _ = profiles::save_profiles_config(&config_path, &stored);

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.emit("active-profile-changed", ());
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let home = home_dir().unwrap_or_else(|_| PathBuf::from("."));
    let stored_profiles = profiles::load_profiles_config(&profiles_config_path());
    let discovered_profiles = profiles::discover_profiles(&home, &stored_profiles.extra_config_dirs);
    let active_config_dir = profiles::resolve_active_profile(
        &discovered_profiles,
        stored_profiles.active_config_dir.as_deref(),
        &home,
    );

    tauri::Builder::default()
        .manage(RuntimeState {
            claude_setup_status: Mutex::new(initial_setup_status()),
            active_config_dir: Mutex::new(active_config_dir.clone()),
        })
        .invoke_handler(tauri::generate_handler![
            read_state_file,
            get_claude_setup_status,
            configure_claude_hooks,
            reset_session_binding,
            get_claude_usage
        ])
        .setup(move |app| {
            let toggle_window =
                MenuItem::with_id(app, "toggle_window", "Open/Hide", true, None::<&str>)?;
            let toggle_sound =
                MenuItem::with_id(app, "toggle_sound", "Sound On/Off", true, None::<&str>)?;
            let toggle_details =
                MenuItem::with_id(app, "toggle_details", "Show/Hide Details", true, None::<&str>)?;

            let mut profile_items: Vec<(PathBuf, CheckMenuItem<Wry>)> = Vec::new();
            for config_dir in &discovered_profiles {
                let item = CheckMenuItem::with_id(
                    app,
                    format!("{PROFILE_MENU_ID_PREFIX}{}", config_dir.to_string_lossy()),
                    profiles::display_label(config_dir, &home),
                    true,
                    *config_dir == active_config_dir,
                    None::<&str>,
                )?;
                profile_items.push((config_dir.clone(), item));
            }
            let account_item_refs: Vec<&dyn IsMenuItem<Wry>> = profile_items
                .iter()
                .map(|(_, item)| item as &dyn IsMenuItem<Wry>)
                .collect();
            let account_menu = Submenu::with_items(app, "Account", true, &account_item_refs)?;
            app.manage(ProfileMenuState {
                items: Mutex::new(profile_items),
            });

            let configure_hooks = MenuItem::with_id(
                app,
                "configure_claude_hooks",
                "Configure Claude Hooks",
                true,
                None::<&str>,
            )?;
            let reconnect = MenuItem::with_id(
                app,
                "reconnect_session",
                "Reconnect Session",
                true,
                None::<&str>,
            )?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(
                app,
                &[
                    &toggle_window,
                    &toggle_sound,
                    &toggle_details,
                    &account_menu,
                    &configure_hooks,
                    &reconnect,
                    &quit,
                ],
            )?;

            let runtime_state = app.state::<RuntimeState>();
            let initial_status = run_claude_setup(&app.handle());
            let _ = store_setup_status(&app.handle(), &runtime_state, initial_status);

            let mut tray_builder = TrayIconBuilder::new();

            if let Some(icon) = app.default_window_icon().cloned() {
                tray_builder = tray_builder.icon(icon);
            }

            tray_builder
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "toggle_window" => {
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(true) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                    "toggle_sound" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.emit("toggle-sound", ());
                        }
                    }
                    "toggle_details" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.emit("toggle-details", ());
                        }
                    }
                    "configure_claude_hooks" => {
                        let runtime_state = app.state::<RuntimeState>();
                        let status = run_claude_setup(app);
                        let _ = store_setup_status(app, &runtime_state, status);
                    }
                    "reconnect_session" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.emit("reconnect-session", ());
                        }
                    }
                    "quit" => app.exit(0),
                    id if id.starts_with(PROFILE_MENU_ID_PREFIX) => {
                        let config_dir = PathBuf::from(&id[PROFILE_MENU_ID_PREFIX.len()..]);
                        set_active_profile(app, config_dir);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        if let Some(window) = tray.app_handle().get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            #[cfg(target_os = "windows")]
            if let Some(window) = app.get_webview_window("main") {
                if let Ok(hwnd) = window.hwnd() {
                    const DWMWA_BORDER_COLOR: u32 = 34;
                    const DWMWA_COLOR_NONE: u32 = 0xFFFFFFFE;
                    unsafe {
                        DwmSetWindowAttribute(
                            hwnd.0,
                            DWMWA_BORDER_COLOR,
                            &DWMWA_COLOR_NONE as *const u32 as *const core::ffi::c_void,
                            core::mem::size_of::<u32>() as u32,
                        );
                    }
                }
            }

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::{
        aggregate_setup_statuses, build_atomic_write_paths, build_backup_path,
        credentials_expired, extract_credentials, fetch_claude_usage_for, format_hook_command,
        initial_setup_status, no_active_login_message, replace_file_with_rollback,
        run_claude_setup_for_paths, ClaudeSetupStatus, ClaudeSetupStatusKind, OauthCredentials,
    };
    use serde_json::json;
    use std::{
        fs, io,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn unique_test_dir(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("claude-status-light-{label}-{unique}"));
        fs::create_dir_all(&path).expect("test dir should be created");
        path
    }

    fn configured_hooks(command: &str) -> String {
        serde_json::to_string_pretty(&json!({
            "hooks": {
                "UserPromptSubmit": [{
                    "matcher": "",
                    "hooks": [{
                        "type": "command",
                        "command": command
                    }]
                }],
                "Notification": [{
                    "matcher": "",
                    "hooks": [{
                        "type": "command",
                        "command": command
                    }]
                }],
                "PreToolUse": [{
                    "matcher": "AskUserQuestion",
                    "hooks": [{
                        "type": "command",
                        "command": command
                    }]
                }],
                "PostToolUse": [{
                    "matcher": "",
                    "hooks": [{
                        "type": "command",
                        "command": command
                    }]
                }],
                "Stop": [{
                    "matcher": "",
                    "hooks": [{
                        "type": "command",
                        "command": command
                    }]
                }]
            }
        }))
        .expect("configured hook JSON should serialize")
    }

    #[test]
    fn initial_setup_status_starts_without_active_bridge_path() {
        let status = initial_setup_status();

        assert_eq!(status.kind, ClaudeSetupStatusKind::Failed);
        assert_eq!(status.active_bridge_path, None);
    }

    #[test]
    fn returns_already_configured_with_active_bridge_path_when_no_rewrite_is_needed() {
        let temp_dir = unique_test_dir("already-configured");
        let settings_path = temp_dir.join("settings.json");
        let bridge_path = temp_dir
            .join("claude-status-light")
            .join("bridge")
            .join("claude-hook.mjs");
        fs::create_dir_all(bridge_path.parent().expect("bridge should have parent"))
            .expect("bridge dir should be created");
        fs::write(&bridge_path, "// bridge").expect("bridge file should be written");

        let hook_command = format_hook_command(&bridge_path);
        fs::write(&settings_path, configured_hooks(&hook_command))
            .expect("settings file should be written");

        let status = run_claude_setup_for_paths(&settings_path, &bridge_path);

        assert_eq!(status.kind, ClaudeSetupStatusKind::AlreadyConfigured);
        assert_eq!(
            status.message,
            "Claude hook bridge is already configured for this app location."
        );
        assert_eq!(status.settings_path, settings_path.display().to_string());
        assert_eq!(
            status.active_bridge_path,
            Some(bridge_path.to_string_lossy().replace('\\', "/"))
        );
        assert_eq!(status.backup_path, None);
        assert!(!status.wrote_changes);
        assert!(!status.requires_claude_restart);
    }

    // Verbatim `\\?\` path prefixes only exist on Windows; other platforms
    // treat them as a literal (nonexistent) path.
    #[cfg(target_os = "windows")]
    #[test]
    fn normalizes_windows_verbatim_prefix_in_active_bridge_path() {
        let temp_dir = unique_test_dir("normalized-active-bridge");
        let settings_path = temp_dir.join("settings.json");
        let bridge_path = temp_dir
            .join("Claude Status Light_2")
            .join("bridge")
            .join("claude-hook.mjs");
        fs::create_dir_all(bridge_path.parent().expect("bridge should have parent"))
            .expect("bridge dir should be created");
        fs::write(&bridge_path, "// bridge").expect("bridge file should be written");

        let hook_command = format_hook_command(&bridge_path);
        fs::write(&settings_path, configured_hooks(&hook_command))
            .expect("settings file should be written");

        let verbatim_bridge_path = PathBuf::from(format!(r"\\?\{}", bridge_path.display()));
        let status = run_claude_setup_for_paths(&settings_path, &verbatim_bridge_path);

        assert_eq!(status.kind, ClaudeSetupStatusKind::AlreadyConfigured);
        assert_eq!(
            status.active_bridge_path,
            Some(bridge_path.to_string_lossy().replace('\\', "/"))
        );
    }

    #[test]
    fn returns_configured_with_backup_and_active_bridge_path_when_rewrite_happens() {
        let temp_dir = unique_test_dir("rewrite-config");
        let settings_path = temp_dir.join("settings.json");
        let bridge_path = temp_dir
            .join("current")
            .join("claude-status-light")
            .join("bridge")
            .join("claude-hook.mjs");
        let old_bridge_path = temp_dir
            .join("old")
            .join("claude-status-light")
            .join("bridge")
            .join("claude-hook.mjs");
        fs::create_dir_all(bridge_path.parent().expect("bridge should have parent"))
            .expect("bridge dir should be created");
        fs::create_dir_all(old_bridge_path.parent().expect("old bridge should have parent"))
            .expect("old bridge dir should be created");
        fs::write(&bridge_path, "// bridge").expect("bridge file should be written");
        fs::write(&old_bridge_path, "// old bridge").expect("old bridge file should be written");

        let old_command = format_hook_command(&old_bridge_path);
        fs::write(&settings_path, configured_hooks(&old_command))
            .expect("settings file should be written");

        let status = run_claude_setup_for_paths(&settings_path, &bridge_path);
        let written_settings = fs::read_to_string(&settings_path)
            .expect("updated settings should be readable");
        let written_json: serde_json::Value =
            serde_json::from_str(&written_settings).expect("updated settings should parse");
        let current_command = format_hook_command(&bridge_path);

        assert_eq!(status.kind, ClaudeSetupStatusKind::Configured);
        assert_eq!(
            status.message,
            "Claude hook bridge paths were updated for this app location."
        );
        assert_eq!(status.settings_path, settings_path.display().to_string());
        assert_eq!(
            status.active_bridge_path,
            Some(bridge_path.to_string_lossy().replace('\\', "/"))
        );
        assert!(status.backup_path.is_some());
        assert!(status.wrote_changes);
        assert!(status.requires_claude_restart);
        assert_eq!(
            written_json["hooks"]["UserPromptSubmit"][0]["hooks"][0]["command"],
            json!(current_command)
        );
        assert_eq!(
            written_json["hooks"]["Notification"][0]["hooks"][0]["command"],
            json!(current_command)
        );
        assert_eq!(
            written_json["hooks"]["PreToolUse"][0]["hooks"][0]["command"],
            json!(current_command)
        );
        assert_eq!(
            written_json["hooks"]["Stop"][0]["hooks"][0]["command"],
            json!(current_command)
        );
        assert!(!written_settings.contains(&old_command));
    }

    #[test]
    fn fails_setup_when_bridge_script_path_does_not_exist() {
        let temp_dir = unique_test_dir("missing-bridge");
        let settings_path = temp_dir.join("settings.json");
        let bridge_path = temp_dir
            .join("missing")
            .join("claude-status-light")
            .join("bridge")
            .join("claude-hook.mjs");
        let original_settings = "{\n  \"model\": \"opus\"\n}\n";
        fs::write(&settings_path, original_settings).expect("settings file should be written");

        let status = run_claude_setup_for_paths(&settings_path, &bridge_path);

        assert_eq!(status.kind, ClaudeSetupStatusKind::Failed);
        assert!(status.message.contains("does not exist"));
        assert_eq!(
            status.active_bridge_path,
            Some(bridge_path.to_string_lossy().replace('\\', "/"))
        );
        assert!(!status.wrote_changes);
        assert_eq!(
            fs::read_to_string(&settings_path).expect("settings file should still exist"),
            original_settings
        );
    }

    #[test]
    fn restores_original_settings_if_replacement_commit_fails() {
        let temp_dir = unique_test_dir("rollback-write");
        let settings_path = temp_dir.join("settings.json");
        let temp_path = temp_dir.join(".settings.json.tmp");
        let backup_path = temp_dir.join("settings.json.rollback");

        fs::write(&settings_path, "original").expect("settings file should be written");
        fs::write(&temp_path, "replacement").expect("temp file should be written");

        let mut rename_calls = 0;
        let result = replace_file_with_rollback(
            &settings_path,
            &temp_path,
            &backup_path,
            &mut |from, to| {
                rename_calls += 1;
                match rename_calls {
                    1 => fs::rename(from, to),
                    2 => Err(io::Error::other("forced rename failure")),
                    3 => fs::rename(from, to),
                    _ => panic!("unexpected rename call"),
                }
            },
        );

        assert!(result.is_err());
        assert_eq!(
            fs::read_to_string(&settings_path).expect("original settings should be restored"),
            "original"
        );
        assert_eq!(
            fs::read_to_string(&temp_path).expect("replacement temp file should remain"),
            "replacement"
        );
        assert!(!backup_path.exists());
    }

    #[test]
    fn atomic_write_paths_are_unique_across_immediate_calls() {
        let temp_dir = unique_test_dir("atomic-paths");
        let settings_path = temp_dir.join("settings.json");

        let (temp_a, rollback_a) = build_atomic_write_paths(&settings_path);
        let (temp_b, rollback_b) = build_atomic_write_paths(&settings_path);

        assert_ne!(temp_a, temp_b);
        assert_ne!(rollback_a, rollback_b);
    }

    #[test]
    fn backup_paths_are_unique_across_immediate_calls() {
        let temp_dir = unique_test_dir("backup-paths");
        let settings_path = temp_dir.join("settings.json");

        let backup_a = build_backup_path(&settings_path);
        let backup_b = build_backup_path(&settings_path);

        assert_ne!(backup_a, backup_b);
    }

    fn setup_status(kind: ClaudeSetupStatusKind, message: &str, settings_path: &str) -> ClaudeSetupStatus {
        ClaudeSetupStatus {
            kind: kind.clone(),
            message: message.into(),
            settings_path: settings_path.into(),
            backup_path: None,
            active_bridge_path: Some("/apps/bridge/claude-hook.mjs".into()),
            wrote_changes: kind == ClaudeSetupStatusKind::Configured,
            requires_claude_restart: kind == ClaudeSetupStatusKind::Configured,
        }
    }

    #[test]
    fn aggregates_multi_profile_setup_as_configured_when_any_path_was_updated() {
        let aggregated = aggregate_setup_statuses(vec![
            (
                "~/.claude".into(),
                setup_status(
                    ClaudeSetupStatusKind::AlreadyConfigured,
                    "already",
                    "/Users/gj/.claude/settings.json",
                ),
            ),
            (
                "~/.claude-company".into(),
                setup_status(
                    ClaudeSetupStatusKind::Configured,
                    "updated",
                    "/Users/gj/.claude-company/settings.json",
                ),
            ),
        ]);

        assert_eq!(aggregated.kind, ClaudeSetupStatusKind::Configured);
        assert_eq!(
            aggregated.message,
            "Claude hook bridge paths were updated for 2 config path(s)."
        );
        assert_eq!(
            aggregated.settings_path,
            "/Users/gj/.claude/settings.json, /Users/gj/.claude-company/settings.json"
        );
        assert!(aggregated.wrote_changes);
        assert!(aggregated.requires_claude_restart);
    }

    #[test]
    fn aggregates_multi_profile_setup_failures_with_path_labels() {
        let aggregated = aggregate_setup_statuses(vec![
            (
                "~/.claude".into(),
                setup_status(
                    ClaudeSetupStatusKind::Configured,
                    "updated",
                    "/Users/gj/.claude/settings.json",
                ),
            ),
            (
                "~/.claude-company".into(),
                setup_status(
                    ClaudeSetupStatusKind::Failed,
                    "Could not parse Claude settings.json: oops",
                    "/Users/gj/.claude-company/settings.json",
                ),
            ),
        ]);

        assert_eq!(aggregated.kind, ClaudeSetupStatusKind::Failed);
        assert_eq!(
            aggregated.message,
            "~/.claude-company: Could not parse Claude settings.json: oops Other config paths were updated."
        );
        assert!(aggregated.wrote_changes);
    }

    #[test]
    fn aggregate_with_single_status_returns_it_unchanged() {
        let aggregated = aggregate_setup_statuses(vec![(
            "~/.claude".into(),
            setup_status(
                ClaudeSetupStatusKind::AlreadyConfigured,
                "already",
                "/Users/gj/.claude/settings.json",
            ),
        )]);

        assert_eq!(aggregated.kind, ClaudeSetupStatusKind::AlreadyConfigured);
        assert_eq!(aggregated.message, "already");
    }

    #[test]
    fn extracts_credentials_with_expiry_from_credentials_json() {
        let raw = r#"{"claudeAiOauth":{"accessToken":"token-123","expiresAt":1765000000000}}"#;

        let credentials = extract_credentials(raw).expect("credentials should parse");

        assert_eq!(credentials.access_token, "token-123");
        assert_eq!(credentials.expires_at_ms, Some(1765000000000));
        assert!(extract_credentials(r#"{"claudeAiOauth":{"accessToken":"  "}}"#).is_none());
        assert!(extract_credentials("not json").is_none());
    }

    #[test]
    fn treats_past_expiry_as_expired_and_missing_expiry_as_valid() {
        let expired = OauthCredentials {
            access_token: "token".into(),
            expires_at_ms: Some(1),
        };
        let no_expiry = OauthCredentials {
            access_token: "token".into(),
            expires_at_ms: None,
        };
        let future = OauthCredentials {
            access_token: "token".into(),
            expires_at_ms: Some(u64::MAX),
        };

        assert!(credentials_expired(&expired));
        assert!(!credentials_expired(&no_expiry));
        assert!(!credentials_expired(&future));
    }

    #[test]
    fn reports_no_active_login_for_config_dir_without_credentials() {
        let temp_dir = unique_test_dir("usage-no-login");
        let config_dir = temp_dir.join(".claude-empty");
        fs::create_dir_all(&config_dir).expect("config dir should be created");

        let payload = fetch_claude_usage_for(config_dir, temp_dir.clone());

        assert_eq!(payload.config_dir_label, "~/.claude-empty");
        assert!(payload.usage.is_none());
        let error = payload.error.expect("payload should carry an error");
        assert_eq!(error.kind, "no_active_login");
        assert_eq!(error.message, no_active_login_message("~/.claude-empty"));
    }
}
