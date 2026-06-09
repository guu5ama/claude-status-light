# Claude Status Light Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a small Tauri desktop app plus local Claude hook bridge that shows single-session Claude Code status as yellow solid for running, red blinking for pending user input, green solid for done, with a global sound on/off toggle.

**Architecture:** A tiny bridge script receives official Claude Code hook payloads, classifies them into a conservative task state, and writes an atomic local `state.json`. A Tauri app polls that file, renders a floating always-on-top indicator window, exposes a tray menu, and plays sounds only when state changes.

**Tech Stack:** Tauri, React, TypeScript, Vite, Vitest, Testing Library, Node.js bridge script

---

## File Structure

Create and use these files only:

- `claude-status-light/package.json` - project scripts and dependencies
- `claude-status-light/tsconfig.json` - TypeScript configuration
- `claude-status-light/tsconfig.node.json` - TypeScript node config
- `claude-status-light/vite.config.ts` - Vite configuration
- `claude-status-light/index.html` - app shell
- `claude-status-light/src/main.tsx` - React bootstrap
- `claude-status-light/src/App.tsx` - root component
- `claude-status-light/src/styles.css` - all indicator and window styling
- `claude-status-light/src/lib/types.ts` - shared app-side types for state data
- `claude-status-light/src/lib/default-state.ts` - default state and helpers
- `claude-status-light/src/lib/read-state.ts` - poller and parser for `state.json`
- `claude-status-light/src/lib/sound.ts` - browser-side sound playback helpers
- `claude-status-light/src/components/StatusLight.tsx` - light renderer
- `claude-status-light/src/components/StatusLabel.tsx` - optional small debug label
- `claude-status-light/src/hooks/useStatusState.ts` - polling hook for app state
- `claude-status-light/src/hooks/useSoundToggle.ts` - sound toggle persistence and command wiring
- `claude-status-light/src-tauri/Cargo.toml` - native dependencies
- `claude-status-light/src-tauri/tauri.conf.json` - Tauri window and tray config
- `claude-status-light/src-tauri/src/main.rs` - Tauri entrypoint
- `claude-status-light/src-tauri/src/lib.rs` - tray commands and state-file path command
- `claude-status-light/bridge/package.json` - bridge script package metadata
- `claude-status-light/bridge/write-state.mjs` - atomic state writer
- `claude-status-light/bridge/classify-event.mjs` - hook event classification
- `claude-status-light/bridge/claude-hook.mjs` - Claude hook entrypoint
- `claude-status-light/bridge/README.md` - hook setup instructions
- `claude-status-light/assets/sounds/pending.mp3` - pending sound asset
- `claude-status-light/assets/sounds/done.mp3` - done sound asset
- `claude-status-light/public/state/state.json` - dev-only local sample state file
- `claude-status-light/src/lib/__tests__/default-state.test.ts` - default state tests
- `claude-status-light/src/lib/__tests__/read-state.test.ts` - parser tests
- `claude-status-light/src/lib/__tests__/sound.test.ts` - sound helper tests
- `claude-status-light/src/components/__tests__/StatusLight.test.tsx` - visual-state render tests
- `claude-status-light/src/hooks/__tests__/useStatusState.test.tsx` - polling behavior tests
- `claude-status-light/bridge/__tests__/classify-event.test.mjs` - event classifier tests
- `claude-status-light/bridge/__tests__/write-state.test.mjs` - atomic write tests

Post-MVP additions (Tasks 10-11) also use these files:

- `claude-status-light/src/lib/usage.ts` - usage types, parsing, percent/reset formatting
- `claude-status-light/src/lib/__tests__/usage.test.ts` - usage parsing and formatting tests
- `claude-status-light/src/hooks/useClaudeUsage.ts` - low-frequency usage polling hook
- `claude-status-light/src/components/UsagePanel.tsx` - the two usage dials
- `claude-status-light/src/lib/design.ts` - window/design size constants
- `claude-status-light/src-tauri/capabilities/default.json` - window permissions

Assumptions locked in before implementation:

- Keep all work inside `claude-status-light`
- Use one tracked session only
- Use Node.js for bridge to avoid second runtime
- Poll state file rather than running a localhost server
- Default unknown `Stop` results to `pending_user`

### Task 1: Scaffold the Tauri + React project

**Files:**
- Create: `claude-status-light/package.json`
- Create: `claude-status-light/tsconfig.json`
- Create: `claude-status-light/tsconfig.node.json`
- Create: `claude-status-light/vite.config.ts`
- Create: `claude-status-light/index.html`
- Create: `claude-status-light/src/main.tsx`
- Create: `claude-status-light/src/App.tsx`
- Create: `claude-status-light/src/styles.css`
- Create: `claude-status-light/src-tauri/Cargo.toml`
- Create: `claude-status-light/src-tauri/tauri.conf.json`
- Create: `claude-status-light/src-tauri/src/main.rs`
- Create: `claude-status-light/src-tauri/src/lib.rs`

- [ ] **Step 1: Create the base package manifest**

```json
{
  "name": "claude-status-light",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "test": "vitest run",
    "test:watch": "vitest",
    "tauri:dev": "tauri dev",
    "tauri:build": "tauri build"
  },
  "dependencies": {
    "@tauri-apps/api": "^2.0.0",
    "react": "^18.3.1",
    "react-dom": "^18.3.1"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2.0.0",
    "@testing-library/jest-dom": "^6.6.3",
    "@testing-library/react": "^16.1.0",
    "@testing-library/user-event": "^14.5.2",
    "@types/react": "^18.3.12",
    "@types/react-dom": "^18.3.1",
    "@vitejs/plugin-react": "^4.3.3",
    "jsdom": "^25.0.1",
    "typescript": "^5.6.3",
    "vite": "^5.4.10",
    "vitest": "^2.1.4"
  }
}
```

- [ ] **Step 2: Install dependencies**

Run: `npm install`
Expected: install completes without missing-package errors

- [ ] **Step 3: Create TypeScript and Vite config**

```json
// tsconfig.json
{
  "compilerOptions": {
    "target": "ES2020",
    "useDefineForClassFields": true,
    "lib": ["DOM", "DOM.Iterable", "ES2020"],
    "allowJs": false,
    "skipLibCheck": true,
    "esModuleInterop": true,
    "allowSyntheticDefaultImports": true,
    "strict": true,
    "forceConsistentCasingInFileNames": true,
    "module": "ESNext",
    "moduleResolution": "Node",
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "react-jsx"
  },
  "include": ["src"],
  "references": [{ "path": "./tsconfig.node.json" }]
}
```

```json
// tsconfig.node.json
{
  "compilerOptions": {
    "composite": true,
    "module": "ESNext",
    "moduleResolution": "Node",
    "allowSyntheticDefaultImports": true
  },
  "include": ["vite.config.ts"]
}
```

```ts
// vite.config.ts
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'jsdom',
    setupFiles: './src/test-setup.ts'
  }
});
```

- [ ] **Step 4: Create the minimal app shell**

```html
<!-- index.html -->
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Claude Status Light</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

```tsx
// src/main.tsx
import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import './styles.css';

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
```

```tsx
// src/App.tsx
export default function App() {
  return <div>Claude Status Light</div>;
}
```

```css
/* src/styles.css */
html,
body,
#root {
  margin: 0;
  width: 100%;
  height: 100%;
}

body {
  background: transparent;
  font-family: "SF Pro Display", "Segoe UI", sans-serif;
}
```

- [ ] **Step 5: Create the minimal Tauri shell**

```toml
# src-tauri/Cargo.toml
[package]
name = "claude-status-light"
version = "0.1.0"
edition = "2021"

[lib]
name = "claude_status_light_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tauri = { version = "2", features = ["tray-icon"] }
tauri-plugin-opener = "2"
```

```json
// src-tauri/tauri.conf.json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Claude Status Light",
  "version": "0.1.0",
  "identifier": "com.local.claude-status-light",
  "build": {
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build",
    "frontendDist": "../dist",
    "devUrl": "http://localhost:5173"
  },
  "app": {
    "windows": [
      {
        "label": "main",
        "title": "Claude Status Light",
        "width": 84,
        "height": 84,
        "resizable": false,
        "decorations": false,
        "transparent": true,
        "alwaysOnTop": true,
        "visible": true,
        "center": true
      }
    ]
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": []
  }
}
```

```rust
// src-tauri/src/main.rs
fn main() {
    claude_status_light_lib::run();
}
```

```rust
// src-tauri/src/lib.rs
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 6: Run the frontend test and build smoke checks**

Run: `npm test`
Expected: no tests found yet, command exits successfully or with known empty-suite message

Run: `npm run build`
Expected: Vite build succeeds

- [ ] **Step 7: Commit scaffold**

```bash
git add package.json tsconfig.json tsconfig.node.json vite.config.ts index.html src src-tauri
git commit -m "chore: scaffold tauri status app"
```

### Task 2: Define the state model and parser contract

**Files:**
- Create: `claude-status-light/src/lib/types.ts`
- Create: `claude-status-light/src/lib/default-state.ts`
- Create: `claude-status-light/src/lib/read-state.ts`
- Create: `claude-status-light/src/lib/__tests__/default-state.test.ts`
- Create: `claude-status-light/src/lib/__tests__/read-state.test.ts`
- Create: `claude-status-light/src/test-setup.ts`
- Create: `claude-status-light/public/state/state.json`

- [ ] **Step 1: Write the failing tests for default state and parser fallback**

```ts
// src/lib/__tests__/default-state.test.ts
import { describe, expect, it } from 'vitest';
import { createDefaultState } from '../default-state';

describe('createDefaultState', () => {
  it('returns gray idle state before any session is bound', () => {
    expect(createDefaultState()).toEqual({
      sessionId: null,
      status: 'idle_unbound',
      updatedAt: '',
      soundEnabled: true,
      lastEvent: null,
      lastMessageText: '',
      doneReason: 'not_bound',
      bridgeHealthy: false
    });
  });
});
```

```ts
// src/lib/__tests__/read-state.test.ts
import { describe, expect, it } from 'vitest';
import { parseStateFile } from '../read-state';

describe('parseStateFile', () => {
  it('accepts a valid state payload', () => {
    const parsed = parseStateFile(JSON.stringify({
      sessionId: 'session-1',
      status: 'running',
      updatedAt: '2026-06-07T10:30:00.000Z',
      soundEnabled: true,
      lastEvent: 'UserPromptSubmit',
      lastMessageText: '',
      doneReason: 'user_prompt_submit',
      bridgeHealthy: true
    }));

    expect(parsed.status).toBe('running');
    expect(parsed.sessionId).toBe('session-1');
  });

  it('falls back to previous state for malformed JSON', () => {
    const previous = {
      sessionId: null,
      status: 'idle_unbound',
      updatedAt: '',
      soundEnabled: true,
      lastEvent: null,
      lastMessageText: '',
      doneReason: 'not_bound',
      bridgeHealthy: false
    };

    expect(parseStateFile('{bad json', previous)).toEqual(previous);
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm test -- src/lib/__tests__/default-state.test.ts src/lib/__tests__/read-state.test.ts`
Expected: FAIL with module not found errors for `default-state` and `read-state`

- [ ] **Step 3: Create the test setup and shared types**

```ts
// src/test-setup.ts
import '@testing-library/jest-dom';
```

```ts
// src/lib/types.ts
export type StatusKind =
  | 'idle_unbound'
  | 'running'
  | 'pending_user'
  | 'done'
  | 'bridge_disconnected';

export interface StatusState {
  sessionId: string | null;
  status: StatusKind;
  updatedAt: string;
  soundEnabled: boolean;
  lastEvent: string | null;
  lastMessageText: string;
  doneReason: string;
  bridgeHealthy: boolean;
}
```

- [ ] **Step 4: Implement default state and parser**

```ts
// src/lib/default-state.ts
import type { StatusState } from './types';

export function createDefaultState(): StatusState {
  return {
    sessionId: null,
    status: 'idle_unbound',
    updatedAt: '',
    soundEnabled: true,
    lastEvent: null,
    lastMessageText: '',
    doneReason: 'not_bound',
    bridgeHealthy: false
  };
}
```

```ts
// src/lib/read-state.ts
import { createDefaultState } from './default-state';
import type { StatusState, StatusKind } from './types';

const allowed: StatusKind[] = [
  'idle_unbound',
  'running',
  'pending_user',
  'done',
  'bridge_disconnected'
];

function isStatusKind(value: unknown): value is StatusKind {
  return typeof value === 'string' && allowed.includes(value as StatusKind);
}

export function parseStateFile(
  raw: string,
  fallback: StatusState = createDefaultState()
): StatusState {
  try {
    const parsed = JSON.parse(raw) as Partial<StatusState>;
    if (!isStatusKind(parsed.status)) {
      return fallback;
    }

    return {
      sessionId: typeof parsed.sessionId === 'string' ? parsed.sessionId : null,
      status: parsed.status,
      updatedAt: typeof parsed.updatedAt === 'string' ? parsed.updatedAt : '',
      soundEnabled: typeof parsed.soundEnabled === 'boolean' ? parsed.soundEnabled : true,
      lastEvent: typeof parsed.lastEvent === 'string' ? parsed.lastEvent : null,
      lastMessageText: typeof parsed.lastMessageText === 'string' ? parsed.lastMessageText : '',
      doneReason: typeof parsed.doneReason === 'string' ? parsed.doneReason : 'unknown',
      bridgeHealthy: typeof parsed.bridgeHealthy === 'boolean' ? parsed.bridgeHealthy : false
    };
  } catch {
    return fallback;
  }
}
```

```json
// public/state/state.json
{
  "sessionId": null,
  "status": "idle_unbound",
  "updatedAt": "",
  "soundEnabled": true,
  "lastEvent": null,
  "lastMessageText": "",
  "doneReason": "not_bound",
  "bridgeHealthy": false
}
```

- [ ] **Step 5: Re-run parser tests**

Run: `npm test -- src/lib/__tests__/default-state.test.ts src/lib/__tests__/read-state.test.ts`
Expected: PASS

- [ ] **Step 6: Commit state-model layer**

```bash
git add src/lib src/test-setup.ts public/state/state.json
git commit -m "feat: add status state model"
```

### Task 3: Build the bridge classifier and atomic writer

**Files:**
- Create: `claude-status-light/bridge/package.json`
- Create: `claude-status-light/bridge/classify-event.mjs`
- Create: `claude-status-light/bridge/write-state.mjs`
- Create: `claude-status-light/bridge/claude-hook.mjs`
- Create: `claude-status-light/bridge/README.md`
- Create: `claude-status-light/bridge/__tests__/classify-event.test.mjs`
- Create: `claude-status-light/bridge/__tests__/write-state.test.mjs`

- [ ] **Step 1: Write the failing classifier tests**

```js
// bridge/__tests__/classify-event.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { classifyHookEvent } from '../classify-event.mjs';

test('maps UserPromptSubmit to running', () => {
  const result = classifyHookEvent({
    session_id: 's1',
    hook_event_name: 'UserPromptSubmit'
  });

  assert.equal(result.status, 'running');
  assert.equal(result.doneReason, 'user_prompt_submit');
});

test('maps Stop with completion text to done', () => {
  const result = classifyHookEvent({
    session_id: 's1',
    hook_event_name: 'Stop',
    transcript_path: '',
    last_assistant_message: 'Implemented the fix and tests passed.'
  });

  assert.equal(result.status, 'done');
  assert.equal(result.doneReason, 'assistant_signaled_completion');
});

test('maps Stop without completion text to pending_user', () => {
  const result = classifyHookEvent({
    session_id: 's1',
    hook_event_name: 'Stop',
    last_assistant_message: 'Which option do you want me to apply?'
  });

  assert.equal(result.status, 'pending_user');
  assert.equal(result.doneReason, 'assistant_waiting_for_input');
});
```

```js
// bridge/__tests__/write-state.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { writeStateAtomically } from '../write-state.mjs';

test('writes a complete state file', async () => {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), 'claude-status-light-'));
  const filePath = path.join(dir, 'state.json');

  await writeStateAtomically(filePath, {
    sessionId: 's1',
    status: 'running',
    updatedAt: '2026-06-07T10:30:00.000Z',
    soundEnabled: true,
    lastEvent: 'UserPromptSubmit',
    lastMessageText: '',
    doneReason: 'user_prompt_submit',
    bridgeHealthy: true
  });

  const raw = await fs.readFile(filePath, 'utf8');
  const parsed = JSON.parse(raw);
  assert.equal(parsed.status, 'running');
  assert.equal(parsed.sessionId, 's1');
});
```

- [ ] **Step 2: Run the bridge tests to verify they fail**

Run: `node --test bridge/__tests__/classify-event.test.mjs bridge/__tests__/write-state.test.mjs`
Expected: FAIL with module not found errors

- [ ] **Step 3: Create the bridge package manifest**

```json
// bridge/package.json
{
  "name": "claude-status-light-bridge",
  "private": true,
  "type": "module",
  "scripts": {
    "test": "node --test __tests__/*.test.mjs"
  }
}
```

- [ ] **Step 4: Implement the classifier**

```js
// bridge/classify-event.mjs
const waitingPhrases = [
  'please confirm',
  'do you want me to continue',
  'which option do you want',
  'need more information',
  'permission'
];

const completionPhrases = [
  'done',
  'completed',
  'finished',
  'fixed',
  'implemented',
  'tests passed'
];

function normalizeText(value) {
  return typeof value === 'string' ? value.trim().toLowerCase() : '';
}

function inferStopState(message) {
  const text = normalizeText(message);
  if (!text) {
    return { status: 'pending_user', doneReason: 'missing_assistant_text' };
  }

  if (waitingPhrases.some((phrase) => text.includes(phrase)) || text.endsWith('?')) {
    return { status: 'pending_user', doneReason: 'assistant_waiting_for_input' };
  }

  if (completionPhrases.some((phrase) => text.includes(phrase))) {
    return { status: 'done', doneReason: 'assistant_signaled_completion' };
  }

  return { status: 'pending_user', doneReason: 'assistant_waiting_for_input' };
}

export function classifyHookEvent(payload) {
  const eventName = payload.hook_event_name;
  const sessionId = payload.session_id ?? null;
  const lastMessageText = payload.last_assistant_message ?? '';

  if (eventName === 'UserPromptSubmit') {
    return {
      sessionId,
      status: 'running',
      updatedAt: new Date().toISOString(),
      soundEnabled: true,
      lastEvent: eventName,
      lastMessageText,
      doneReason: 'user_prompt_submit',
      bridgeHealthy: true
    };
  }

  if (
    eventName === 'Notification' &&
    ['permission_prompt', 'idle_prompt', 'elicitation_dialog'].includes(payload.notification_type)
  ) {
    return {
      sessionId,
      status: 'pending_user',
      updatedAt: new Date().toISOString(),
      soundEnabled: true,
      lastEvent: eventName,
      lastMessageText,
      doneReason: 'notification_pending_user',
      bridgeHealthy: true
    };
  }

  if (eventName === 'Stop') {
    const inferred = inferStopState(lastMessageText);
    return {
      sessionId,
      status: inferred.status,
      updatedAt: new Date().toISOString(),
      soundEnabled: true,
      lastEvent: eventName,
      lastMessageText,
      doneReason: inferred.doneReason,
      bridgeHealthy: true
    };
  }

  return null;
}
```

- [ ] **Step 5: Implement atomic state writes and hook entrypoint**

```js
// bridge/write-state.mjs
import fs from 'node:fs/promises';
import path from 'node:path';

export async function writeStateAtomically(filePath, state) {
  const dir = path.dirname(filePath);
  const tmpPath = `${filePath}.tmp`;
  await fs.mkdir(dir, { recursive: true });
  await fs.writeFile(tmpPath, `${JSON.stringify(state, null, 2)}\n`, 'utf8');
  await fs.rename(tmpPath, filePath);
}
```

```js
// bridge/claude-hook.mjs
import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { classifyHookEvent } from './classify-event.mjs';
import { writeStateAtomically } from './write-state.mjs';

const bridgeDir = path.dirname(fileURLToPath(import.meta.url));
const statePath = path.resolve(bridgeDir, '../public/state/state.json');

async function main() {
  const raw = await fs.readFile(0, 'utf8');
  const payload = JSON.parse(raw);
  const nextState = classifyHookEvent(payload);
  if (!nextState) {
    return;
  }

  await writeStateAtomically(statePath, nextState);
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
```

```md
<!-- bridge/README.md -->
# Claude Hook Setup

Add the hook command to Claude Code settings so `UserPromptSubmit`, `Notification`, and `Stop` execute:

```json
{
  "hooks": {
    "UserPromptSubmit": [
      {
        "command": "node C:/code/claude-status-light/bridge/claude-hook.mjs"
      }
    ],
    "Notification": [
      {
        "command": "node C:/code/claude-status-light/bridge/claude-hook.mjs"
      }
    ],
    "Stop": [
      {
        "command": "node C:/code/claude-status-light/bridge/claude-hook.mjs"
      }
    ]
  }
}
```
```

- [ ] **Step 6: Re-run the bridge tests**

Run: `node --test bridge/__tests__/classify-event.test.mjs bridge/__tests__/write-state.test.mjs`
Expected: PASS

- [ ] **Step 7: Commit bridge**

```bash
git add bridge
git commit -m "feat: add claude hook bridge"
```

### Task 4: Add session binding and conservative state merging

**Files:**
- Modify: `claude-status-light/bridge/classify-event.mjs`
- Modify: `claude-status-light/bridge/claude-hook.mjs`
- Create: `claude-status-light/bridge/read-current-state.mjs`
- Create: `claude-status-light/bridge/__tests__/session-binding.test.mjs`

- [ ] **Step 1: Write the failing session-binding tests**

```js
// bridge/__tests__/session-binding.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { mergeSessionState } from '../read-current-state.mjs';

test('binds first session when none exists', () => {
  const next = mergeSessionState(null, {
    sessionId: 's1',
    status: 'running'
  });

  assert.equal(next.sessionId, 's1');
  assert.equal(next.status, 'running');
});

test('ignores events from other sessions after binding', () => {
  const current = {
    sessionId: 's1',
    status: 'pending_user'
  };

  const next = mergeSessionState(current, {
    sessionId: 's2',
    status: 'done'
  });

  assert.equal(next.sessionId, 's1');
  assert.equal(next.status, 'pending_user');
});
```

- [ ] **Step 2: Run the session-binding test to verify it fails**

Run: `node --test bridge/__tests__/session-binding.test.mjs`
Expected: FAIL with module not found error for `read-current-state.mjs`

- [ ] **Step 3: Implement session merge helpers**

```js
// bridge/read-current-state.mjs
export function mergeSessionState(current, next) {
  if (!current || !current.sessionId) {
    return next;
  }

  if (!next.sessionId || next.sessionId !== current.sessionId) {
    return current;
  }

  return {
    ...current,
    ...next
  };
}
```

- [ ] **Step 4: Wire state merging into the hook entrypoint**

```js
// excerpt for bridge/claude-hook.mjs
import { mergeSessionState } from './read-current-state.mjs';

async function readCurrentState(filePath) {
  try {
    const raw = await fs.readFile(filePath, 'utf8');
    return JSON.parse(raw);
  } catch {
    return null;
  }
}

async function main() {
  const raw = await fs.readFile(0, 'utf8');
  const payload = JSON.parse(raw);
  const nextState = classifyHookEvent(payload);
  if (!nextState) {
    return;
  }

  const currentState = await readCurrentState(statePath);
  const mergedState = mergeSessionState(currentState, nextState);
  await writeStateAtomically(statePath, mergedState);
}
```

- [ ] **Step 5: Re-run all bridge tests**

Run: `node --test bridge/__tests__/classify-event.test.mjs bridge/__tests__/write-state.test.mjs bridge/__tests__/session-binding.test.mjs`
Expected: PASS

- [ ] **Step 6: Commit session binding**

```bash
git add bridge
git commit -m "feat: bind single claude session"
```

### Task 5: Render the floating indicator UI

**Files:**
- Create: `claude-status-light/src/components/StatusLight.tsx`
- Create: `claude-status-light/src/components/StatusLabel.tsx`
- Create: `claude-status-light/src/components/__tests__/StatusLight.test.tsx`
- Modify: `claude-status-light/src/App.tsx`
- Modify: `claude-status-light/src/styles.css`

- [ ] **Step 1: Write the failing visual-state test**

```tsx
// src/components/__tests__/StatusLight.test.tsx
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { StatusLight } from '../StatusLight';

describe('StatusLight', () => {
  it('renders running as yellow', () => {
    render(<StatusLight status="running" />);
    expect(screen.getByTestId('status-light')).toHaveAttribute('data-status', 'running');
  });

  it('renders pending_user as blinking red', () => {
    render(<StatusLight status="pending_user" />);
    expect(screen.getByTestId('status-light')).toHaveAttribute('data-status', 'pending_user');
  });
});
```

- [ ] **Step 2: Run the component test to verify it fails**

Run: `npm test -- src/components/__tests__/StatusLight.test.tsx`
Expected: FAIL with module not found error for `StatusLight`

- [ ] **Step 3: Implement the light and label components**

```tsx
// src/components/StatusLight.tsx
import type { StatusKind } from '../lib/types';

export function StatusLight({ status }: { status: StatusKind }) {
  return <div className="status-light" data-status={status} data-testid="status-light" />;
}
```

```tsx
// src/components/StatusLabel.tsx
import type { StatusKind } from '../lib/types';

export function StatusLabel({ status }: { status: StatusKind }) {
  return <div className="status-label">{status.replace('_', ' ')}</div>;
}
```

- [ ] **Step 4: Replace the placeholder app UI**

```tsx
// src/App.tsx
import { StatusLight } from './components/StatusLight';
import { StatusLabel } from './components/StatusLabel';
import { createDefaultState } from './lib/default-state';

export default function App() {
  const state = createDefaultState();

  return (
    <main className="app-shell">
      <StatusLight status={state.status} />
      <StatusLabel status={state.status} />
    </main>
  );
}
```

```css
/* append to src/styles.css */
.app-shell {
  width: 100%;
  height: 100%;
  display: grid;
  place-items: center;
  gap: 8px;
  padding: 10px;
  box-sizing: border-box;
}

.status-light {
  width: 52px;
  height: 52px;
  border-radius: 999px;
  box-shadow: 0 0 22px rgba(0, 0, 0, 0.35);
}

.status-light[data-status='idle_unbound'],
.status-light[data-status='bridge_disconnected'] {
  background: #8f96a3;
}

.status-light[data-status='running'] {
  background: #ffcb3b;
}

.status-light[data-status='pending_user'] {
  background: #ff4d4f;
  animation: pulse-red 1s infinite step-start;
}

.status-light[data-status='done'] {
  background: #2fbf71;
}

.status-label {
  color: #f6f7fb;
  font-size: 11px;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}

@keyframes pulse-red {
  0%, 49% {
    opacity: 1;
  }
  50%, 100% {
    opacity: 0.25;
  }
}
```

- [ ] **Step 5: Re-run the component test**

Run: `npm test -- src/components/__tests__/StatusLight.test.tsx`
Expected: PASS

- [ ] **Step 6: Commit UI shell**

```bash
git add src/App.tsx src/components src/styles.css
git commit -m "feat: render floating status light"
```

### Task 6: Poll the state file and update the UI

**Files:**
- Create: `claude-status-light/src/hooks/useStatusState.ts`
- Create: `claude-status-light/src/hooks/__tests__/useStatusState.test.tsx`
- Modify: `claude-status-light/src/App.tsx`
- Modify: `claude-status-light/src/lib/read-state.ts`
- Modify: `claude-status-light/src-tauri/src/lib.rs`

- [ ] **Step 1: Write the failing polling test**

```tsx
// src/hooks/__tests__/useStatusState.test.tsx
import { renderHook, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { useStatusState } from '../useStatusState';

describe('useStatusState', () => {
  it('loads state from the configured path', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({
      ok: true,
      text: async () => JSON.stringify({
        sessionId: 's1',
        status: 'running',
        updatedAt: '2026-06-07T10:30:00.000Z',
        soundEnabled: true,
        lastEvent: 'UserPromptSubmit',
        lastMessageText: '',
        doneReason: 'user_prompt_submit',
        bridgeHealthy: true
      })
    }));

    const { result } = renderHook(() => useStatusState('/state/state.json', 1000));

    await waitFor(() => {
      expect(result.current.status).toBe('running');
    });
  });
});
```

- [ ] **Step 2: Run the hook test to verify it fails**

Run: `npm test -- src/hooks/__tests__/useStatusState.test.tsx`
Expected: FAIL with module not found error for `useStatusState`

- [ ] **Step 3: Extend the parser with a fetch helper**

```ts
// append to src/lib/read-state.ts
export async function loadStateFromUrl(
  url: string,
  fallback: StatusState
): Promise<StatusState> {
  try {
    const response = await fetch(url, { cache: 'no-store' });
    if (!response.ok) {
      return fallback;
    }

    const raw = await response.text();
    return parseStateFile(raw, fallback);
  } catch {
    return fallback;
  }
}
```

- [ ] **Step 4: Implement the polling hook and wire it into the app**

```ts
// src/hooks/useStatusState.ts
import { useEffect, useRef, useState } from 'react';
import { createDefaultState } from '../lib/default-state';
import { loadStateFromUrl } from '../lib/read-state';
import type { StatusState } from '../lib/types';

export function useStatusState(url: string, intervalMs: number): StatusState {
  const [state, setState] = useState<StatusState>(createDefaultState());
  const latestState = useRef(state);

  useEffect(() => {
    latestState.current = state;
  }, [state]);

  useEffect(() => {
    let cancelled = false;

    async function refresh() {
      const next = await loadStateFromUrl(url, latestState.current);
      if (!cancelled) {
        latestState.current = next;
        setState(next);
      }
    }

    refresh();
    const timer = window.setInterval(refresh, intervalMs);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [intervalMs, url]);

  return state;
}
```

```tsx
// src/App.tsx
import { StatusLight } from './components/StatusLight';
import { StatusLabel } from './components/StatusLabel';
import { useStatusState } from './hooks/useStatusState';

export default function App() {
  const state = useStatusState('/state/state.json', 500);

  return (
    <main className="app-shell">
      <StatusLight status={state.status} />
      <StatusLabel status={state.status} />
    </main>
  );
}
```

```rust
// src-tauri/src/lib.rs
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 5: Re-run the hook test**

Run: `npm test -- src/hooks/__tests__/useStatusState.test.tsx`
Expected: PASS

- [ ] **Step 6: Run the app locally**

Run: `npm run dev`
Expected: browser window shows gray light initially

- [ ] **Step 7: Commit polling**

```bash
git add src/App.tsx src/hooks src/lib/read-state.ts
git commit -m "feat: poll local status state"
```

### Task 7: Add sound toggle and transition-only playback

**Files:**
- Create: `claude-status-light/src/lib/sound.ts`
- Create: `claude-status-light/src/lib/__tests__/sound.test.ts`
- Create: `claude-status-light/src/hooks/useSoundToggle.ts`
- Modify: `claude-status-light/src/App.tsx`

- [ ] **Step 1: Write the failing sound behavior tests**

```ts
// src/lib/__tests__/sound.test.ts
import { describe, expect, it, vi } from 'vitest';
import { playStatusSound, shouldPlayStatusSound } from '../sound';

describe('shouldPlayStatusSound', () => {
  it('plays when moving from running to done', () => {
    expect(shouldPlayStatusSound('running', 'done')).toBe(true);
  });

  it('does not play when state does not change', () => {
    expect(shouldPlayStatusSound('pending_user', 'pending_user')).toBe(false);
  });
});

describe('playStatusSound', () => {
  it('does nothing when muted', async () => {
    const play = vi.fn().mockResolvedValue(undefined);
    await playStatusSound({
      status: 'done',
      muted: true,
      createAudio: () => ({ play })
    });
    expect(play).not.toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: Run the sound tests to verify they fail**

Run: `npm test -- src/lib/__tests__/sound.test.ts`
Expected: FAIL with module not found error for `sound`

- [ ] **Step 3: Implement the sound helpers**

```ts
// src/lib/sound.ts
import type { StatusKind } from './types';

export function shouldPlayStatusSound(previous: StatusKind, next: StatusKind): boolean {
  return previous !== next;
}

export async function playStatusSound({
  status,
  muted,
  createAudio = (src: string) => new Audio(src)
}: {
  status: StatusKind;
  muted: boolean;
  createAudio?: (src: string) => { play: () => Promise<void> | void };
}) {
  if (muted || status === 'idle_unbound' || status === 'bridge_disconnected') {
    return;
  }

  if (status === 'running') {
    return;
  }

  const src = status === 'done' ? '/sounds/done.mp3' : '/sounds/pending.mp3';
  await createAudio(src).play();
}
```

- [ ] **Step 4: Add a sound toggle hook**

```ts
// src/hooks/useSoundToggle.ts
import { useEffect, useState } from 'react';

const storageKey = 'claude-status-light:sound-enabled';

export function useSoundToggle(defaultValue = true) {
  const [enabled, setEnabled] = useState(defaultValue);

  useEffect(() => {
    const saved = window.localStorage.getItem(storageKey);
    if (saved === 'true' || saved === 'false') {
      setEnabled(saved === 'true');
    }
  }, []);

  useEffect(() => {
    window.localStorage.setItem(storageKey, String(enabled));
  }, [enabled]);

  return {
    soundEnabled: enabled,
    toggleSound: () => setEnabled((current) => !current)
  };
}
```

- [ ] **Step 5: Wire sound playback into the app**

```tsx
// src/App.tsx
import { useEffect, useRef } from 'react';
import { StatusLight } from './components/StatusLight';
import { StatusLabel } from './components/StatusLabel';
import { useStatusState } from './hooks/useStatusState';
import { useSoundToggle } from './hooks/useSoundToggle';
import { playStatusSound, shouldPlayStatusSound } from './lib/sound';

export default function App() {
  const state = useStatusState('/state/state.json', 500);
  const { soundEnabled } = useSoundToggle(true);
  const previousStatus = useRef(state.status);

  useEffect(() => {
    if (shouldPlayStatusSound(previousStatus.current, state.status)) {
      void playStatusSound({
        status: state.status,
        muted: !soundEnabled
      });
      previousStatus.current = state.status;
    }
  }, [soundEnabled, state.status]);

  return (
    <main className="app-shell">
      <StatusLight status={state.status} />
      <StatusLabel status={state.status} />
    </main>
  );
}
```

- [ ] **Step 6: Re-run sound tests**

Run: `npm test -- src/lib/__tests__/sound.test.ts`
Expected: PASS

- [ ] **Step 7: Commit sound behavior**

```bash
git add src/App.tsx src/hooks/useSoundToggle.ts src/lib/sound.ts src/lib/__tests__/sound.test.ts
git commit -m "feat: add status sound playback"
```

### Task 8: Add tray actions and window controls

**Files:**
- Modify: `claude-status-light/src-tauri/src/lib.rs`
- Modify: `claude-status-light/src-tauri/tauri.conf.json`

- [ ] **Step 1: Write the native tray requirements as an acceptance checklist**

```text
- A tray menu item toggles visibility
- A tray menu item exits the app
- A tray menu item emits a reconnect event placeholder
```

- [ ] **Step 2: Implement tray and window commands**

```rust
// src-tauri/src/lib.rs
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, WebviewWindowBuilder, Wry,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let show_hide = MenuItem::with_id(app, "toggle_window", "Open/Hide", true, None::<&str>)?;
            let reconnect = MenuItem::with_id(app, "reconnect_session", "Reconnect Session", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_hide, &reconnect, &quit])?;

            TrayIconBuilder::new()
                .menu(&menu)
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
```

- [ ] **Step 3: Run a native smoke test**

Run: `npm run tauri:dev`
Expected: app launches with floating window and tray icon

- [ ] **Step 4: Manually verify tray actions**

Check:
- `Open/Hide` toggles window visibility
- tray left-click shows the window
- `Quit` closes the app

- [ ] **Step 5: Commit native controls**

```bash
git add src-tauri
git commit -m "feat: add tray window controls"
```

### Task 9: Document hook setup and run full verification

**Files:**
- Modify: `claude-status-light/bridge/README.md`
- Modify: `claude-status-light/2026-06-07-claude-status-light-design.md`

- [ ] **Step 1: Expand the bridge setup README with exact local workflow**

````md
## Local Development Flow

1. Run `npm install` in `C:/code/claude-status-light`
2. Run `npm run tauri:dev`
3. In a second shell, trigger the bridge manually:

```powershell
'{"session_id":"s1","hook_event_name":"UserPromptSubmit"}' | node .\bridge\claude-hook.mjs
```

4. Confirm `public/state/state.json` changes to `running`
5. Trigger a pending event and a done event the same way
````

- [ ] **Step 2: Run all automated tests**

Run: `npm test`
Expected: PASS

Run: `node --test bridge/__tests__/classify-event.test.mjs bridge/__tests__/write-state.test.mjs bridge/__tests__/session-binding.test.mjs`
Expected: PASS

- [ ] **Step 3: Run production build checks**

Run: `npm run build`
Expected: PASS

Run: `npm run tauri:build`
Expected: PASS or platform-specific bundle output without compile errors

- [ ] **Step 4: Perform end-to-end manual verification**

Check:
- app starts gray before any hook event
- `UserPromptSubmit` turns light yellow
- `Notification(permission_prompt)` turns light blinking red
- `Stop` with completion text turns light green
- toggling sound off suppresses further notification audio
- second session events do not replace the first bound session

- [ ] **Step 5: Commit docs and verification changes**

```bash
git add bridge/README.md 2026-06-07-claude-status-light-design.md
git commit -m "docs: add local setup and verification notes"
```

### Task 10: Claude plan-usage dials (post-MVP)

> Already implemented and verified. Checkboxes reflect completion.

**Files:**
- Create: `claude-status-light/src/lib/usage.ts`
- Create: `claude-status-light/src/lib/__tests__/usage.test.ts`
- Create: `claude-status-light/src/hooks/useClaudeUsage.ts`
- Create: `claude-status-light/src/components/UsagePanel.tsx`
- Modify: `claude-status-light/src-tauri/Cargo.toml`
- Modify: `claude-status-light/src-tauri/src/lib.rs`
- Modify: `claude-status-light/src/App.tsx`
- Modify: `claude-status-light/src/styles.css`
- Modify: `claude-status-light/src/lib/design.ts`
- Modify: `claude-status-light/src-tauri/tauri.conf.json`

- [x] **Step 1: Write failing tests for usage parsing and reset formatting**

Cover `parseClaudeUsage` (snake_case `five_hour` / `seven_day` from the backend), `clampUtilization`, and `formatResetIn` (minutes/hours/days, unparseable → empty).

Run: `npm test -- src/lib/__tests__/usage.test.ts`
Expected: FAIL (module not found), then PASS after Step 2.

- [x] **Step 2: Implement `usage.ts`**

`ClaudeUsage` / `UsageWindow` types, `parseClaudeUsage` (defensive, returns null when neither window is present), `clampUtilization` (round + 0-100), `formatResetIn(resetsAt, now)`.

- [x] **Step 3: Add the Rust `get_claude_usage` command**

In `Cargo.toml` add `reqwest = { version = "0.12", default-features = false, features = ["blocking", "rustls-tls-native-roots", "json"] }`. `rustls` avoids the Windows schannel revocation failure; native roots trust an intercepting/corporate CA.

In `lib.rs`, read the OAuth token from `~/.claude/.credentials.json` (`claudeAiOauth.accessToken`), then:

```rust
let response = client
    .get("https://api.anthropic.com/api/oauth/usage")
    .header("Authorization", format!("Bearer {token}"))
    .header("anthropic-beta", "oauth-2025-04-20")
    .header("anthropic-version", "2023-06-01")
    .send()?;
```

Parse `five_hour` / `seven_day` `{utilization, resets_at}` and return them. Run the blocking client inside `tauri::async_runtime::spawn_blocking`. Register `get_claude_usage` in the invoke handler.

- [x] **Step 4: Add `useClaudeUsage` and the `UsagePanel` dials**

Hook polls `get_claude_usage` every 5 minutes (the endpoint rate-limits hard) and keeps the last good value on any error. `UsagePanel` renders two SVG ring dials (`5H` / `7D` label on top, percent in center, `resets in Xh` below). Ring + percent are orange-yellow below 80% and orange at 80%+. Transparent center; dark text outline for legibility on any wallpaper.

- [x] **Step 5: Make room and verify**

Increase the design/window heights in `design.ts` + `tauri.conf.json`. Wire `UsagePanel` into `App.tsx`.

Run: `npm test` → PASS
Run: `npm run build` → PASS
Run: `cargo check` (inside the VS dev environment) → PASS

- [x] **Step 6: Commit**

```bash
git add src/lib/usage.ts src/hooks/useClaudeUsage.ts src/components/UsagePanel.tsx src/App.tsx src/styles.css src/lib/design.ts src-tauri
git commit -m "feat: show claude plan usage dials"
```

### Task 11: Show/Hide Details toggle (post-MVP)

> Already implemented and verified. Checkboxes reflect completion.

**Files:**
- Modify: `claude-status-light/src-tauri/src/lib.rs`
- Modify: `claude-status-light/src-tauri/capabilities/default.json`
- Modify: `claude-status-light/src/App.tsx`
- Modify: `claude-status-light/src/lib/design.ts`
- Modify: `claude-status-light/src/styles.css`

- [x] **Step 1: Add the tray item and event**

Add a `toggle_details` menu item that emits `toggle-details` to the main window, mirroring `toggle_sound`.

- [x] **Step 2: Grant the window-resize permission**

Add `core:window:allow-set-size` to `capabilities/default.json`.

- [x] **Step 3: Add the toggle in the app**

`detailsVisible` state (default true) hides the whole area below the light (status label, setup note, usage panel). A `toggle-details` listener and an in-window chevron button both call a sequenced handler:

- expanding: grow the window first, then render the details
- collapsing: render the collapsed layout first (await a paint), then shrink the window

This avoids a scale flash during the resize.

- [x] **Step 4: Keep the light from jumping**

Add a `COLLAPSED_DESIGN_HEIGHT` / `COLLAPSED_WINDOW_HEIGHT` so the light stays full scale when collapsed, and set `align-content: start` + `transform-origin: top` so content is top-anchored and the light holds a fixed position across toggles.

- [x] **Step 5: Verify**

Run: `npm test` → PASS
Run: `npm run build` → PASS
Run: `cargo check` → PASS
Manual: toggle from tray and from the chevron button; the light does not move and the window shrinks/grows with no flash.

- [x] **Step 6: Commit**

```bash
git add src-tauri src/App.tsx src/lib/design.ts src/styles.css
git commit -m "feat: add show/hide details toggle"
```

## Self-Review

Spec coverage:

- Single-session hook registration: Task 4
- Yellow running / red blinking pending / green done: Tasks 3, 5, 6
- Sound on/off: Task 7
- Floating app and tray: Tasks 5 and 8
- VS Code-compatible hook flow: Task 3 docs and bridge entrypoint
- Claude plan-usage dials: Task 10 (post-MVP)
- Show/Hide Details toggle with window resize: Task 11 (post-MVP)

Placeholder scan:

- No placeholder markers remain
- Every task has explicit files, commands, and expected outcomes

Type consistency:

- Shared status names are consistent across app state, classifier, tests, and UI
- `sessionId`, `doneReason`, and `bridgeHealthy` names stay stable across tasks
