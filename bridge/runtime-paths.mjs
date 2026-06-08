import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const PORTABLE_APP_DIR_NAME = 'Claude Status Light';
const PORTABLE_STATE_DIR_SEGMENT = 'state';

function defaultFileExists(candidate) {
  return fs.existsSync(candidate);
}

function resolvePortableStateDir({
  platform = process.platform,
  homeDir = os.homedir(),
  localAppData = process.env.LOCALAPPDATA
} = {}) {
  if (platform === 'win32') {
    const baseDir =
      typeof localAppData === 'string' && localAppData.trim()
        ? localAppData
        : path.join(homeDir, 'AppData', 'Local');

    return path.resolve(baseDir, PORTABLE_APP_DIR_NAME, PORTABLE_STATE_DIR_SEGMENT);
  }

  if (platform === 'darwin') {
    return path.resolve(
      homeDir,
      'Library',
      'Application Support',
      PORTABLE_APP_DIR_NAME,
      PORTABLE_STATE_DIR_SEGMENT
    );
  }

  return path.resolve(homeDir, '.local', 'share', 'claude-status-light', PORTABLE_STATE_DIR_SEGMENT);
}

export function resolveStatePath({
  bridgeDir,
  configuredPath = process.env.CLAUDE_STATUS_LIGHT_STATE_PATH,
  platform = process.platform,
  homeDir = os.homedir(),
  localAppData = process.env.LOCALAPPDATA,
  fileExists = defaultFileExists
} = {}) {
  if (typeof configuredPath === 'string' && configuredPath.trim()) {
    return path.isAbsolute(configuredPath)
      ? configuredPath
      : path.resolve(bridgeDir, configuredPath);
  }

  const devPath = path.resolve(bridgeDir, '../public/state/state.json');
  if (fileExists(devPath)) {
    return devPath;
  }

  return path.resolve(
    resolvePortableStateDir({ platform, homeDir, localAppData }),
    'state.json'
  );
}

export function resolveDebugLogPath({
  bridgeDir,
  configuredPath = process.env.CLAUDE_STATUS_LIGHT_DEBUG_LOG_PATH,
  platform = process.platform,
  homeDir = os.homedir(),
  localAppData = process.env.LOCALAPPDATA,
  fileExists = defaultFileExists
} = {}) {
  if (typeof configuredPath === 'string' && configuredPath.trim()) {
    return path.isAbsolute(configuredPath)
      ? configuredPath
      : path.resolve(bridgeDir, configuredPath);
  }

  const devPath = path.resolve(bridgeDir, '../public/state/hook-debug.jsonl');
  if (fileExists(devPath)) {
    return devPath;
  }

  return path.resolve(
    resolvePortableStateDir({ platform, homeDir, localAppData }),
    'hook-debug.jsonl'
  );
}
