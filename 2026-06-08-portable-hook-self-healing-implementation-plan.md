# Portable Hook Self-Healing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make installed and portable Claude Status Light builds automatically rewrite Claude hook paths to the currently running app location, removing stale Claude Status Light paths without touching unrelated hooks.

**Architecture:** Keep the bridge path resolution in the Tauri backend, but upgrade the Rust settings merger from “append if missing” to “canonicalize to exactly one current Claude Status Light command per required event/matcher pair.” Surface richer setup status back to the React UI so startup can show `ALREADY CONFIGURED`, `HOOKS UPDATED`, or `SETUP NEEDED` with the active bridge path and backup path.

**Tech Stack:** Rust + Tauri backend, React + TypeScript frontend, existing Vitest and `cargo test` suites.

---

## File Structure

**Modify**
- `src-tauri/src/claude_settings.rs`
  - Own the Claude hook detection, stale-path pruning, merge result classification, and Rust unit tests.
- `src-tauri/src/lib.rs`
  - Use the richer merge result and expose the new setup status payload fields.
- `src/lib/claude-setup.ts`
  - Expand setup status types and frontend notice formatting.
- `src/lib/__tests__/claude-setup.test.ts`
  - Cover the new `HOOKS UPDATED` and `ALREADY CONFIGURED` UI text.
- `README.md`
  - Document self-healing startup behavior for portable builds.
- `2026-06-07-claude-status-light-design.md`
  - Keep the design synchronized with the implementation outcome if names or payload fields change during coding.

**No new runtime modules expected**
- Keep the change focused inside the existing Rust settings merger instead of adding another abstraction layer.

**Verification**
- `cargo test`
- `npm test -- src/lib/__tests__/claude-setup.test.ts`
- `npm run build`
- `npm run tauri:build:windows`

---

### Task 1: Make the Rust merger canonicalize Claude Status Light hook paths

**Files:**
- Modify: `src-tauri/src/claude_settings.rs`
- Test: `src-tauri/src/claude_settings.rs` (existing inline Rust unit tests)

- [ ] **Step 1: Write the failing Rust tests for stale-path cleanup**

Add tests that describe the new canonical behavior before touching the merge logic:

```rust
#[test]
fn replaces_stale_claude_status_light_paths_with_the_current_command() {
    let existing = json!({
        "hooks": {
            "UserPromptSubmit": [
                {
                    "matcher": "",
                    "hooks": [
                        {
                            "type": "command",
                            "command": "node \"C:/old/location/bridge/claude-hook.mjs\""
                        },
                        {
                            "type": "command",
                            "command": "node \"C:/some/other/tool.mjs\""
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
                            "command": "node \"C:/old/location/bridge/claude-hook.mjs\""
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
                            "command": "node \"C:/old/location/bridge/claude-hook.mjs\""
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
                            "command": "node \"C:/old/location/bridge/claude-hook.mjs\""
                        }
                    ]
                }
            ]
        }
    });

    let (settings, changed) = merge_hook_settings(
        existing,
        r#"node "C:/portable/current/bridge/claude-hook.mjs""#,
    )
    .expect("merge should succeed");

    assert!(changed);
    assert_eq!(
        settings["hooks"]["UserPromptSubmit"][0]["hooks"],
        json!([
            {
                "type": "command",
                "command": r#"node "C:/some/other/tool.mjs""#
            },
            {
                "type": "command",
                "command": r#"node "C:/portable/current/bridge/claude-hook.mjs""#
            }
        ])
    );
}

#[test]
fn does_not_rewrite_when_the_current_path_is_already_the_only_claude_status_light_path() {
    let existing = json!({
        "hooks": {
            "UserPromptSubmit": [
                {
                    "matcher": "",
                    "hooks": [
                        {
                            "type": "command",
                            "command": r#"node "C:/portable/current/bridge/claude-hook.mjs""#
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
                            "command": r#"node "C:/portable/current/bridge/claude-hook.mjs""#
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
                            "command": r#"node "C:/portable/current/bridge/claude-hook.mjs""#
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
                            "command": r#"node "C:/portable/current/bridge/claude-hook.mjs""#
                        }
                    ]
                }
            ]
        }
    });

    let (settings, changed) = merge_hook_settings(
        existing.clone(),
        r#"node "C:/portable/current/bridge/claude-hook.mjs""#,
    )
    .expect("merge should succeed");

    assert!(!changed);
    assert_eq!(settings, existing);
}
```

- [ ] **Step 2: Run the Rust tests to verify they fail for the expected reason**

Run:

```powershell
$env:PATH='C:\Users\shan\.cargo\bin;'+$env:PATH
cargo test
```

Expected:
- New stale-path cleanup test fails because `merge_hook_settings` currently appends the current command instead of removing stale Claude Status Light paths.

- [ ] **Step 3: Implement minimal canonical merge behavior**

Refactor `merge_hook_settings` so it filters each target matcher group before deciding whether to append the current command:

```rust
fn is_exact_command_hook(hook: &Value, command: &str) -> bool {
    hook.get("type").and_then(Value::as_str) == Some("command")
        && hook.get("command").and_then(Value::as_str) == Some(command)
}

fn retain_non_stale_hooks(hooks: &mut Vec<Value>, command: &str) -> bool {
    let before = hooks.clone();
    hooks.retain(|hook| {
        let Some(existing_command) = hook.get("command").and_then(Value::as_str) else {
            return true;
        };

        if !is_bridge_hook_command(existing_command) {
            return true;
        }

        existing_command == command
    });

    before != *hooks
}

pub fn merge_hook_settings(existing: Value, command: &str) -> Result<(Value, bool), String> {
    // existing object setup unchanged
    let command_hook = json!({
        "type": "command",
        "command": command
    });
    let mut changed = false;

    for target in TARGET_HOOKS {
        // existing event / matcher lookup unchanged
        if matched_target_matcher {
            let hooks = /* existing hooks array lookup */;

            if retain_non_stale_hooks(hooks, command) {
                changed = true;
            }

            if !hooks.iter().any(|hook| is_exact_command_hook(hook, command)) {
                hooks.push(command_hook.clone());
                changed = true;
            }

            continue;
        }

        groups.push(json!({
            "matcher": target.matcher,
            "hooks": [command_hook.clone()]
        }));
        changed = true;
    }

    Ok((settings, changed))
}
```

Implementation rules:
- Only remove stale commands where `is_bridge_hook_command(existing_command)` is true and `existing_command != command`.
- Preserve unrelated hooks in the same matcher group.
- Preserve unrelated matcher groups for the same event.
- Keep the existing error messages for malformed `hooks` structures.

- [ ] **Step 4: Run the Rust tests to verify the merger now passes**

Run:

```powershell
$env:PATH='C:\Users\shan\.cargo\bin;'+$env:PATH
cargo test
```

Expected:
- All existing Rust tests pass.
- The new stale-path cleanup tests pass.

- [ ] **Step 5: Record a local checkpoint**

This workspace is not a git repo, so do not fabricate a commit. Record the checkpoint by listing the changed file and the passing command in your execution notes:

```text
Checkpoint: src-tauri/src/claude_settings.rs updated, cargo test green.
```

---

### Task 2: Return richer setup status from the Tauri backend

**Files:**
- Modify: `src-tauri/src/claude_settings.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/claude_settings.rs` (inline unit tests remain the guardrail)

- [ ] **Step 1: Write the failing status-shape test in Rust**

Add a Rust unit test that documents the new distinction between “already current” and “rewritten to current” using a helper that interprets the merge result:

```rust
#[test]
fn reports_already_configured_only_when_no_rewrite_is_needed() {
    let existing = json!({
        "hooks": {
            "UserPromptSubmit": [{ "matcher": "", "hooks": [{ "type": "command", "command": r#"node "C:/portable/current/bridge/claude-hook.mjs""# }] }],
            "Notification": [{ "matcher": "", "hooks": [{ "type": "command", "command": r#"node "C:/portable/current/bridge/claude-hook.mjs""# }] }],
            "PreToolUse": [{ "matcher": "AskUserQuestion", "hooks": [{ "type": "command", "command": r#"node "C:/portable/current/bridge/claude-hook.mjs""# }] }],
            "Stop": [{ "matcher": "", "hooks": [{ "type": "command", "command": r#"node "C:/portable/current/bridge/claude-hook.mjs""# }] }]
        }
    });

    let (settings, changed) = merge_hook_settings(
        existing.clone(),
        r#"node "C:/portable/current/bridge/claude-hook.mjs""#,
    )
    .expect("merge should succeed");

    assert!(!changed);
    assert_eq!(settings, existing);
}
```

This test will pass after Task 1. The failing behavior for this task is at the application-status layer: `run_claude_setup` still returns the generic `Configured` / `AlreadyConfigured` messages and has no active-bridge-path field.

- [ ] **Step 2: Run `cargo test` to confirm the current backend status payload is not yet covered**

Run:

```powershell
$env:PATH='C:\Users\shan\.cargo\bin;'+$env:PATH
cargo test
```

Expected:
- Tests still pass at the merger level.
- You confirm there is no structured `activeBridgePath` field or `HOOKS UPDATED` message in `ClaudeSetupStatus`.

- [ ] **Step 3: Extend `ClaudeSetupStatus` and `run_claude_setup` minimally**

Add `active_bridge_path` and update the success messages:

```rust
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
```

Update `run_claude_setup` to:
- resolve the current bridge path first
- build `hook_command` from that path
- call `merge_hook_settings`
- return:

```rust
ClaudeSetupStatus {
    kind: ClaudeSetupStatusKind::AlreadyConfigured,
    message: "Claude hook bridge is already configured for this app location.".into(),
    settings_path: settings_path.display().to_string(),
    backup_path: None,
    active_bridge_path: Some(bridge_script_path.display().to_string()),
    wrote_changes: false,
    requires_claude_restart: false,
}
```

and on a real rewrite:

```rust
ClaudeSetupStatus {
    kind: ClaudeSetupStatusKind::Configured,
    message: "Claude hook bridge paths were updated for this app location.".into(),
    settings_path: settings_path.display().to_string(),
    backup_path: backup_path.map(|path| path.display().to_string()),
    active_bridge_path: Some(bridge_script_path.display().to_string()),
    wrote_changes: true,
    requires_claude_restart: true,
}
```

Also update `initial_setup_status()` in `src-tauri/src/lib.rs` to set `active_bridge_path: None`.

- [ ] **Step 4: Run Rust tests again**

Run:

```powershell
$env:PATH='C:\Users\shan\.cargo\bin;'+$env:PATH
cargo test
```

Expected:
- All Rust tests still pass after the status payload change.

- [ ] **Step 5: Record a local checkpoint**

```text
Checkpoint: src-tauri/src/claude_settings.rs + src-tauri/src/lib.rs updated, cargo test green.
```

---

### Task 3: Update the frontend setup notice wording

**Files:**
- Modify: `src/lib/claude-setup.ts`
- Test: `src/lib/__tests__/claude-setup.test.ts`

- [ ] **Step 1: Write the failing Vitest cases for the new status wording**

Extend `src/lib/__tests__/claude-setup.test.ts` with concrete UI expectations:

```ts
import { describe, expect, it } from 'vitest';
import { getSetupNotice, getStatusLabelText } from '../claude-setup';

describe('claude setup notice', () => {
  it('shows HOOKS UPDATED with backup and active path after a rewrite', () => {
    const notice = getSetupNotice({
      kind: 'configured',
      message: 'Claude hook bridge paths were updated for this app location.',
      settingsPath: 'C:/Users/shan/.claude/settings.json',
      backupPath: 'C:/Users/shan/.claude/settings.json.bak-123',
      activeBridgePath: 'C:/Users/shan/Desktop/Claude Status Light_0.1.0_x64_portable/bridge/claude-hook.mjs',
      wroteChanges: true,
      requiresClaudeRestart: true
    });

    expect(notice).toEqual({
      tone: 'success',
      title: 'HOOKS UPDATED',
      detail:
        'Active: C:/Users/shan/Desktop/Claude Status Light_0.1.0_x64_portable/bridge/claude-hook.mjs\nBackup: C:/Users/shan/.claude/settings.json.bak-123'
    });
  });

  it('shows ALREADY CONFIGURED in the label path when the current bridge path is already active', () => {
    const label = getStatusLabelText('idle_unbound', {
      kind: 'already_configured',
      message: 'Claude hook bridge is already configured for this app location.',
      settingsPath: 'C:/Users/shan/.claude/settings.json',
      backupPath: null,
      activeBridgePath: 'C:/Users/shan/Desktop/Claude Status Light_0.1.0_x64_portable/bridge/claude-hook.mjs',
      wroteChanges: false,
      requiresClaudeRestart: false
    });

    expect(label).toBe('IDLE UNBOUND');
  });
});
```

- [ ] **Step 2: Run the frontend tests to verify the new expectations fail**

Run:

```powershell
npm test -- src/lib/__tests__/claude-setup.test.ts
```

Expected:
- Failure because `activeBridgePath` is not part of the TypeScript interface yet.
- Failure because `getSetupNotice()` still returns `SETUP OK` instead of `HOOKS UPDATED`.

- [ ] **Step 3: Implement the minimal TypeScript notice update**

Update `src/lib/claude-setup.ts`:

```ts
export interface ClaudeSetupStatus {
  kind: ClaudeSetupStatusKind;
  message: string;
  settingsPath: string;
  backupPath: string | null;
  activeBridgePath: string | null;
  wroteChanges: boolean;
  requiresClaudeRestart: boolean;
}

export function getSetupNotice(
  setupStatus: ClaudeSetupStatus | null
): ClaudeSetupNotice | null {
  if (!setupStatus) {
    return null;
  }

  if (setupStatus.kind === 'configured') {
    const details = [
      setupStatus.activeBridgePath ? `Active: ${setupStatus.activeBridgePath}` : null,
      setupStatus.backupPath ? `Backup: ${setupStatus.backupPath}` : null
    ].filter((value): value is string => Boolean(value));

    return {
      tone: 'success',
      title: 'HOOKS UPDATED',
      detail: details.length > 0 ? details.join('\n') : setupStatus.message
    };
  }

  if (setupStatus.kind === 'failed') {
    return {
      tone: 'error',
      title: 'SETUP NEEDED',
      detail: setupStatus.message
    };
  }

  return null;
}
```

Keep `already_configured` silent in the notice area for now; the main change is the rewritten-path success message.

- [ ] **Step 4: Run the targeted frontend tests**

Run:

```powershell
npm test -- src/lib/__tests__/claude-setup.test.ts
```

Expected:
- The new `HOOKS UPDATED` test passes.
- Existing setup-status tests continue to pass.

- [ ] **Step 5: Record a local checkpoint**

```text
Checkpoint: src/lib/claude-setup.ts updated, targeted frontend tests green.
```

---

### Task 4: Document the self-healing portable behavior and run full verification

**Files:**
- Modify: `README.md`
- Modify: `2026-06-07-claude-status-light-design.md` only if the implementation changed field names or message text from this plan

- [ ] **Step 1: Update the README release notes**

Add concrete portable behavior notes near the existing portable package section:

```md
Portable hook-path self-healing:

- On startup, Claude Status Light treats the currently running app location as the only valid Claude Status Light hook path.
- If `~/.claude/settings.json` still points to an older repo checkout or older portable folder, startup backs up the file and rewrites the four required Claude hook events to the current app path.
- Unrelated hook commands are preserved.
- After a rewrite, reopen Claude Code so it reloads `settings.json`.
```

- [ ] **Step 2: Sync the design doc only if implementation wording drifted**

If the Rust and TypeScript implementation ended up with different user-facing strings than the design, update the relevant design sections so they match exactly:

```md
- `HOOKS UPDATED` when startup or manual configuration rewrites stale Claude Status Light hook paths to the current app path
- `ALREADY CONFIGURED` when the current app path is already the only active Claude Status Light hook path
```

If the implementation matches the current design, leave the design doc unchanged in this task.

- [ ] **Step 3: Run the full verification stack**

Run:

```powershell
$env:PATH='C:\Users\shan\.cargo\bin;'+$env:PATH
cargo test
npm test
npm run build
npm run tauri:build:windows
```

Expected:
- `cargo test` passes
- `npm test` passes
- `npm run build` passes
- `npm run tauri:build:windows` emits the refreshed Windows installer artifacts

- [ ] **Step 4: Rebuild the portable zip from the refreshed release**

Recreate the portable folder and zip after the Windows build:

```powershell
$portableRoot = 'C:\code\claude-status-light\src-tauri\target\release\bundle\portable'
$portableDir = Join-Path $portableRoot 'Claude Status Light Portable'
$bridgeDir = Join-Path $portableDir 'bridge'

New-Item -ItemType Directory -Force -Path $bridgeDir | Out-Null
Copy-Item -LiteralPath 'C:\code\claude-status-light\src-tauri\target\release\claude-status-light.exe' -Destination (Join-Path $portableDir 'claude-status-light.exe') -Force

$bridgeFiles = @(
  'append-debug-log.mjs',
  'classify-event.mjs',
  'claude-hook.mjs',
  'read-current-state.mjs',
  'runtime-paths.mjs',
  'write-state.mjs',
  'README.md'
)

foreach ($file in $bridgeFiles) {
  Copy-Item -LiteralPath (Join-Path 'C:\code\claude-status-light\bridge' $file) -Destination (Join-Path $bridgeDir $file) -Force
}

$zipPath = Join-Path $portableRoot 'Claude Status Light_0.1.0_x64_portable.zip'
Compress-Archive -Path (Join-Path $portableDir '*') -DestinationPath $zipPath -Force
```

- [ ] **Step 5: Record the final handoff**

Write down the final artifacts and verification evidence:

```text
Artifacts:
- src-tauri/target/release/bundle/nsis/Claude Status Light_0.1.0_x64-setup.exe
- src-tauri/target/release/bundle/msi/Claude Status Light_0.1.0_x64_en-US.msi
- src-tauri/target/release/bundle/portable/Claude Status Light_0.1.0_x64_portable.zip

Verification:
- cargo test
- npm test
- npm run build
- npm run tauri:build:windows
```
