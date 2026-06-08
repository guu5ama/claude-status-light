# Claude Hook Bridge

This bridge reads Claude Code hook payloads from stdin, classifies them into app states, and writes `public/state/state.json` atomically.

The desktop app can configure these hooks automatically on startup, but the schema below is the exact shape it writes.

## Claude Code Settings

Add the same command hook to `UserPromptSubmit`, `Notification`, `PreToolUse` for `AskUserQuestion`, and `Stop` in `~/.claude/settings.json`:

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

macOS development path example:

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

Why `PreToolUse(AskUserQuestion)` matters:

- plain replies normally end on `Stop`
- plan-mode follow-up questions can pause on the `AskUserQuestion` tool before `Stop`
- the bridge treats that tool path as `pending_user` so the light turns red immediately instead of staying yellow

## State Path

The bridge and the Tauri app both read `CLAUDE_STATUS_LIGHT_STATE_PATH` when it is set.

- default: `C:/code/claude-status-light/public/state/state.json`
- override: set the same env var for both the hook process and the Tauri app

## Packaged App Note

The Tauri bundle includes the `bridge/` folder as an app resource so automatic setup can point Claude hooks at the packaged bridge script, not only the development checkout.

For macOS packaging, the same logic should resolve the bundled bridge resource from the native `.app` resource directory instead of the development checkout.

## Optional Debug Logging

Debug logging is disabled by default.

Enable it only when investigating hook behavior:

```powershell
$env:CLAUDE_STATUS_LIGHT_DEBUG = "1"
```

Optional custom log file:

```powershell
$env:CLAUDE_STATUS_LIGHT_DEBUG_LOG_PATH = "C:/code/claude-status-light/public/state/hook-debug.jsonl"
```

When enabled, the bridge writes JSONL entries for:

- received hooks
- ignored hooks
- ignored session merges
- state writes
- hook errors

## Manual Smoke Test

```powershell
'{"session_id":"s1","hook_event_name":"UserPromptSubmit"}' | node .\bridge\claude-hook.mjs
```

Expected result:

- `public/state/state.json` becomes `running`
- the desktop light turns yellow

## Local Development Flow

1. Run `npm install`.
2. Run `npm test`.
3. Run `npm run build`.
4. Run the manual smoke test above.
5. Run `npm run tauri:dev`.
6. Reconnect the tray session and send a real Claude Code message.
