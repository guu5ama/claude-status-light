import fs from 'node:fs/promises';
import path from 'node:path';

function normalizeSessionId(sessionId) {
  if (typeof sessionId !== 'string') {
    return null;
  }

  const trimmed = sessionId.trim();
  return trimmed ? trimmed : null;
}

export async function readCurrentState(filePath) {
  const targetPath = path.resolve(filePath);

  try {
    const raw = await fs.readFile(targetPath, 'utf8');
    const parsed = JSON.parse(raw);

    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
      return null;
    }

    return parsed;
  } catch {
    return null;
  }
}

export function mergeSessionState(current, next) {
  if (!next || typeof next !== 'object') {
    return current ?? null;
  }

  const currentSessionId = normalizeSessionId(current?.sessionId);
  const nextSessionId = normalizeSessionId(next.sessionId);

  if (!nextSessionId) {
    return current ?? null;
  }

  if (!current || !currentSessionId) {
    const merged = {
      ...(current && typeof current === 'object' ? current : {}),
      ...next,
      sessionId: nextSessionId
    };

    if (typeof current?.soundEnabled === 'boolean') {
      merged.soundEnabled = current.soundEnabled;
    }

    return merged;
  }

  if (nextSessionId !== currentSessionId) {
    return current;
  }

  return {
    ...current,
    ...next,
    sessionId: currentSessionId
  };
}
