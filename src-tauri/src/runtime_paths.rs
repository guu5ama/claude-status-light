use std::{
    env,
    path::{Path, PathBuf},
};

const DEFAULT_STATE_RELATIVE_PATH: &str = "../public/state/state.json";
const DEFAULT_BRIDGE_SCRIPT_RELATIVE_PATH: &str = "../bridge/claude-hook.mjs";
const PORTABLE_APP_DIR_NAME: &str = "Claude Status Light";

pub fn dev_state_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_STATE_RELATIVE_PATH)
}

pub fn dev_bridge_script_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_BRIDGE_SCRIPT_RELATIVE_PATH)
}

pub fn portable_state_path(home_dir: &Path, local_app_data: Option<&Path>) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        let base = local_app_data
            .map(PathBuf::from)
            .unwrap_or_else(|| home_dir.join("AppData").join("Local"));
        return base
            .join(PORTABLE_APP_DIR_NAME)
            .join("state")
            .join("state.json");
    }

    #[cfg(target_os = "macos")]
    {
        return home_dir
            .join("Library")
            .join("Application Support")
            .join(PORTABLE_APP_DIR_NAME)
            .join("state")
            .join("state.json");
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = local_app_data;
        home_dir
            .join(".local")
            .join("share")
            .join("claude-status-light")
            .join("state")
            .join("state.json")
    }
}

pub fn resolve_state_path() -> Result<PathBuf, String> {
    if let Ok(path) = env::var("CLAUDE_STATUS_LIGHT_STATE_PATH") {
        if !path.trim().is_empty() {
            return Ok(PathBuf::from(path));
        }
    }

    if cfg!(debug_assertions) {
        return Ok(dev_state_path());
    }

    let home = env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .ok_or_else(|| "Could not resolve the user home directory.".to_string())?;
    let local_app_data = env::var_os("LOCALAPPDATA").map(PathBuf::from);

    Ok(portable_state_path(
        &PathBuf::from(home),
        local_app_data.as_deref(),
    ))
}

pub fn portable_bridge_candidates(current_exe_path: &Path) -> Vec<PathBuf> {
    let Some(exe_dir) = current_exe_path.parent() else {
        return Vec::new();
    };

    vec![
        exe_dir.join("bridge").join("claude-hook.mjs"),
        exe_dir.join("resources").join("bridge").join("claude-hook.mjs"),
    ]
}

#[cfg(test)]
mod tests {
    use super::{portable_bridge_candidates, portable_state_path};
    use std::path::Path;

    #[test]
    fn resolves_windows_portable_state_path_under_local_app_data() {
        let home = Path::new("C:/Users/shan");
        let local_app_data = Path::new("C:/Users/shan/AppData/Local");

        let resolved = portable_state_path(home, Some(local_app_data));

        #[cfg(target_os = "windows")]
        assert_eq!(
            resolved,
            Path::new("C:/Users/shan/AppData/Local/Claude Status Light/state/state.json")
        );

        #[cfg(target_os = "macos")]
        assert_eq!(
            resolved,
            Path::new(
                "C:/Users/shan/Library/Application Support/Claude Status Light/state/state.json"
            )
        );

        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        assert_eq!(
            resolved,
            Path::new("C:/Users/shan/.local/share/claude-status-light/state/state.json")
        );
    }

    #[test]
    fn prefers_exe_sibling_bridge_for_portable_builds() {
        let candidates = portable_bridge_candidates(Path::new(
            "D:/Apps/Claude Status Light Portable/claude-status-light.exe",
        ));

        assert_eq!(
            candidates[0],
            Path::new("D:/Apps/Claude Status Light Portable/bridge/claude-hook.mjs")
        );
        assert_eq!(
            candidates[1],
            Path::new(
                "D:/Apps/Claude Status Light Portable/resources/bridge/claude-hook.mjs"
            )
        );
    }
}
