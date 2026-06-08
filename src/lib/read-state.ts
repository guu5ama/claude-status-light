import { invoke, isTauri } from '@tauri-apps/api/core';
import { createDefaultState } from './default-state';
import { STATUS_KINDS, type StatusKind, type StatusState } from './types';

function isStatusKind(value: unknown): value is StatusKind {
  return typeof value === 'string' && STATUS_KINDS.includes(value as StatusKind);
}

export function parseStateFile(
  raw: string,
  fallback: StatusState = createDefaultState()
): StatusState {
  try {
    const parsed = JSON.parse(raw) as Partial<StatusState> | null;

    if (typeof parsed !== 'object' || parsed === null) {
      return fallback;
    }

    if (!isStatusKind(parsed.status)) {
      return fallback;
    }

    return {
      sessionId:
        parsed.sessionId === null || typeof parsed.sessionId === 'string'
          ? parsed.sessionId
          : fallback.sessionId,
      status: parsed.status,
      updatedAt: typeof parsed.updatedAt === 'string' ? parsed.updatedAt : fallback.updatedAt,
      soundEnabled:
        typeof parsed.soundEnabled === 'boolean' ? parsed.soundEnabled : fallback.soundEnabled,
      lastEvent:
        parsed.lastEvent === null || typeof parsed.lastEvent === 'string'
          ? parsed.lastEvent
          : fallback.lastEvent,
      lastMessageText:
        typeof parsed.lastMessageText === 'string'
          ? parsed.lastMessageText
          : fallback.lastMessageText,
      doneReason: typeof parsed.doneReason === 'string' ? parsed.doneReason : fallback.doneReason,
      bridgeHealthy:
        typeof parsed.bridgeHealthy === 'boolean' ? parsed.bridgeHealthy : fallback.bridgeHealthy
    };
  } catch {
    return fallback;
  }
}

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

export async function loadState(
  url: string,
  fallback: StatusState
): Promise<StatusState> {
  if (!isTauri()) {
    return loadStateFromUrl(url, fallback);
  }

  try {
    const raw = await invoke<string>('read_state_file');
    return parseStateFile(raw, fallback);
  } catch {
    return fallback;
  }
}
