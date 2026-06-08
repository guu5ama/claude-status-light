# Claude Status Light

Floating desktop status light for Claude Code on macOS and Windows.

- `yellow solid` = Claude is running
- `red blinking` = Claude is waiting for you
- `green solid` = Claude finished answering without follow-up

The app is a small Tauri window plus a local hook bridge:

1. Claude Code fires `UserPromptSubmit`, `Notification`, `PreToolUse(AskUserQuestion)`, and `Stop` hooks.
2. `bridge/claude-hook.mjs` reads the hook payload from stdin.
3. The bridge writes `public/state/state.json`.
4. The Tauri app polls that state file and updates the light.

On startup, the app also tries to configure Claude hooks automatically.

Status sounds now prefer local MP3 files in `public/sounds/` and only fall back to synthesized tones if file playback is unavailable.

## Requirements

- Node.js 20+
- npm
- Rust + Cargo
- Tauri prerequisites for your platform

On Windows, you also need Visual Studio C++ build tools for Tauri native builds.
On macOS, you need Xcode Command Line Tools for native Tauri builds.

## Install

```powershell
npm install
```

## Run

Frontend and bridge checks:

```powershell
npm test
npm run build
```

Desktop app:

```powershell
npm run tauri:dev
```

Production package:

```powershell
npm run tauri:build
```

Platform-specific release commands:

Windows:

```powershell
npm run tauri:build:windows
```

macOS:

```bash
npm run tauri:build:mac
```

Expected package output:

- Windows: `.exe` installer and `.msi`
- macOS: native `.app` and `.dmg`

Important:

- run `npm run tauri:build:windows` on Windows
- run `npm run tauri:build:mac` on a Mac
- this Windows machine cannot emit a real macOS `.app/.dmg`; Tauri's macOS bundle docs require running `tauri build` on a Mac computer

Portable Windows package:

- a no-install zip can be created from the release `exe` plus the `bridge/` folder
- the portable build still requires local Node.js for Claude hook execution
- portable and installed release builds store runtime state under the user-local app data directory instead of a repo-relative path
- portable and moved builds self-heal Claude hook paths on startup; the current app location is the only valid Claude Status Light hook path

## Platform Notes

Windows:

- Claude settings path: `C:\Users\<user>\.claude\settings.json`
- native package shape: `.exe` installer / Windows bundle output from Tauri
- release command: `npm run tauri:build:windows`

macOS:

- Claude settings path: `/Users/<user>/.claude/settings.json`
- native package shape: `.app` and optionally `.dmg`
- recommended prerequisites: `xcode-select --install`
- release command: `npm run tauri:build:mac`

This project is intended to ship as native desktop app bundles on both platforms, not as Docker containers.

## Automatic Claude Hook Setup

On first launch, the app tries to:

1. detect `~/.claude/settings.json`
2. merge in the four required hook events
3. back up the original file before any real write

Behavior:

- existing unrelated hooks are preserved
- existing matching bridge hooks are not duplicated
- existing valid `claude-hook.mjs` entries are treated as already configured, even if the app does not need to rewrite them
- the running app location is treated as the only valid Claude Status Light hook path
- stale Claude Status Light hook paths that point at old repos or old portable folders are backed up and rewritten to the current app location
- UTF-8 BOM in `settings.json` is tolerated during parsing
- if a write happens, Claude Code should be reopened so it reloads `settings.json`

The tray menu also exposes `Configure Claude Hooks` so you can rerun setup manually.

When startup or `Configure Claude Hooks` rewrites stale Claude Status Light paths:

1. the app first backs up the previous `settings.json`
2. unrelated hook commands stay intact
3. Claude Code must be reopened so it reloads the rewritten hook path

## Manual Claude Hook Setup

For development checkouts or manual troubleshooting, you can inspect or edit the file yourself and point Claude at a local checkout path like this:

```json
{
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
}
```

macOS uses the same hook schema, but the command path should point to the local macOS checkout or bundled app resource path. Development example:

```json
{
  "hooks": {
    "UserPromptSubmit": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "node \"/Users/<user>/code/claude-status-light/bridge/claude-hook.mjs\""
          }
        ]
      }
    ]
  }
}
```

This repo-path example is for a local development checkout. For installed or portable app builds, let the app self-configure so the active hook path follows the current app location.

After editing settings manually:

1. Reopen Claude Code so it reloads `settings.json`.
2. In the tray menu, click `Reconnect Session`.
3. Send a new message in Claude Code.

## State Semantics

- `UserPromptSubmit` -> `running`
- `Notification(permission_prompt | idle_prompt | elicitation_dialog)` -> `pending_user`
- `PreToolUse(AskUserQuestion)` -> `pending_user`
- `Stop`:
  - obvious wait-for-user language -> `pending_user`
  - obvious completion language -> `done`
  - direct answer with no follow-up request -> `done`

This means a plain factual answer should turn green, even if it does not say "done".

## Tray Controls

- `Sound On/Off`
- `Open/Hide`
- `Configure Claude Hooks`
- `Reconnect Session`
- `Quit`

`Reconnect Session` resets `state.json` to `idle_unbound` so the next real Claude event can bind a fresh session.

## Current UI

- vertical three-lens traffic light body
- no bottom support post
- speaker button below the status area so the window stays narrow
- setup success note can show `SETUP OK` and the backup path when the app actually rewrites Claude settings
- default dev window is sized to fit the signal, label, and setup note without scrollbars

## State File

Default behavior:

```text
C:/code/claude-status-light/public/state/state.json
```

- development checkouts use the repo-local `public/state/state.json`
- release and portable builds default to a user-local app-data state path instead of a repo-relative path

Override it for both the bridge and the app with:

```text
CLAUDE_STATUS_LIGHT_STATE_PATH
```

## Optional Debug Logging

Debug logging is off by default.

Enable it only when diagnosing hook behavior:

```powershell
$env:CLAUDE_STATUS_LIGHT_DEBUG = "1"
```

Optional custom log path:

```powershell
$env:CLAUDE_STATUS_LIGHT_DEBUG_LOG_PATH = "C:/code/claude-status-light/public/state/hook-debug.jsonl"
```

When enabled, the bridge appends JSONL entries for received hooks, ignored events, merge ignores, writes, and errors.

## Sound Assets

- `public/sounds/done.mp3`
- `public/sounds/pending.mp3`
- source and license notes: [SOURCES.md](C:/code/claude-status-light/public/sounds/SOURCES.md)

## Manual Bridge Smoke Test

```powershell
'{"session_id":"s1","hook_event_name":"UserPromptSubmit"}' | node .\bridge\claude-hook.mjs
```

Expected result:

- `public/state/state.json` changes to `running`
- the light turns yellow

## Common Issues

`The light stays idle_unbound`

- Claude hooks are missing or malformed in `~/.claude/settings.json`
- Claude Code has not been reopened after editing settings
- `Reconnect Session` was not used after manual bridge tests

`The label says SETUP NEEDED`

- automatic Claude hook configuration failed
- rerun `Configure Claude Hooks` from the tray
- inspect `~/.claude/settings.json`
- if needed, enable debug logging and retry

`The light stays yellow forever`

- inspect the last hook payload by enabling debug logging
- check whether `Stop` is arriving
- check whether `PreToolUse` for `AskUserQuestion` is configured; that path is what turns plan-mode user questions red immediately

`The light turns red after a normal answer`

- this usually means Claude's last message looked like a follow-up request
- inspect `lastMessageText` in `public/state/state.json`

`No tray icon or native app build`

- confirm Rust, Cargo, and Tauri prerequisites are installed

`macOS build works locally but app will not open on another Mac`

- unsigned local builds may be blocked by Gatekeeper
- for wider distribution you will likely need Apple code signing, notarization, and a signed `.app` / `.dmg`

## macOS Verification Checklist

- GitHub Actions `macOS Verify` workflow passes on `macos-latest`
- the workflow runs `npm test`, `npm run build`, and `npm run tauri:build:mac -- --no-sign`
- the workflow uploads unsigned `.app` and `.dmg` artifacts for inspection
- prerequisites installed: `xcode-select --install`, Rust, Node.js
- `npm run tauri:dev` launches a native macOS window
- tray icon appears in the macOS menu bar area
- transparent always-on-top window can be dragged
- local sounds play after first user interaction
- automatic Claude hook configuration writes `/Users/<user>/.claude/settings.json` safely
- `UserPromptSubmit`, `PreToolUse(AskUserQuestion)`, `Notification`, and `Stop` all drive the correct light transitions
- `npm run tauri:build:mac` produces a working `.app` and `.dmg`

## Task 4 Handoff Snapshot

This is a point-in-time Windows release snapshot from the Task 4 handoff, not evergreen user documentation:

- `src-tauri/target/release/bundle/nsis/Claude Status Light_0.1.0_x64-setup.exe`
- `src-tauri/target/release/bundle/msi/Claude Status Light_0.1.0_x64_en-US.msi`
- `src-tauri/target/release/bundle/portable/Claude Status Light_0.1.0_x64_portable.zip`

Verification snapshot from that handoff:

- `cargo test` passed
- `npm test` passed
- `npm run build` passed
- `npm run tauri:build:windows` passed and rebuilt the Windows release `.exe` and `.msi`
- the portable folder was rebuilt from `src-tauri/target/release/claude-status-light.exe` plus the runtime bridge files only, and `src-tauri/target/release/bundle/portable/Claude Status Light_0.1.0_x64_portable.zip` was regenerated successfully

Prepared source zip to move onto a Mac and build there:

- `C:/code/claude-status-light-mac-build-src.zip`

## Project Structure

- [bridge](C:/code/claude-status-light/bridge/README.md)
- [src](C:/code/claude-status-light/src)
- [src-tauri](C:/code/claude-status-light/src-tauri)
