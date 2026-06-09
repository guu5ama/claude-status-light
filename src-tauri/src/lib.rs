mod claude_settings;
mod runtime_paths;

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
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, State,
};

struct RuntimeState {
    claude_setup_status: Mutex<ClaudeSetupStatus>,
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

fn resolve_claude_settings_path() -> Result<PathBuf, String> {
    if let Ok(path) = env::var("CLAUDE_STATUS_LIGHT_CLAUDE_SETTINGS_PATH") {
        if !path.trim().is_empty() {
            return Ok(PathBuf::from(path));
        }
    }

    let home = env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .ok_or_else(|| "Could not resolve the user home directory.".to_string())?;

    Ok(PathBuf::from(home).join(".claude").join("settings.json"))
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
        let bundled = resource_dir.join("bridge").join("claude-hook.mjs");
        if bundled.is_file() {
            return Ok(bundled);
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
    let settings_path = match resolve_claude_settings_path() {
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

    let bridge_script_path = match resolve_bridge_script_path(app) {
        Ok(path) => path,
        Err(message) => {
            return ClaudeSetupStatus {
                kind: ClaudeSetupStatusKind::Failed,
                message,
                settings_path: settings_path.display().to_string(),
                backup_path: None,
                active_bridge_path: None,
                wrote_changes: false,
                requires_claude_restart: false,
            }
        }
    };

    run_claude_setup_for_paths(&settings_path, &bridge_script_path)
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

fn resolve_claude_credentials_path() -> Result<PathBuf, String> {
    if let Ok(path) = env::var("CLAUDE_STATUS_LIGHT_CLAUDE_CREDENTIALS_PATH") {
        if !path.trim().is_empty() {
            return Ok(PathBuf::from(path));
        }
    }

    let home = env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .ok_or_else(|| "Could not resolve the user home directory.".to_string())?;

    Ok(PathBuf::from(home).join(".claude").join(".credentials.json"))
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

fn read_oauth_access_token() -> Result<String, String> {
    let path = resolve_claude_credentials_path()?;
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("Could not read Claude credentials: {error}"))?;
    let json: serde_json::Value = serde_json::from_str(strip_utf8_bom(&raw))
        .map_err(|error| format!("Could not parse Claude credentials: {error}"))?;

    json.get("claudeAiOauth")
        .and_then(|oauth| oauth.get("accessToken"))
        .and_then(|token| token.as_str())
        .filter(|token| !token.trim().is_empty())
        .map(|token| token.to_string())
        .ok_or_else(|| "No OAuth access token found in Claude credentials.".to_string())
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

fn fetch_claude_usage() -> Result<ClaudeUsage, String> {
    let token = read_oauth_access_token()?;
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|error| format!("Could not build HTTP client: {error}"))?;

    let response = client
        .get("https://api.anthropic.com/api/oauth/usage")
        .header("Authorization", format!("Bearer {token}"))
        .header("anthropic-beta", "oauth-2025-04-20")
        .header("anthropic-version", "2023-06-01")
        .send()
        .map_err(|error| format!("Usage request failed: {error}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "Usage endpoint returned HTTP {}.",
            response.status().as_u16()
        ));
    }

    let body: serde_json::Value = response
        .json()
        .map_err(|error| format!("Could not parse usage response: {error}"))?;

    Ok(ClaudeUsage {
        five_hour: parse_usage_window(body.get("five_hour")),
        seven_day: parse_usage_window(body.get("seven_day")),
    })
}

#[tauri::command]
async fn get_claude_usage() -> Result<ClaudeUsage, String> {
    tauri::async_runtime::spawn_blocking(fetch_claude_usage)
        .await
        .map_err(|error| format!("Usage task failed: {error}"))?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(RuntimeState {
            claude_setup_status: Mutex::new(initial_setup_status()),
        })
        .invoke_handler(tauri::generate_handler![
            read_state_file,
            get_claude_setup_status,
            configure_claude_hooks,
            reset_session_binding,
            get_claude_usage
        ])
        .setup(|app| {
            let toggle_window =
                MenuItem::with_id(app, "toggle_window", "Open/Hide", true, None::<&str>)?;
            let toggle_sound =
                MenuItem::with_id(app, "toggle_sound", "Sound On/Off", true, None::<&str>)?;
            let toggle_details =
                MenuItem::with_id(app, "toggle_details", "Show/Hide Details", true, None::<&str>)?;
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
                &[&toggle_window, &toggle_sound, &toggle_details, &configure_hooks, &reconnect, &quit],
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

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::{
        build_atomic_write_paths, build_backup_path, format_hook_command, initial_setup_status,
        replace_file_with_rollback, run_claude_setup_for_paths, ClaudeSetupStatusKind,
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
}
