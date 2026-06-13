use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const DEFAULT_CONFIG_DIR_NAME: &str = ".claude";
const KEYCHAIN_SERVICE_BASE: &str = "Claude Code-credentials";

/// Persisted profile selection, stored next to state.json so it survives
/// app restarts without touching any Claude config directory.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfilesConfig {
    #[serde(default)]
    pub active_config_dir: Option<String>,
    #[serde(default)]
    pub extra_config_dirs: Vec<String>,
}

pub fn load_profiles_config(config_path: &Path) -> ProfilesConfig {
    fs::read_to_string(config_path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

pub fn save_profiles_config(config_path: &Path, config: &ProfilesConfig) -> Result<(), String> {
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let serialized =
        serde_json::to_string_pretty(config).map_err(|error| error.to_string())?;
    fs::write(config_path, format!("{serialized}\n")).map_err(|error| error.to_string())
}

pub fn default_config_dir(home: &Path) -> PathBuf {
    home.join(DEFAULT_CONFIG_DIR_NAME)
}

/// A directory counts as a Claude config dir when any Claude app has left
/// recognizable footprints in it. This keeps unrelated `.claude*` tooling
/// directories out of the account menu.
fn looks_like_claude_config_dir(path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }

    ["settings.json", ".credentials.json", "projects", "sessions"]
        .iter()
        .any(|marker| path.join(marker).exists())
}

/// Scan the home directory for `.claude*` config dirs, merge in any extra
/// dirs from the profiles config, and return them sorted with the default
/// `~/.claude` first.
pub fn discover_profiles(home: &Path, extra_config_dirs: &[String]) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = Vec::new();

    if let Ok(entries) = fs::read_dir(home) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if !name.starts_with(DEFAULT_CONFIG_DIR_NAME) {
                continue;
            }
            let path = entry.path();
            if looks_like_claude_config_dir(&path) {
                found.push(path);
            }
        }
    }

    for extra in extra_config_dirs {
        let path = PathBuf::from(extra);
        if looks_like_claude_config_dir(&path) && !found.contains(&path) {
            found.push(path);
        }
    }

    let default_dir = default_config_dir(home);
    found.sort_by(|a, b| {
        let a_default = *a == default_dir;
        let b_default = *b == default_dir;
        b_default.cmp(&a_default).then_with(|| a.cmp(b))
    });

    if found.is_empty() {
        found.push(default_dir);
    }

    found
}

/// Claude Code stores OAuth credentials in the macOS Keychain under
/// "Claude Code-credentials" for the default config dir, and appends the
/// first 8 hex chars of sha256(CLAUDE_CONFIG_DIR) for custom dirs.
pub fn keychain_service_name(config_dir: &Path, home: &Path) -> String {
    if config_dir == default_config_dir(home) {
        return KEYCHAIN_SERVICE_BASE.to_string();
    }

    let digest = Sha256::digest(config_dir.to_string_lossy().as_bytes());
    let suffix: String = digest
        .iter()
        .take(4)
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("{KEYCHAIN_SERVICE_BASE}-{suffix}")
}

/// Display label for menus and the usage panel: home prefix abbreviated to ~.
pub fn display_label(config_dir: &Path, home: &Path) -> String {
    let dir = config_dir.to_string_lossy().replace('\\', "/");
    let home = home.to_string_lossy().replace('\\', "/");

    match dir.strip_prefix(&home) {
        Some(rest) if rest.starts_with('/') => format!("~{rest}"),
        _ => dir,
    }
}

/// Pick the active profile: the persisted choice when it is still in the
/// discovered list, otherwise the default dir, otherwise the first profile.
pub fn resolve_active_profile(
    profiles: &[PathBuf],
    persisted_active: Option<&str>,
    home: &Path,
) -> PathBuf {
    if let Some(active) = persisted_active {
        let active_path = PathBuf::from(active);
        if profiles.contains(&active_path) {
            return active_path;
        }
    }

    let default_dir = default_config_dir(home);
    if profiles.contains(&default_dir) {
        return default_dir;
    }

    profiles
        .first()
        .cloned()
        .unwrap_or(default_dir)
}

#[cfg(test)]
mod tests {
    use super::{
        default_config_dir, discover_profiles, display_label, keychain_service_name,
        load_profiles_config, resolve_active_profile, save_profiles_config, ProfilesConfig,
    };
    use std::{
        fs,
        path::{Path, PathBuf},
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

    fn make_config_dir(home: &Path, name: &str, marker: &str) -> PathBuf {
        let dir = home.join(name);
        fs::create_dir_all(&dir).expect("config dir should be created");
        if marker.ends_with('/') {
            fs::create_dir_all(dir.join(marker.trim_end_matches('/')))
                .expect("marker dir should be created");
        } else {
            fs::write(dir.join(marker), "{}").expect("marker file should be written");
        }
        dir
    }

    #[test]
    fn derives_default_keychain_service_for_default_dir() {
        let home = Path::new("/Users/gj");
        assert_eq!(
            keychain_service_name(&home.join(".claude"), home),
            "Claude Code-credentials"
        );
    }

    #[test]
    fn derives_hashed_keychain_service_for_custom_dir() {
        // sha256("/Users/gj/.claude-company")[0..8] == 97b8d7f5, verified
        // against a real Claude Code Keychain entry.
        let home = Path::new("/Users/gj");
        assert_eq!(
            keychain_service_name(&home.join(".claude-company"), home),
            "Claude Code-credentials-97b8d7f5"
        );
    }

    #[test]
    fn discovers_claude_dirs_with_markers_and_puts_default_first() {
        let home = unique_test_dir("discover");
        make_config_dir(&home, ".claude-company", "settings.json");
        make_config_dir(&home, ".claude", "projects/");
        make_config_dir(&home, ".claude-empty", "unrelated.txt");
        fs::write(home.join(".claude.json"), "{}").expect("file should be written");

        let profiles = discover_profiles(&home, &[]);

        assert_eq!(
            profiles,
            vec![home.join(".claude"), home.join(".claude-company")]
        );
    }

    #[test]
    fn includes_extra_config_dirs_when_they_look_like_claude_dirs() {
        let home = unique_test_dir("discover-extra");
        make_config_dir(&home, ".claude", "settings.json");
        let outside = unique_test_dir("outside-home");
        let extra = make_config_dir(&outside, "claude-work", ".credentials.json");

        let profiles = discover_profiles(&home, &[extra.to_string_lossy().to_string()]);

        assert_eq!(profiles, vec![home.join(".claude"), extra]);
    }

    #[test]
    fn falls_back_to_default_dir_when_nothing_is_found() {
        let home = unique_test_dir("discover-none");

        let profiles = discover_profiles(&home, &[]);

        assert_eq!(profiles, vec![home.join(".claude")]);
    }

    #[test]
    fn abbreviates_home_prefix_in_display_label() {
        let home = Path::new("/Users/gj");
        assert_eq!(
            display_label(&home.join(".claude-company"), home),
            "~/.claude-company"
        );
        assert_eq!(
            display_label(Path::new("/opt/claude-work"), home),
            "/opt/claude-work"
        );
    }

    #[test]
    fn resolves_active_profile_with_fallbacks() {
        let home = Path::new("/Users/gj");
        let profiles = vec![home.join(".claude"), home.join(".claude-company")];

        assert_eq!(
            resolve_active_profile(
                &profiles,
                Some("/Users/gj/.claude-company"),
                home
            ),
            home.join(".claude-company")
        );
        assert_eq!(
            resolve_active_profile(&profiles, Some("/Users/gj/.claude-gone"), home),
            home.join(".claude")
        );
        assert_eq!(resolve_active_profile(&profiles, None, home), home.join(".claude"));

        let no_default = vec![home.join(".claude-company")];
        assert_eq!(
            resolve_active_profile(&no_default, None, home),
            home.join(".claude-company")
        );
    }

    #[test]
    fn round_trips_profiles_config() {
        let dir = unique_test_dir("profiles-config");
        let config_path = dir.join("state").join("profiles.json");

        let config = ProfilesConfig {
            active_config_dir: Some("/Users/gj/.claude-company".into()),
            extra_config_dirs: vec!["/opt/claude-work".into()],
        };
        save_profiles_config(&config_path, &config).expect("config should save");

        let loaded = load_profiles_config(&config_path);
        assert_eq!(
            loaded.active_config_dir.as_deref(),
            Some("/Users/gj/.claude-company")
        );
        assert_eq!(loaded.extra_config_dirs, vec!["/opt/claude-work".to_string()]);

        let missing = load_profiles_config(&dir.join("missing.json"));
        assert_eq!(missing.active_config_dir, None);
        assert!(missing.extra_config_dirs.is_empty());
    }

    #[test]
    fn default_config_dir_is_home_dot_claude() {
        assert_eq!(
            default_config_dir(Path::new("/Users/gj")),
            PathBuf::from("/Users/gj/.claude")
        );
    }
}
