import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { spawn } from 'node:child_process';
import { mergeSessionState } from '../read-current-state.mjs';

const projectRoot = path.resolve(import.meta.dirname, '..', '..');
const hookPath = path.join(projectRoot, 'bridge', 'claude-hook.mjs');

function runHook(payload, statePath) {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, [hookPath], {
      cwd: projectRoot,
      env: {
        ...process.env,
        CLAUDE_STATUS_LIGHT_STATE_PATH: statePath
      },
      stdio: ['pipe', 'pipe', 'pipe']
    });

    let stderr = '';
    child.stderr.setEncoding('utf8');
    child.stderr.on('data', (chunk) => {
      stderr += chunk;
    });

    child.on('error', reject);
    child.on('close', (code) => {
      if (code !== 0) {
        reject(new Error(`hook exited with code ${code}: ${stderr}`));
        return;
      }

      resolve();
    });

    child.stdin.end(JSON.stringify(payload));
  });
}

test('binds first session when none exists', () => {
  const next = mergeSessionState(null, {
    sessionId: 's1',
    status: 'running'
  });

  assert.equal(next.sessionId, 's1');
  assert.equal(next.status, 'running');
});

test('does not bind a missing or invalid session id', () => {
  assert.equal(
    mergeSessionState(null, {
      sessionId: null,
      status: 'running'
    }),
    null
  );

  assert.equal(
    mergeSessionState(null, {
      sessionId: '   ',
      status: 'running'
    }),
    null
  );

  const current = {
    sessionId: 's1',
    status: 'pending_user'
  };

  const next = mergeSessionState(current, {
    sessionId: '',
    status: 'done'
  });

  assert.equal(next, current);
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

test('first bind preserves existing unbound state fields that should survive', () => {
  const current = {
    sessionId: null,
    status: 'idle_unbound',
    updatedAt: '',
    soundEnabled: false,
    lastEvent: null,
    lastMessageText: '',
    doneReason: 'not_bound',
    bridgeHealthy: false
  };

  const next = mergeSessionState(current, {
    sessionId: 's1',
    status: 'running',
    updatedAt: '2026-06-07T12:00:00.000Z',
    soundEnabled: true,
    lastEvent: 'UserPromptSubmit',
    lastMessageText: '',
    doneReason: 'user_prompt_submit',
    bridgeHealthy: true
  });

  assert.equal(next.sessionId, 's1');
  assert.equal(next.status, 'running');
  assert.equal(next.soundEnabled, false);
  assert.equal(next.lastEvent, 'UserPromptSubmit');
  assert.equal(next.doneReason, 'user_prompt_submit');
  assert.equal(next.bridgeHealthy, true);
});

test('claude-hook preserves the first bound session across later events', async () => {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), 'claude-status-light-bind-'));
  const statePath = path.join(dir, 'hook-state.json');

  await runHook(
    {
      session_id: 's1',
      hook_event_name: 'UserPromptSubmit'
    },
    statePath
  );

  await runHook(
    {
      session_id: 's2',
      hook_event_name: 'Stop',
      last_assistant_message: 'Implemented the fix and tests passed.'
    },
    statePath
  );

  const raw = await fs.readFile(statePath, 'utf8');
  const parsed = JSON.parse(raw);

  assert.equal(parsed.sessionId, 's1');
  assert.equal(parsed.status, 'running');
  assert.equal(parsed.lastEvent, 'UserPromptSubmit');
  assert.equal(parsed.doneReason, 'user_prompt_submit');
});

test('claude-hook does not rewrite the state file for a different session after binding', async () => {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), 'claude-status-light-bind-ignore-'));
  const statePath = path.join(dir, 'hook-state.json');

  await runHook(
    {
      session_id: 's1',
      hook_event_name: 'UserPromptSubmit'
    },
    statePath
  );

  const firstStats = await fs.stat(statePath);

  await new Promise((resolve) => setTimeout(resolve, 25));

  await runHook(
    {
      session_id: 's2',
      hook_event_name: 'Stop',
      last_assistant_message: 'Implemented the fix and tests passed.'
    },
    statePath
  );

  const secondStats = await fs.stat(statePath);

  assert.equal(secondStats.mtimeMs, firstStats.mtimeMs);
});

test('claude-hook does not create a colored state file for a missing session id', async () => {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), 'claude-status-light-missing-session-'));
  const statePath = path.join(dir, 'hook-state.json');

  await runHook(
    {
      hook_event_name: 'UserPromptSubmit'
    },
    statePath
  );

  await assert.rejects(fs.readFile(statePath, 'utf8'));
});

test('claude-hook preserves soundEnabled when first binding over an unbound file', async () => {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), 'claude-status-light-unbound-'));
  const statePath = path.join(dir, 'hook-state.json');

  await fs.writeFile(
    statePath,
    `${JSON.stringify({
      sessionId: null,
      status: 'idle_unbound',
      updatedAt: '',
      soundEnabled: false,
      lastEvent: null,
      lastMessageText: '',
      doneReason: 'not_bound',
      bridgeHealthy: false
    })}\n`,
    'utf8'
  );

  await runHook(
    {
      session_id: 's1',
      hook_event_name: 'UserPromptSubmit'
    },
    statePath
  );

  const raw = await fs.readFile(statePath, 'utf8');
  const parsed = JSON.parse(raw);

  assert.equal(parsed.sessionId, 's1');
  assert.equal(parsed.status, 'running');
  assert.equal(parsed.soundEnabled, false);
  assert.equal(parsed.lastEvent, 'UserPromptSubmit');
});
