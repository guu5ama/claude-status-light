# Claude Status Light

Floating desktop traffic light for Claude Code on macOS and Windows.

Claude Status Light sits on your desktop and mirrors Claude Code session state in one glance:

- `yellow solid` = Claude is running
- `red blinking` = Claude is waiting for you
- `green solid` = Claude answered and is done with the current turn

It is built with `Tauri + React`, plus a small local hook bridge that listens to Claude Code hooks and writes shared state for the desktop app.

## Why

Claude Code often lives inside a busy editor window. This app makes status ambient:

- keep coding or reading on another screen
- notice immediately when Claude needs input
- hear a sound when status changes
- run the same idea on Windows and macOS

## Download

Releases: [github.com/guu5ama/claude-status-light/releases](https://github.com/guu5ama/claude-status-light/releases)

Current release assets:

- Windows installer: `.exe`
- Windows installer: `.msi`
- Windows portable: `.zip`
- macOS test build: unsigned `aarch64 .dmg`

macOS note:

- the current DMG is for Apple Silicon
- it is unsigned, so Gatekeeper warnings are expected during testing

## How It Works

1. Claude Code fires hooks for `UserPromptSubmit`, `Notification`, `PreToolUse(AskUserQuestion)`, and `Stop`.
2. `bridge/claude-hook.mjs` receives the hook payload on `stdin`.
3. The bridge writes normalized state to a shared `state.json`.
4. The desktop app polls that state and updates the light and sounds.

The app also tries to configure Claude hooks automatically on startup.

## Status Semantics

- `UserPromptSubmit` -> `running`
- `Notification(permission_prompt | idle_prompt | elicitation_dialog)` -> `pending_user`
- `PreToolUse(AskUserQuestion)` -> `pending_user`
- `Stop`:
  - wait-for-user language -> `pending_user`
  - explicit completion language -> `done`
  - direct answer with no follow-up request -> `done`

This means a plain factual answer should turn green even if it does not literally say "done".

## Automatic Hook Setup

On startup, the app tries to manage `~/.claude/settings.json` for you.

Behavior:

- merges only the Claude Status Light hooks it needs
- preserves unrelated existing hooks
- backs up the original `settings.json` before any real write
- treats the current app location as the only valid Claude Status Light bridge path
- removes stale Claude Status Light paths from old repos or moved portable folders
- tolerates UTF-8 BOM in `settings.json`

If the app rewrites hooks:

- it shows `HOOKS UPDATED`
- it shows the active bridge path and backup path
- Claude Code should be reopened so it reloads the new hook path

Tray controls:

- `Open/Hide`
- `Sound On/Off`
- `Configure Claude Hooks`
- `Reconnect Session`
- `Quit`

`Reconnect Session` resets binding to `idle_unbound` so the next real Claude event can claim the app cleanly.

## End-User Notes

Windows:

- Claude settings path: `C:\Users\<user>\.claude\settings.json`
- installed and portable builds store runtime state under local app data

macOS:

- Claude settings path: `/Users/<user>/.claude/settings.json`
- app ships as native `.app` / `.dmg`, not Docker

Sounds:

- prefer local MP3 assets in `public/sounds/`
- fall back to synthesized tones if file playback is unavailable

## Development

Requirements:

- Node.js 20+
- npm
- Rust + Cargo
- Tauri prerequisites for your platform

Platform prerequisites:

- Windows: Visual Studio C++ Build Tools
- macOS: Xcode Command Line Tools via `xcode-select --install`

Install:

```powershell
npm install
```

Run tests and frontend build:

```powershell
npm test
npm run build
```

Run desktop app in development:

```powershell
npm run tauri:dev
```

Build release packages:

Windows:

```powershell
npm run tauri:build:windows
```

macOS:

```bash
npm run tauri:build:mac
```

Important:

- run `npm run tauri:build:windows` on Windows
- run `npm run tauri:build:mac` on a Mac, or use the GitHub Actions macOS workflow

## Manual Hook Example

Automatic setup is the default and recommended path. For development or troubleshooting, a local checkout can be wired manually like this:

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

After manual edits:

1. Reopen Claude Code.
2. Click `Reconnect Session`.
3. Send a new Claude message.

Bridge details: [bridge/README.md](bridge/README.md)

## Troubleshooting

`The light stays idle_unbound`

- Claude hooks are missing or malformed in `~/.claude/settings.json`
- Claude Code has not been reopened after hook changes
- `Reconnect Session` was not used after manual bridge tests

`The label says SETUP NEEDED`

- automatic Claude hook configuration failed
- rerun `Configure Claude Hooks`
- inspect `~/.claude/settings.json`
- enable debug logging if needed

`The light stays yellow forever`

- check whether `Stop` is arriving
- check whether `PreToolUse(AskUserQuestion)` is configured
- inspect `lastMessageText` in the state file

`macOS build exists but the app will not open`

- unsigned local builds may be blocked by Gatekeeper
- wider distribution will need Apple signing and notarization

## macOS Verification

Current verification status:

- GitHub Actions `macOS Verify` passes on `macos-latest`
- the workflow runs `npm test`
- the workflow runs `npm run build`
- the workflow runs `npm run tauri:build:mac -- --no-sign`
- the workflow uploads unsigned `.app` and `.dmg` artifacts

Still recommended on a real Mac:

- tray behavior
- transparent always-on-top window behavior
- drag behavior
- local sound playback
- real Claude hook flow end-to-end

## Project Structure

- [bridge](bridge/README.md)
- [src](src)
- [src-tauri](src-tauri)
