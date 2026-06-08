import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { spawn } from 'node:child_process';

const projectRoot = path.resolve(import.meta.dirname, '..', '..');
const hookPath = path.join(projectRoot, 'bridge', 'claude-hook.mjs');

function runHook(payload, statePath, extraEnv = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, [hookPath], {
      cwd: projectRoot,
      env: {
        ...process.env,
        CLAUDE_STATUS_LIGHT_STATE_PATH: statePath,
        ...extraEnv
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

test('claude-hook reads stdin JSON and writes classified state to override path', async () => {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), 'claude-status-light-hook-'));
  const statePath = path.join(dir, 'hook-state.json');

  await runHook(
    {
      session_id: 's1',
      hook_event_name: 'Stop',
      last_assistant_message: 'Implemented permission handling and tests passed.'
    },
    statePath
  );

  const raw = await fs.readFile(statePath, 'utf8');
  const parsed = JSON.parse(raw);

  assert.equal(parsed.sessionId, 's1');
  assert.equal(parsed.lastEvent, 'Stop');
  assert.equal(parsed.status, 'done');
  assert.equal(parsed.doneReason, 'assistant_signaled_completion');
  assert.equal(parsed.bridgeHealthy, true);
});

test('claude-hook does not write a debug log unless debug mode is enabled', async () => {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), 'claude-status-light-hook-no-debug-'));
  const statePath = path.join(dir, 'hook-state.json');
  const debugLogPath = path.join(dir, 'hook-debug.jsonl');

  await runHook(
    {
      session_id: 's1',
      hook_event_name: 'UserPromptSubmit'
    },
    statePath,
    {
      CLAUDE_STATUS_LIGHT_DEBUG_LOG_PATH: debugLogPath
    }
  );

  await assert.rejects(fs.readFile(debugLogPath, 'utf8'));
});

test('claude-hook writes a debug log when debug mode is enabled', async () => {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), 'claude-status-light-hook-debug-'));
  const statePath = path.join(dir, 'hook-state.json');
  const debugLogPath = path.join(dir, 'hook-debug.jsonl');

  await runHook(
    {
      session_id: 's1',
      hook_event_name: 'UserPromptSubmit'
    },
    statePath,
    {
      CLAUDE_STATUS_LIGHT_DEBUG: '1',
      CLAUDE_STATUS_LIGHT_DEBUG_LOG_PATH: debugLogPath
    }
  );

  const raw = await fs.readFile(debugLogPath, 'utf8');
  assert.match(raw, /"type":"hook_received"/);
  assert.match(raw, /"type":"state_written"/);
});
