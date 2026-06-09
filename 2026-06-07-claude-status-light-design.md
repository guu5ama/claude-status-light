# Claude Status Light Design

Date: 2026-06-07

## Goal

Build a small independent desktop app for macOS and Windows that shows Claude Code session status with a floating traffic-light style indicator:

- Yellow solid: Claude is running
- Red blinking: Claude is waiting for user input
- Green solid: task is done

The app must work when the user uses Claude Code inside VS Code, not only in terminal. Sound notifications must be globally toggleable on or off.

## Scope

MVP scope:

- Independent desktop app
- Single Claude session only
- Session registered by Claude hook on first event
- Status driven by official Claude Code hooks
- Floating always-on-top indicator window
- Tray menu with sound toggle, open/hide, reconnect, and quit
- Automatic Claude hook configuration with backup of existing settings
- Local-only operation

Out of scope for MVP:

- Reading private VS Code or Claude extension UI state
- Multiple simultaneous Claude sessions
- Cross-device sync
- Cloud backend
- Rich settings UI
- Custom sound packs

## Product Semantics

The indicator tracks task state, not merely response boundaries.

Definitions:

- Running: Claude is actively processing the current turn
- Pending user input: Claude has stopped and needs user action, user clarification, or user approval
- Done: Claude has stopped and its latest response indicates the task was completed

Important distinction:

- A Claude `Stop` event means the current response ended
- A Claude `Stop` event does not necessarily mean the overall task is complete

Because of that, `Stop` must be interpreted together with the last assistant message content.

Current implemented interpretation:

- If Claude is clearly asking for confirmation, choice, permission, or more input, show `pending_user`
- If Claude gives a direct answer without a follow-up request, show `done`
- If Claude gives explicit completion language such as "done", "implemented", or "tests passed", show `done`
- If Claude enters the `AskUserQuestion` tool path before `Stop`, show `pending_user` immediately

## Chosen Approach

Recommended architecture for MVP:

1. Claude Code hooks produce local events
2. A lightweight local bridge script normalizes those events
3. The bridge writes a local `state.json` file atomically
4. A Tauri desktop app polls that file and renders the status light

Why this approach:

- Uses official Claude Code extension points instead of private internals
- Works with Claude Code in VS Code because Claude Code settings and hooks are shared across supported IDE integrations
- Avoids local HTTP server complexity for MVP
- Easier to debug than direct transcript parsing
- Keeps responsibilities clean: Claude side emits state, app side displays state

Alternatives considered:

### Alternative A: Local HTTP bridge

Hooks send events to a localhost endpoint served by the app.

Pros:

- More real-time
- Cleaner event stream

Cons:

- Requires port management
- More app lifecycle edge cases
- Higher setup and recovery complexity for MVP

### Alternative B: Transcript/log scraping

The app directly reads Claude transcript or log files and infers status.

Pros:

- Less explicit hook setup

Cons:

- Relies on private implementation details
- Fragile across Claude Code updates
- Harder to reason about correctness

## High-Level Architecture

### 1. Claude Hook Bridge

A small local script invoked by Claude Code hooks.

Responsibilities:

- Read hook payload from Claude Code
- Extract `session_id`, event type, and latest assistant text when available
- Normalize Claude events into app-specific status events
- Write `state.json` atomically

The bridge is the source of truth for session registration and status updates.

### 2. State Evaluator

Logic layer shared or embedded in the bridge.

Responsibilities:

- Maintain the current tracked `sessionId`
- Ignore events from other sessions after first registration
- Convert Claude hook events into one of:
  - `running`
  - `pending_user`
  - `done`
- Provide a `doneReason` string for debugging

### 3. Desktop Indicator App

Independent Tauri app.

Responsibilities:

- Poll `state.json`
- Render the floating indicator
- Blink red when in pending state
- Play sound on state transitions when enabled
- Expose tray actions

Current implementation note:

- The frontend reads state through the app and updates the indicator live from the shared state file
- The window supports dragging, tray icon controls, and proportional content scaling for different monitor DPI settings
- The app also queries Claude hook setup status from the Tauri backend and can surface `SETUP NEEDED` when automatic configuration fails

### 4. Audio Manager

Responsibilities:

- Play one sound on meaningful state change
- Respect global mute toggle
- Debounce repeated events that do not change status

## Status Model

Canonical states:

- `idle_unbound`
- `running`
- `pending_user`
- `done`
- `bridge_disconnected`

Display mapping:

- `idle_unbound` -> gray solid
- `running` -> yellow solid
- `pending_user` -> red blinking
- `done` -> green solid
- `bridge_disconnected` -> gray solid with error metadata in tray

State transitions:

- `UserPromptSubmit` -> `running`
- `Notification(permission_prompt)` -> `pending_user`
- `Notification(idle_prompt)` -> `pending_user`
- `Notification(elicitation_dialog)` -> `pending_user`
- `PreToolUse(AskUserQuestion)` -> `pending_user`
- `Stop` + completion signal -> `done`
- `Stop` + direct answer with no waiting signal -> `done`

Rules:

- Only play sound on state change
- Do not replay sound if repeated events keep the same state
- If latest assistant text cannot be extracted on `Stop`, fall back to `pending_user`

## Done Detection

MVP done detection is rule-based, not AI-based.

Positive completion signals include phrases such as:

- "done"
- "completed"
- "finished"
- "fixed"
- "implemented"
- "tests passed"
- localized equivalents that clearly indicate completion

Negative or waiting signals include phrases such as:

- questions ending the reply
- permission prompts
- "please confirm"
- "do you want me to continue"
- "which option do you want"
- "need more information"
- localized equivalents that clearly request confirmation, choice, or more input

MVP decision rule:

- If a reply contains a strong waiting signal, classify as `pending_user`
- Else if a reply contains a strong completion signal, classify as `done`
- Else default to `done`

Implemented rationale:

- The earlier conservative default-to-red rule misclassified normal factual answers as pending
- The current rule treats direct answers as green unless the assistant is explicitly waiting on the user

## Session Binding

MVP session behavior:

- The first valid event after startup registers the active `sessionId`
- The app tracks only that one session
- Events from other sessions are ignored
- If the user wants to monitor a different session, they use a tray action to clear and rebind

This satisfies the "single session, hook registers current session" requirement without needing a session picker UI.

## Automatic Claude Hook Configuration

Approved behavior:

- Attempt configuration automatically on startup
- Keep a tray action to rerun configuration manually
- Back up the original `~/.claude/settings.json` before any actual write
- Merge hook entries instead of overwriting existing settings
- Preserve unrelated hooks
- Avoid duplicate insertion of the same bridge command
- Treat the current running app location as the source of truth for the valid bridge path
- Automatically remove stale Claude Status Light bridge paths that point at old repos or old portable folders

Current implementation shape:

- Tauri backend resolves the Claude settings path from the user home directory
- Tauri backend resolves the bridge script path from either bundled resources or the development checkout
- Hook merge is done through a pure Rust JSON merger
- Existing valid `claude-hook.mjs` entries are detected first and treated as already configured before any rewrite is attempted
- Automatic configuration now treats `UserPromptSubmit`, `Notification`, `PreToolUse(AskUserQuestion)`, and `Stop` as the required bridge hook set
- UTF-8 BOM in `settings.json` is stripped before JSON parsing so previously written Windows config files do not trigger false setup failures
- The running app location is now treated as the only valid Claude Status Light hook path
- Stale Claude Status Light bridge paths that point at old repos or old portable folders are removed and rewritten to the current app location
- Unrelated non-Claude-Status-Light hook commands in the same matcher group are preserved during rewrites
- If automatic configuration fails, the frontend can show `SETUP NEEDED`
- If automatic configuration writes changes, the frontend can show `SETUP OK` with the backup path for that successful rewrite
- Portable startup should no longer rely on the user manually re-pointing hooks after moving the extracted folder

Current self-healing behavior:

- On every startup, the app resolves the current bridge path from the running app location
- For each required hook event, it keeps exactly one Claude Status Light command entry that points to the current bridge path
- Other Claude Status Light command entries that point to stale locations are removed
- If the current path is already the only Claude Status Light path, the app reports `ALREADY CONFIGURED` and does not rewrite
- If startup rewrites hooks, the app reports `HOOKS UPDATED` and can surface both the active bridge path and backup path in the UI
- Reopening Claude Code is still required after an actual rewrite so the IDE reloads `settings.json`

Current limitation:

- Automatic configuration currently targets the standard `~/.claude/settings.json` location
- Restarting or reopening Claude Code is still required after a real settings write
- macOS packaging and runtime behavior still need native validation even though the path resolution logic is already designed to use the user home directory and bundled bridge resources

## Data Contract

The bridge writes a local `state.json` with a minimal schema:

```json
{
  "sessionId": "abc123",
  "status": "running",
  "updatedAt": "2026-06-07T10:30:00.000Z",
  "soundEnabled": true,
  "lastEvent": "UserPromptSubmit",
  "lastMessageText": "Updated button color to theme blue and tests passed.",
  "doneReason": "assistant_signaled_completion",
  "bridgeHealthy": true
}
```

Field notes:

- `sessionId`: currently tracked Claude session
- `status`: one of the canonical statuses
- `updatedAt`: ISO timestamp of last accepted event
- `soundEnabled`: persisted global mute setting
- `lastEvent`: raw Claude hook event name
- `lastMessageText`: latest assistant text used for classification
- `doneReason`: debug explanation for why `done` or `pending_user` was chosen
- `bridgeHealthy`: whether the bridge wrote valid state recently

## File Locations

Project root:

- `c:\code\claude-status-light`

Expected project layout:

- `src-tauri/` for native shell
- `src/` for UI
- `bridge/` for Claude hook scripts

Runtime state file:

- Current implementation uses repo-local `public/state/state.json`
- `CLAUDE_STATUS_LIGHT_STATE_PATH` can override this for both bridge and app
- Development builds keep using the repo-local state file
- Release and portable builds now default to a user-local state path instead of a compile-time repo path
- Portable relocations should not require state path changes by the user; only hook path healing is needed

Claude settings file:

- Windows: `C:\Users\<user>\.claude\settings.json`
- macOS: `/Users/<user>/.claude/settings.json`

## Desktop UI

Floating window behavior:

- Tall narrow window with a transparent background
- Traffic-light proportion rather than square proportion
- Default body sized like a small floating desktop signal
- Borderless
- Always on top
- Draggable
- Minimal chrome
- Right-click context menu
- Keep the window narrow even when adding controls or setup feedback; prefer vertical stacking over extra width

Visual behavior:

- Three circular lights stacked vertically
- Red light at top
- Yellow light in middle
- Green light at bottom
- Only the active state light is illuminated strongly at any one time
- Inactive lights remain visible as dim lenses
- Yellow light is steady
- Red light blinks
- Green light is steady
- Gray is used before session bind and on bridge disconnection

Implemented UI notes:

- Current UI is a vertical three-lens traffic-light body with dim inactive lenses
- The window uses transparent chrome around the signal body
- The content scales proportionally to avoid scrollbars or clipping on different displays
- The old bottom support post has been removed from the signal body
- The old top cap bar has also been removed, leaving a cleaner single-body silhouette
- The bottom controls row holds two icon buttons: a chevron `Show/Hide Details` toggle and the sound mute toggle, so the UI keeps a narrow footprint
- A setup note can appear below the status label to show automatic configuration success or failure details
- A usage panel can appear below the status/setup area showing Claude plan usage (see `Claude Usage Display`)
- Because the window is transparent, the status label and usage text use a dark outline so they stay legible on any desktop wallpaper

## Claude Usage Display

Post-MVP addition. The app shows the same plan-usage information that Claude Code's `/usage` view reports: a 5-hour session window and a 7-day weekly window, each as a circular dial with a relative reset time.

Data source:

- Official Anthropic OAuth endpoint `GET https://api.anthropic.com/api/oauth/usage`
- Requires `Authorization: Bearer <accessToken>` and `anthropic-beta: oauth-2025-04-20`
- The access token is read from `~/.claude/.credentials.json` (`claudeAiOauth.accessToken`), the same credentials Claude Code itself uses
- Response fields consumed: `five_hour.{utilization,resets_at}` and `seven_day.{utilization,resets_at}`; other windows in the response are ignored

Why this differs from the rejected "cloud backend" non-goal:

- The app does not run or depend on its own server; it makes a read-only call to the user's existing Claude account endpoint from the local machine
- No state is sent anywhere; only usage percentages are read back for display

Architecture:

- Fetch happens in the Tauri Rust backend via a `get_claude_usage` command, not in the bridge and not in the webview
- The bridge is event-driven by hooks and cannot poll on a timer, so it is the wrong place for periodic usage reads
- Keeping the call in Rust avoids exposing the OAuth token to the webview and avoids webview CORS restrictions against `api.anthropic.com`
- The Rust client uses `reqwest` with the `rustls-tls-native-roots` backend: rustls avoids the Windows schannel certificate-revocation failure seen under TLS inspection, while native (OS) root certificates let it trust a corporate/interception CA the same way the system tools do (the bundled webpki roots reject such intercepted certificates, which made requests fail)

Polling and resilience:

- The endpoint rate-limits aggressively, so the frontend polls only every 5 minutes, far less often than the 500ms `state.json` poll
- On any error (network failure, rate limit, expired token) the last good usage value is kept and no error is surfaced
- If no usage data has been obtained yet, the panel renders nothing rather than showing a placeholder

UI:

- Two circular dials side by side: each has a small `5H` / `7D` label on top, the percentage in the center, and a `resets in Xh` line below
- The ring arc and the center percentage are color-coded by utilization: orange-yellow below 80%, orange at 80% and above
- The dial center is transparent (no filled disc); text uses a dark outline so it stays legible against any desktop wallpaper, since the window is transparent and the text sits directly on the desktop
- The panel keeps the narrow window footprint; the window is sized to the steady-state layout

Current limitation:

- Token refresh is not implemented; when the stored access token expires the panel silently keeps the last value until valid credentials return

## macOS Packaging Direction

Preferred shipping model:

- Native Tauri `.app`
- Optional `.dmg` as the distribution wrapper

Current release workflow:

- One shared codebase
- Windows package built on Windows
- macOS package built on macOS
- Not a single universal installer artifact across both desktop OSes

Rejected runtime model:

- Docker is not suitable for this product because it needs a real floating desktop window, tray presence, local audio, and direct access to the user's Claude settings file

Current macOS assumptions:

- Claude settings path resolves through `$HOME/.claude/settings.json`
- The app should use the bundled bridge resource when running from a packaged `.app`
- Native tray/menu-bar behavior, transparent window behavior, and audio behavior still need direct macOS verification
- The project now exposes a dedicated `npm run tauri:build:mac` command to standardize the Mac-side release build
- A Windows-side source zip can be prepared and moved onto a Mac for native packaging there
- Portable Windows builds can locate `bridge/claude-hook.mjs` next to the `exe`, so a no-install zip can still auto-configure Claude hooks correctly
- The same self-healing hook-path rule applies on macOS portable or moved app bundles: current app location wins, stale Claude Status Light paths are removed

macOS validation checklist:

- `npm run tauri:dev` launches correctly on macOS
- tray/menu-bar icon is visible and interactive
- the always-on-top floating window drags correctly
- local MP3 sound assets play correctly after first user interaction
- startup auto-configuration safely writes the Claude settings file backup and merged hooks
- `npm run tauri:build` produces a working `.app`
- packaged app can still resolve the bundled `bridge/claude-hook.mjs`

### Visual Direction

Approved visual direction:

- Real traffic-light silhouette
- Vertical three-light layout
- Simplified premium hardware look
- No heavy screws, no industrial clutter, no complex control panel

Housing:

- Tall rounded-rectangle body
- Matte dark charcoal or black shell
- Subtle metallic edge highlights
- Transparent outer window background so only the signal body feels visible on the desktop

Lens treatment:

- Lenses should look clear and hard, closer to acrylic or glass than frosted plastic
- Avoid heavy matte diffusion or grain
- Keep a crisp circular edge and visible lens shape even when dim
- Active light should have a bright readable core with restrained outer glow
- Inactive lights should keep form without looking flat black

Motion:

- Red should use a slower, softer blink rather than abrupt on-off flashing
- Yellow and green should remain stable
- Any glow pulse should be subtle enough to feel like hardware, not neon UI

Text and controls:

- No permanent text on the traffic-light body itself
- Debug/status text may appear outside the body in app-only contexts, but should stay visually secondary
- Tray and context controls should stay simple and mostly system-native rather than becoming a custom floating control panel

Tray menu:

- Sound On/Off
- Open/Hide
- Show/Hide Details
- Configure Claude Hooks
- Reconnect Session
- Quit

Show/Hide Details can be triggered from the tray menu or from an in-window chevron button next to the sound button (both fire the same toggle). It hides the entire area below the traffic light (status label, setup note, and usage panel), leaving just the signal body and the two controls. When hidden, the OS window shrinks to a collapsed height so there is no dead transparent space, and grows back when shown. Two details make this stable:

- A separate collapsed design height keeps the traffic light at full scale in both modes.
- Content is top-anchored (`align-content: start`) so the traffic light stays at a fixed position and does not jump when toggling; the resize is sequenced with the render (grow window before showing, hide content before shrinking) to avoid a transient scale flash.

The toggle is in-memory and defaults to visible on each launch.

Setup messaging:

- `ALREADY CONFIGURED` when the current app path is already the only active Claude Status Light hook path
- `HOOKS UPDATED` when startup or manual configuration rewrites stale Claude Status Light hook paths to the current app path
- `HOOKS UPDATED` is transient: it auto-hides after a short delay and disappears immediately once a real Claude session status (`running`, `pending_user`, or `done`) arrives
- `SETUP NEEDED` when parsing, backup, or write fails

## Sound Behavior

MVP sound rules:

- Global on/off only
- One sound per state transition
- No sound replay if state stays the same
- Default implementation may use synthesized tones instead of bundled audio assets if that keeps the MVP simpler and more portable

Implemented sound choice:

- The current build prefers bundled local MP3 notification files and falls back to synthesized tones only if file playback is unavailable
- Sound remains globally toggleable from the tray and from the small speaker button in the main UI
- The current build primes Web Audio on first user interaction with the window or the speaker button so later red/green transitions can be heard reliably in the desktop webview

Recommended initial transitions:

- `running -> pending_user`: play pending sound
- `running -> done`: play done sound
- `pending_user -> running`: optional running sound, default off to reduce noise

## Failure Handling

### App starts before any session

Behavior:

- Show gray light
- Wait for first valid bridge write

### Bridge write fails

Behavior:

- Keep last good state
- Mark bridge unhealthy after timeout window
- Expose disconnection status in tray

### `state.json` is malformed

Behavior:

- Ignore malformed read
- Keep last known good state
- Retry on next poll

### Unknown or unsupported hook payload

Behavior:

- Ignore event
- Do not change state

### Missing assistant text on `Stop`

Behavior:

- Default to `pending_user`

## Technology Choice

Recommended stack:

- Tauri for desktop shell
- TypeScript for frontend logic
- Small bridge script in Node.js

Reasoning:

- Tauri is lightweight and cross-platform
- TypeScript keeps UI and classifier logic easy to iterate
- Bridge script can stay tiny and portable

Implemented choice:

- The bridge is implemented in Node.js
- The current desktop shell is Tauri + Rust with a React frontend

## Testing Strategy

Required tests for MVP:

- Status classification unit tests
- Session binding unit tests
- File parsing tests for malformed or partial `state.json`
- UI smoke test for each visual state
- Manual sound toggle verification

Implemented verification coverage:

- Bridge classification tests
- Bridge session-binding tests
- Bridge hook execution tests
- Rust tests for Claude settings merge behavior
- Frontend component tests for light state rendering
- TypeScript + Vite build verification

Manual verification scenarios:

1. Start app before session -> gray
2. Send Claude prompt -> yellow
3. Claude asks for clarification -> red blinking
4. Claude completes a concrete task -> green
5. Turn sound off -> state still changes, no sound
6. Start second session -> ignored until rebind

## Implementation Notes

Initial implementation should optimize for correctness and debuggability over polish.

Priority order:

1. Correct hook-to-state pipeline
2. Correct done detection for both task completion and direct answers
3. Stable floating window and tray behavior
4. Sound toggle persistence
5. Visual polish

## Open Questions Resolved

- App type: independent desktop app
- Session count: single session only
- Session binding: hook-driven registration
- Pending state color: red blinking
- Running state color: yellow steady
- Done state color: green steady
- Sound: global on/off toggle
- VS Code support: yes, via Claude Code hooks rather than private extension internals

## Implementation Snapshot

Implemented as of this design sync:

- Hook bridge in `bridge/claude-hook.mjs`
- Event classification in `bridge/classify-event.mjs`
- Session binding logic in `bridge/read-current-state.mjs`
- Automatic Claude settings merge + backup logic in `src-tauri/src/claude_settings.rs`
- Floating traffic-light UI in `src/components/StatusLight.tsx`
- Claude plan-usage dials in `src/components/UsagePanel.tsx`, `src/hooks/useClaudeUsage.ts`, `src/lib/usage.ts`, and the `get_claude_usage` Tauri command in `src-tauri/src/lib.rs` (reqwest with `rustls-tls-native-roots`)
- Show/Hide Details toggle (tray item + in-window chevron) with sequenced window resize in `src/App.tsx`, the `toggle_details` tray emit in `src-tauri/src/lib.rs`, and the `core:window:allow-set-size` capability
- State polling and sound control in `src/App.tsx` and `src/hooks/`
- Tauri tray and window shell in `src-tauri/`
- Root usage guide in `README.md`, now rewritten in a product-first GitHub style instead of a handoff-log style
- README positioning is now explicit that the product is primarily for the Claude Code plugin inside VS Code; other Claude Code surfaces may work via shared hooks, but they are not the primary documented target
- `AskUserQuestion` pending-user detection is handled before `Stop`, so plan-mode user questions no longer leave the light stuck on yellow
- Release scripts now include `tauri:build:windows` and `tauri:build:mac`
- A Windows release build has been generated successfully as `.exe` and `.msi`
- Windows release entrypoint now uses the GUI subsystem, so installed release builds should no longer open an extra console window
- Release path handling is now split cleanly: dev uses repo-local state, while packaged and portable releases use a user-local state path and can discover the bridge beside the `exe`
- Startup hook configuration is now self-healing for portable and moved builds
- Old Claude Status Light bridge paths in `settings.json` are removed automatically, including repo paths with `src-tauri/../bridge`, Windows verbatim `//?/` forms, and renamed portable folders such as `Claude Status Light_1`
- The running app location is now the single source of truth for the active hook path
- Windows verbatim path prefixes such as `\\?\` or `//?/` are stripped before writing the Node hook command, because `node "//?/C:/.../claude-hook.mjs"` does not execute correctly on Windows

Known current limitation:

- Windows behavior has been validated directly
- macOS now has a GitHub Actions verification path that runs frontend tests, production build, and unsigned `.app/.dmg` packaging on `macos-latest`
- macOS still needs direct native validation for tray behavior, transparent window behavior, drag behavior, and local audio before being treated as confirmed-shipping

## Next Step

After user reviews and approves this design document, write a concrete implementation plan before coding.
