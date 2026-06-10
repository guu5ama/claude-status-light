use std::path::Path;

use serde::Serialize;
use serde_json::{json, Value};

struct TargetHookConfig {
    event_name: &'static str,
    matcher: &'static str,
}

const TARGET_HOOKS: [TargetHookConfig; 5] = [
    TargetHookConfig {
        event_name: "UserPromptSubmit",
        matcher: "",
    },
    TargetHookConfig {
        event_name: "Notification",
        matcher: "",
    },
    TargetHookConfig {
        event_name: "PreToolUse",
        matcher: "AskUserQuestion",
    },
    TargetHookConfig {
        event_name: "PostToolUse",
        matcher: "",
    },
    TargetHookConfig {
        event_name: "Stop",
        matcher: "",
    },
];

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClaudeSetupStatusKind {
    Configured,
    AlreadyConfigured,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeSetupStatus {
    pub kind: ClaudeSetupStatusKind,
    pub message: String,
    pub settings_path: String,
    pub backup_path: Option<String>,
    pub active_bridge_path: Option<String>,
    pub wrote_changes: bool,
    pub requires_claude_restart: bool,
}

pub fn format_hook_command(script_path: &Path) -> String {
    let normalized = normalize_display_path(&script_path.to_string_lossy());
    format!("node \"{normalized}\"")
}

pub fn normalize_display_path(path: &str) -> String {
    let trimmed = path.trim();
    let without_verbatim_prefix = trimmed
        .strip_prefix("//?/")
        .or_else(|| trimmed.strip_prefix(r"\\?\"))
        .unwrap_or(trimmed);

    without_verbatim_prefix.replace('\\', "/")
}

pub fn strip_utf8_bom(raw: &str) -> &str {
    raw.strip_prefix('\u{feff}').unwrap_or(raw)
}

fn is_bridge_hook_command(command: &str) -> bool {
    let trimmed = command.trim();
    let Some(node_args) = trimmed
        .strip_prefix("node ")
        .or_else(|| trimmed.strip_prefix("NODE "))
    else {
        return false;
    };

    let quoted_path = node_args
        .trim()
        .strip_prefix('"')
        .and_then(|path| path.strip_suffix('"'));
    let Some(path) = quoted_path else {
        return false;
    };

    let normalized_path = normalize_command_path(path);
    let segments: Vec<&str> = normalized_path.split('/').collect();
    if segments.len() < 3 {
        return false;
    }

    let Some(file_name) = segments.last() else {
        return false;
    };
    if *file_name != "claude-hook.mjs" || segments[segments.len() - 2] != "bridge" {
        return false;
    }

    segments[..segments.len() - 2]
        .iter()
        .any(|segment| is_claude_status_light_segment(segment))
}

fn normalize_command_path(path: &str) -> String {
    let trimmed = path.trim();
    let without_verbatim_prefix = trimmed
        .strip_prefix("//?/")
        .or_else(|| trimmed.strip_prefix(r"\\?\"))
        .unwrap_or(trimmed);
    let slash_normalized = without_verbatim_prefix.replace('\\', "/");
    let without_duplicate_prefix = slash_normalized
        .strip_prefix("//?/")
        .unwrap_or(&slash_normalized);

    let mut normalized_segments: Vec<String> = Vec::new();
    for segment in without_duplicate_prefix.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }

        if segment == ".." {
            if normalized_segments
                .last()
                .map(|value| !value.ends_with(':'))
                .unwrap_or(false)
            {
                normalized_segments.pop();
            }
            continue;
        }

        normalized_segments.push(segment.to_ascii_lowercase());
    }

    normalized_segments.join("/")
}

fn is_claude_status_light_segment(segment: &str) -> bool {
    let normalized_words = segment
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    normalized_words.starts_with("claude status light")
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn has_existing_bridge_hooks(settings: &Value) -> bool {
    let Some(root) = settings.as_object() else {
        return false;
    };

    let Some(hooks) = root.get("hooks").and_then(Value::as_object) else {
        return false;
    };

    TARGET_HOOKS.iter().all(|target| {
        hooks.get(target.event_name)
            .and_then(Value::as_array)
            .map(|groups| {
                groups.iter().any(|group| {
                    group.as_object().map(|group_object| {
                        let matcher = group_object
                            .get("matcher")
                            .and_then(Value::as_str)
                            .unwrap_or_default();

                        if matcher != target.matcher {
                            return false;
                        }

                        group_object
                            .get("hooks")
                            .and_then(Value::as_array)
                            .map(|hook_entries| {
                                hook_entries.iter().any(|hook| {
                                    hook.get("type").and_then(Value::as_str) == Some("command")
                                        && hook
                                            .get("command")
                                            .and_then(Value::as_str)
                                            .map(is_bridge_hook_command)
                                            .unwrap_or(false)
                                })
                            })
                            .unwrap_or(false)
                    }).unwrap_or(false)
                })
            })
            .unwrap_or(false)
    })
}

pub fn merge_hook_settings(existing: Value, command: &str) -> Result<(Value, bool), String> {
    let mut settings = match existing {
        Value::Object(map) => Value::Object(map),
        _ => return Err("Claude settings root must be a JSON object".into()),
    };

    let Some(root) = settings.as_object_mut() else {
        return Err("Claude settings root must be a JSON object".into());
    };

    let hooks_value = root.entry("hooks").or_insert_with(|| json!({}));
    let Some(hooks_object) = hooks_value.as_object_mut() else {
        return Err("Claude settings `hooks` must be a JSON object".into());
    };

    let command_hook = json!({
        "type": "command",
        "command": command
    });

    let mut changed = false;

    for target in TARGET_HOOKS {
        let event_value = hooks_object
            .entry(target.event_name.to_string())
            .or_insert_with(|| Value::Array(Vec::new()));

        let Some(groups) = event_value.as_array_mut() else {
            return Err(format!("Claude hook event `{}` must be an array", target.event_name));
        };

        let mut matched_target_matcher = false;
        let mut kept_current_command = false;
        for group in groups.iter_mut() {
            let Some(group_object) = group.as_object_mut() else {
                continue;
            };

            let matcher = group_object
                .get("matcher")
                .and_then(Value::as_str)
                .unwrap_or_default();

            if matcher != target.matcher {
                continue;
            }

            matched_target_matcher = true;

            if !matches!(group_object.get("hooks"), Some(Value::Array(_))) {
                group_object.insert("hooks".to_string(), Value::Array(Vec::new()));
                changed = true;
            }

            let hooks = group_object
                .get_mut("hooks")
                .and_then(Value::as_array_mut)
                .expect("matched hook group should have been normalized to an array");

            let original_hooks = hooks.clone();
            let mut normalized_hooks = Vec::with_capacity(original_hooks.len() + 1);
            for hook in original_hooks {
                if hook == command_hook {
                    if !kept_current_command {
                        normalized_hooks.push(hook);
                        kept_current_command = true;
                    }
                    continue;
                }

                let keep_hook = hook
                    .get("command")
                    .and_then(Value::as_str)
                    .map(|existing_command| !is_bridge_hook_command(existing_command))
                    .unwrap_or(true);

                if keep_hook {
                    normalized_hooks.push(hook);
                }
            }

            if !kept_current_command {
                normalized_hooks.push(command_hook.clone());
                kept_current_command = true;
            }

            if *hooks != normalized_hooks {
                *hooks = normalized_hooks;
                changed = true;
            }
        }

        if !matched_target_matcher {
            groups.push(json!({
                "matcher": target.matcher,
                "hooks": [command_hook.clone()]
            }));
            changed = true;
        }
    }

    Ok((settings, changed))
}

#[cfg(test)]
mod tests {
    use super::{
        format_hook_command, has_existing_bridge_hooks, merge_hook_settings, strip_utf8_bom,
        ClaudeSetupStatus, ClaudeSetupStatusKind,
    };
    use serde_json::json;
    use std::path::Path;

    #[test]
    fn formats_bridge_script_command_with_forward_slashes() {
        let command = format_hook_command(Path::new(r"C:\code\claude-status-light\bridge\claude-hook.mjs"));
        assert_eq!(
            command,
            r#"node "C:/code/claude-status-light/bridge/claude-hook.mjs""#
        );
    }

    #[test]
    fn strips_windows_verbatim_prefix_when_formatting_bridge_command() {
        let command = format_hook_command(Path::new(
            r"\\?\C:\Users\shan\Desktop\Claude Status Light_2\bridge\claude-hook.mjs",
        ));
        assert_eq!(
            command,
            r#"node "C:/Users/shan/Desktop/Claude Status Light_2/bridge/claude-hook.mjs""#
        );
    }

    #[test]
    fn strips_utf8_bom_from_json_text() {
        assert_eq!(strip_utf8_bom("\u{feff}{\"a\":1}"), "{\"a\":1}");
        assert_eq!(strip_utf8_bom("{\"a\":1}"), "{\"a\":1}");
    }

    #[test]
    fn serializes_active_bridge_path_as_camel_case() {
        let status = ClaudeSetupStatus {
            kind: ClaudeSetupStatusKind::Configured,
            message: "configured".into(),
            settings_path: "C:/Users/test/.claude/settings.json".into(),
            backup_path: Some("C:/Users/test/.claude/settings.json.bak".into()),
            active_bridge_path: Some("C:/code/claude-status-light/bridge/claude-hook.mjs".into()),
            wrote_changes: true,
            requires_claude_restart: true,
        };

        assert_eq!(
            serde_json::to_value(status).expect("status should serialize"),
            json!({
                "kind": "configured",
                "message": "configured",
                "settingsPath": "C:/Users/test/.claude/settings.json",
                "backupPath": "C:/Users/test/.claude/settings.json.bak",
                "activeBridgePath": "C:/code/claude-status-light/bridge/claude-hook.mjs",
                "wroteChanges": true,
                "requiresClaudeRestart": true
            })
        );
    }

    #[test]
    fn creates_missing_hook_structure() {
        let (settings, changed) = merge_hook_settings(json!({ "model": "opus" }), "node \"bridge\"")
            .expect("merge should succeed");

        assert!(changed);
        assert_eq!(
            settings["hooks"]["UserPromptSubmit"][0],
            json!({
                "matcher": "",
                "hooks": [
                    {
                        "type": "command",
                        "command": "node \"bridge\""
                    }
                ]
            })
        );
        assert_eq!(
            settings["hooks"]["Notification"][0]["hooks"][0]["command"],
            json!("node \"bridge\"")
        );
        assert_eq!(
            settings["hooks"]["PreToolUse"][0],
            json!({
                "matcher": "AskUserQuestion",
                "hooks": [
                    {
                        "type": "command",
                        "command": "node \"bridge\""
                    }
                ]
            })
        );
        assert_eq!(
            settings["hooks"]["PostToolUse"][0],
            json!({
                "matcher": "",
                "hooks": [
                    {
                        "type": "command",
                        "command": "node \"bridge\""
                    }
                ]
            })
        );
        assert_eq!(
            settings["hooks"]["Stop"][0]["hooks"][0]["command"],
            json!("node \"bridge\"")
        );
    }

    #[test]
    fn appends_command_to_existing_empty_matcher_group() {
        let existing = json!({
            "hooks": {
                "UserPromptSubmit": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"existing-a\""
                            }
                        ]
                    }
                ],
                "Notification": [],
                "Stop": []
            }
        });

        let (settings, changed) =
            merge_hook_settings(existing, "node \"bridge\"").expect("merge should succeed");

        assert!(changed);
        assert_eq!(
            settings["hooks"]["UserPromptSubmit"][0]["hooks"],
            json!([
                {
                    "type": "command",
                    "command": "node \"existing-a\""
                },
                {
                    "type": "command",
                    "command": "node \"bridge\""
                }
            ])
        );
    }

    #[test]
    fn does_not_duplicate_existing_bridge_hook() {
        let existing = json!({
            "hooks": {
                "UserPromptSubmit": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"bridge\""
                            }
                        ]
                    }
                ],
                "Notification": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"bridge\""
                            }
                        ]
                    }
                ],
                "PreToolUse": [
                    {
                        "matcher": "AskUserQuestion",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"bridge\""
                            }
                        ]
                    }
                ],
                "PostToolUse": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"bridge\""
                            }
                        ]
                    }
                ],
                "Stop": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"bridge\""
                            }
                        ]
                    }
                ]
            }
        });

        let (settings, changed) =
            merge_hook_settings(existing.clone(), "node \"bridge\"").expect("merge should succeed");

        assert!(!changed);
        assert_eq!(settings, existing);
    }

    #[test]
    fn detects_existing_bridge_hooks_without_needing_an_exact_command_path() {
        let existing = json!({
            "hooks": {
                "UserPromptSubmit": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"C:/code/claude-status-light/bridge/claude-hook.mjs\""
                            }
                        ]
                    }
                ],
                "Notification": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"/Users/shan/Claude Status Light/bridge/claude-hook.mjs\""
                            }
                        ]
                    }
                ],
                "PreToolUse": [
                    {
                        "matcher": "AskUserQuestion",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"/Users/shan/Claude Status Light/bridge/claude-hook.mjs\""
                            }
                        ]
                    }
                ],
                "PostToolUse": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"/Users/shan/Claude Status Light/bridge/claude-hook.mjs\""
                            }
                        ]
                    }
                ],
                "Stop": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"/tmp/Claude Status Light/bridge/claude-hook.mjs\""
                            }
                        ]
                    }
                ]
            }
        });

        assert!(has_existing_bridge_hooks(&existing));
    }

    #[test]
    fn ignores_unrelated_claude_hook_commands_when_detecting_existing_bridge_hooks() {
        let existing = json!({
            "hooks": {
                "UserPromptSubmit": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"/tmp/bridge/claude-hook.mjs\""
                            }
                        ]
                    }
                ],
                "Notification": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"C:/tools/claude-hook.mjs\""
                            }
                        ]
                    }
                ],
                "PreToolUse": [
                    {
                        "matcher": "AskUserQuestion",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "python \"C:/claude-status-light/bridge/claude-hook.mjs\""
                            }
                        ]
                    }
                ],
                "Stop": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"C:/tmp/not-claude-status-light/bridge/claude-hook.mjs\""
                            }
                        ]
                    }
                ]
            }
        });

        assert!(!has_existing_bridge_hooks(&existing));
    }

    #[test]
    fn detects_existing_bridge_hooks_through_relative_and_windows_verbatim_paths() {
        let existing = json!({
            "hooks": {
                "UserPromptSubmit": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"C:/code/claude-status-light/src-tauri/../bridge/claude-hook.mjs\""
                            }
                        ]
                    }
                ],
                "Notification": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"//?/C:/Users/shan/Desktop/Claude Status Light_0.1.0_x64_portable/bridge/claude-hook.mjs\""
                            }
                        ]
                    }
                ],
                "PreToolUse": [
                    {
                        "matcher": "AskUserQuestion",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"//?/C:/Users/shan/Desktop/Claude Status Light_1/bridge/claude-hook.mjs\""
                            }
                        ]
                    }
                ],
                "PostToolUse": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"C:/Users/shan/Desktop/Claude Status Light Portable/bridge/claude-hook.mjs\""
                            }
                        ]
                    }
                ],
                "Stop": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"C:/Users/shan/Desktop/Claude Status Light Portable/./bridge/claude-hook.mjs\""
                            }
                        ]
                    }
                ]
            }
        });

        assert!(has_existing_bridge_hooks(&existing));
    }

    #[test]
    fn requires_all_three_events_to_count_as_existing_configuration() {
        let partial = json!({
            "hooks": {
                "UserPromptSubmit": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"C:/code/claude-status-light/bridge/claude-hook.mjs\""
                            }
                        ]
                    }
                ]
            }
        });

        assert!(!has_existing_bridge_hooks(&partial));
    }

    #[test]
    fn detects_existing_bridge_hooks_only_when_ask_user_question_pretool_hook_is_present() {
        let missing_pretool = json!({
            "hooks": {
                "UserPromptSubmit": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"C:/code/claude-status-light/bridge/claude-hook.mjs\""
                            }
                        ]
                    }
                ],
                "Notification": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"C:/code/claude-status-light/bridge/claude-hook.mjs\""
                            }
                        ]
                    }
                ],
                "PostToolUse": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"C:/code/claude-status-light/bridge/claude-hook.mjs\""
                            }
                        ]
                    }
                ],
                "Stop": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"C:/code/claude-status-light/bridge/claude-hook.mjs\""
                            }
                        ]
                    }
                ]
            }
        });

        let complete = json!({
            "hooks": {
                "UserPromptSubmit": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"C:/code/claude-status-light/bridge/claude-hook.mjs\""
                            }
                        ]
                    }
                ],
                "Notification": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"C:/code/claude-status-light/bridge/claude-hook.mjs\""
                            }
                        ]
                    }
                ],
                "PreToolUse": [
                    {
                        "matcher": "AskUserQuestion",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"C:/code/claude-status-light/bridge/claude-hook.mjs\""
                            }
                        ]
                    }
                ],
                "PostToolUse": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"C:/code/claude-status-light/bridge/claude-hook.mjs\""
                            }
                        ]
                    }
                ],
                "Stop": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"C:/code/claude-status-light/bridge/claude-hook.mjs\""
                            }
                        ]
                    }
                ]
            }
        });

        assert!(!has_existing_bridge_hooks(&missing_pretool));
        assert!(has_existing_bridge_hooks(&complete));
    }

    #[test]
    fn preserves_non_empty_matcher_groups_and_adds_own_group() {
        let existing = json!({
            "hooks": {
                "UserPromptSubmit": [
                    {
                        "matcher": "project-a",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"existing-a\""
                            }
                        ]
                    }
                ],
                "Notification": [],
                "Stop": []
            }
        });

        let (settings, changed) =
            merge_hook_settings(existing, "node \"bridge\"").expect("merge should succeed");

        assert!(changed);
        assert_eq!(settings["hooks"]["UserPromptSubmit"].as_array().unwrap().len(), 2);
        assert_eq!(
            settings["hooks"]["UserPromptSubmit"][1],
            json!({
                "matcher": "",
                "hooks": [
                    {
                        "type": "command",
                        "command": "node \"bridge\""
                    }
                ]
            })
        );
    }

    #[test]
    fn replaces_stale_bridge_paths_with_current_command_and_preserves_unrelated_hooks() {
        let current_command =
            r#"node "C:/code/claude-status-light/bridge/claude-hook.mjs""#;
        let existing = json!({
            "hooks": {
                "UserPromptSubmit": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"keep-user\""
                            },
                            {
                                "type": "command",
                                "command": "node \"/tmp/Claude Status Light/bridge/claude-hook.mjs\""
                            }
                        ]
                    },
                    {
                        "matcher": "other-project",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"leave-this-group-alone\""
                            }
                        ]
                    }
                ],
                "Notification": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"keep-notification\""
                            },
                            {
                                "type": "command",
                                "command": "node \"D:/stale/claude-status-light/bridge/claude-hook.mjs\""
                            }
                        ]
                    }
                ],
                "PreToolUse": [
                    {
                        "matcher": "AskUserQuestion",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"keep-pretool\""
                            },
                            {
                                "type": "command",
                                "command": "node \"/Users/old/Claude Status Light/bridge/claude-hook.mjs\""
                            }
                        ]
                    },
                    {
                        "matcher": "OtherMatcher",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"leave-other-matcher\""
                            }
                        ]
                    }
                ],
                "Stop": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"keep-stop\""
                            },
                            {
                                "type": "command",
                                "command": "node \"../old/claude-status-light/bridge/claude-hook.mjs\""
                            }
                        ]
                    }
                ]
            }
        });

        let (settings, changed) =
            merge_hook_settings(existing, current_command).expect("merge should succeed");

        assert!(changed);
        assert_eq!(
            settings["hooks"]["UserPromptSubmit"][0]["hooks"],
            json!([
                {
                    "type": "command",
                    "command": "node \"keep-user\""
                },
                {
                    "type": "command",
                    "command": current_command
                }
            ])
        );
        assert_eq!(
            settings["hooks"]["Notification"][0]["hooks"],
            json!([
                {
                    "type": "command",
                    "command": "node \"keep-notification\""
                },
                {
                    "type": "command",
                    "command": current_command
                }
            ])
        );
        assert_eq!(
            settings["hooks"]["PreToolUse"][0]["hooks"],
            json!([
                {
                    "type": "command",
                    "command": "node \"keep-pretool\""
                },
                {
                    "type": "command",
                    "command": current_command
                }
            ])
        );
        assert_eq!(
            settings["hooks"]["Stop"][0]["hooks"],
            json!([
                {
                    "type": "command",
                    "command": "node \"keep-stop\""
                },
                {
                    "type": "command",
                    "command": current_command
                }
            ])
        );
        assert_eq!(
            settings["hooks"]["UserPromptSubmit"][1],
            json!({
                "matcher": "other-project",
                "hooks": [
                    {
                        "type": "command",
                        "command": "node \"leave-this-group-alone\""
                    }
                ]
            })
        );
        assert_eq!(
            settings["hooks"]["PreToolUse"][1],
            json!({
                "matcher": "OtherMatcher",
                "hooks": [
                    {
                        "type": "command",
                        "command": "node \"leave-other-matcher\""
                    }
                ]
            })
        );
    }

    #[test]
    fn does_not_rewrite_when_current_command_is_already_only_bridge_path() {
        let current_command =
            r#"node "C:/code/claude-status-light/bridge/claude-hook.mjs""#;
        let existing = json!({
            "hooks": {
                "UserPromptSubmit": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"keep-user\""
                            },
                            {
                                "type": "command",
                                "command": current_command
                            }
                        ]
                    }
                ],
                "Notification": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"keep-notification\""
                            },
                            {
                                "type": "command",
                                "command": current_command
                            }
                        ]
                    }
                ],
                "PreToolUse": [
                    {
                        "matcher": "AskUserQuestion",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"keep-pretool\""
                            },
                            {
                                "type": "command",
                                "command": current_command
                            }
                        ]
                    }
                ],
                "PostToolUse": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"keep-posttool\""
                            },
                            {
                                "type": "command",
                                "command": current_command
                            }
                        ]
                    }
                ],
                "Stop": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"keep-stop\""
                            },
                            {
                                "type": "command",
                                "command": current_command
                            }
                        ]
                    }
                ]
            }
        });

        let (settings, changed) = merge_hook_settings(existing.clone(), current_command)
            .expect("merge should succeed");

        assert!(!changed);
        assert_eq!(settings, existing);
    }

    #[test]
    fn collapses_duplicate_current_bridge_commands_to_one_per_required_group() {
        let current_command =
            r#"node "C:/code/claude-status-light/bridge/claude-hook.mjs""#;
        let existing = json!({
            "hooks": {
                "UserPromptSubmit": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"keep-user\""
                            },
                            {
                                "type": "command",
                                "command": current_command
                            },
                            {
                                "type": "command",
                                "command": current_command
                            }
                        ]
                    }
                ],
                "Notification": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": current_command
                            },
                            {
                                "type": "command",
                                "command": current_command
                            }
                        ]
                    }
                ],
                "PreToolUse": [
                    {
                        "matcher": "AskUserQuestion",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"keep-pretool\""
                            },
                            {
                                "type": "command",
                                "command": current_command
                            },
                            {
                                "type": "command",
                                "command": current_command
                            }
                        ]
                    }
                ],
                "Stop": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"keep-stop\""
                            },
                            {
                                "type": "command",
                                "command": current_command
                            },
                            {
                                "type": "command",
                                "command": current_command
                            }
                        ]
                    }
                ]
            }
        });

        let (settings, changed) = merge_hook_settings(existing, current_command)
            .expect("merge should succeed");

        assert!(changed);
        assert_eq!(
            settings["hooks"]["UserPromptSubmit"][0]["hooks"],
            json!([
                {
                    "type": "command",
                    "command": "node \"keep-user\""
                },
                {
                    "type": "command",
                    "command": current_command
                }
            ])
        );
        assert_eq!(
            settings["hooks"]["Notification"][0]["hooks"],
            json!([
                {
                    "type": "command",
                    "command": current_command
                }
            ])
        );
        assert_eq!(
            settings["hooks"]["PreToolUse"][0]["hooks"],
            json!([
                {
                    "type": "command",
                    "command": "node \"keep-pretool\""
                },
                {
                    "type": "command",
                    "command": current_command
                }
            ])
        );
        assert_eq!(
            settings["hooks"]["Stop"][0]["hooks"],
            json!([
                {
                    "type": "command",
                    "command": "node \"keep-stop\""
                },
                {
                    "type": "command",
                    "command": current_command
                }
            ])
        );
    }

    #[test]
    fn cleans_up_all_groups_that_share_a_required_matcher() {
        let current_command =
            r#"node "C:/code/claude-status-light/bridge/claude-hook.mjs""#;
        let existing = json!({
            "hooks": {
                "UserPromptSubmit": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": current_command
                            }
                        ]
                    },
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"/tmp/old-a/claude-status-light/bridge/claude-hook.mjs\""
                            },
                            {
                                "type": "command",
                                "command": current_command
                            },
                            {
                                "type": "command",
                                "command": "node \"keep-user\""
                            }
                        ]
                    }
                ],
                "Notification": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": current_command
                            }
                        ]
                    },
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"D:/old/claude-status-light/bridge/claude-hook.mjs\""
                            }
                        ]
                    }
                ],
                "PreToolUse": [
                    {
                        "matcher": "AskUserQuestion",
                        "hooks": [
                            {
                                "type": "command",
                                "command": current_command
                            }
                        ]
                    },
                    {
                        "matcher": "AskUserQuestion",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"../old/claude-status-light/bridge/claude-hook.mjs\""
                            },
                            {
                                "type": "command",
                                "command": "node \"keep-pretool\""
                            }
                        ]
                    }
                ],
                "Stop": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": current_command
                            }
                        ]
                    },
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": current_command
                            },
                            {
                                "type": "command",
                                "command": "node \"keep-stop\""
                            }
                        ]
                    }
                ]
            }
        });

        let (settings, changed) = merge_hook_settings(existing, current_command)
            .expect("merge should succeed");

        assert!(changed);
        assert_eq!(
            settings["hooks"]["UserPromptSubmit"][1]["hooks"],
            json!([
                {
                    "type": "command",
                    "command": "node \"keep-user\""
                }
            ])
        );
        assert_eq!(
            settings["hooks"]["Notification"][1]["hooks"],
            json!([])
        );
        assert_eq!(
            settings["hooks"]["PreToolUse"][1]["hooks"],
            json!([
                {
                    "type": "command",
                    "command": "node \"keep-pretool\""
                }
            ])
        );
        assert_eq!(
            settings["hooks"]["Stop"][1]["hooks"],
            json!([
                {
                    "type": "command",
                    "command": "node \"keep-stop\""
                }
            ])
        );
    }

    #[test]
    fn repairs_malformed_matched_target_groups_instead_of_failing() {
        let current_command =
            r#"node "C:/code/claude-status-light/bridge/claude-hook.mjs""#;
        let existing = json!({
            "hooks": {
                "UserPromptSubmit": [
                    {
                        "matcher": ""
                    }
                ],
                "Notification": [
                    {
                        "matcher": "",
                        "hooks": {"type": "command"}
                    }
                ],
                "PreToolUse": [
                    {
                        "matcher": "AskUserQuestion",
                        "hooks": "broken"
                    }
                ],
                "Stop": [
                    {
                        "matcher": "",
                        "hooks": null
                    }
                ]
            }
        });

        let (settings, changed) = merge_hook_settings(existing, current_command)
            .expect("merge should self-heal malformed groups");

        assert!(changed);
        assert_eq!(
            settings["hooks"]["UserPromptSubmit"][0]["hooks"],
            json!([
                {
                    "type": "command",
                    "command": current_command
                }
            ])
        );
        assert_eq!(
            settings["hooks"]["Notification"][0]["hooks"],
            json!([
                {
                    "type": "command",
                    "command": current_command
                }
            ])
        );
        assert_eq!(
            settings["hooks"]["PreToolUse"][0]["hooks"],
            json!([
                {
                    "type": "command",
                    "command": current_command
                }
            ])
        );
        assert_eq!(
            settings["hooks"]["Stop"][0]["hooks"],
            json!([
                {
                    "type": "command",
                    "command": current_command
                }
            ])
        );
    }

    #[test]
    fn removes_old_repo_and_portable_paths_that_point_to_claude_status_light() {
        let current_command =
            r#"node "C:/Users/shan/Desktop/Claude Status Light_2/bridge/claude-hook.mjs""#;
        let existing = json!({
            "hooks": {
                "UserPromptSubmit": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"C:/code/claude-status-light/src-tauri/../bridge/claude-hook.mjs\""
                            },
                            {
                                "type": "command",
                                "command": "node \"keep-user\""
                            }
                        ]
                    }
                ],
                "Notification": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"C:/Users/shan/Desktop/Claude Status Light_0.1.0_x64_portable/bridge/claude-hook.mjs\""
                            }
                        ]
                    }
                ],
                "PreToolUse": [
                    {
                        "matcher": "AskUserQuestion",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"//?/C:/Users/shan/Desktop/Claude Status Light_1/bridge/claude-hook.mjs\""
                            },
                            {
                                "type": "command",
                                "command": "node \"keep-pretool\""
                            }
                        ]
                    }
                ],
                "Stop": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node \"C:/Users/shan/Desktop/Claude Status Light Portable/./bridge/claude-hook.mjs\""
                            },
                            {
                                "type": "command",
                                "command": "node \"keep-stop\""
                            }
                        ]
                    }
                ]
            }
        });

        let (settings, changed) = merge_hook_settings(existing, current_command)
            .expect("merge should succeed");

        assert!(changed);
        assert_eq!(
            settings["hooks"]["UserPromptSubmit"][0]["hooks"],
            json!([
                {
                    "type": "command",
                    "command": "node \"keep-user\""
                },
                {
                    "type": "command",
                    "command": current_command
                }
            ])
        );
        assert_eq!(
            settings["hooks"]["Notification"][0]["hooks"],
            json!([
                {
                    "type": "command",
                    "command": current_command
                }
            ])
        );
        assert_eq!(
            settings["hooks"]["PreToolUse"][0]["hooks"],
            json!([
                {
                    "type": "command",
                    "command": "node \"keep-pretool\""
                },
                {
                    "type": "command",
                    "command": current_command
                }
            ])
        );
        assert_eq!(
            settings["hooks"]["Stop"][0]["hooks"],
            json!([
                {
                    "type": "command",
                    "command": "node \"keep-stop\""
                },
                {
                    "type": "command",
                    "command": current_command
                }
            ])
        );
    }
}
