import test from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';

import {
  resolveDebugLogPath,
  resolveStatePath
} from '../runtime-paths.mjs';

test('resolveStatePath prefers the repo-local dev state file when it exists', () => {
  const bridgeDir = path.resolve('C:/code/claude-status-light/bridge');

  const resolved = resolveStatePath({
    bridgeDir,
    configuredPath: '',
    fileExists: (candidate) =>
      candidate === path.resolve(bridgeDir, '../public/state/state.json')
  });

  assert.equal(
    resolved,
    path.resolve(bridgeDir, '../public/state/state.json')
  );
});

test('resolveStatePath falls back to a portable Windows user-data path when no dev state file exists', () => {
  const bridgeDir = 'D:/Apps/Claude Status Light Portable/bridge';

  const resolved = resolveStatePath({
    bridgeDir,
    configuredPath: '',
    platform: 'win32',
    homeDir: 'C:/Users/shan',
    localAppData: 'C:/Users/shan/AppData/Local',
    fileExists: () => false
  });

  assert.equal(
    resolved,
    path.resolve('C:/Users/shan/AppData/Local/Claude Status Light/state/state.json')
  );
});

test('resolveDebugLogPath falls back next to the portable state file', () => {
  const bridgeDir = 'D:/Apps/Claude Status Light Portable/bridge';

  const resolved = resolveDebugLogPath({
    bridgeDir,
    configuredPath: '',
    platform: 'win32',
    homeDir: 'C:/Users/shan',
    localAppData: 'C:/Users/shan/AppData/Local',
    fileExists: () => false
  });

  assert.equal(
    resolved,
    path.resolve('C:/Users/shan/AppData/Local/Claude Status Light/state/hook-debug.jsonl')
  );
});
