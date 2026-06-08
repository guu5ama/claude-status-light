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
