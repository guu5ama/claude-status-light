import { useEffect, useState } from 'react';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { parseUsagePayload, type ClaudeUsage, type UsageError } from '../lib/usage';

// The /api/oauth/usage endpoint rate-limits aggressively, so poll sparingly
// and keep the last good value across failures.
const DEFAULT_INTERVAL_MS = 5 * 60 * 1000;

export interface ClaudeUsageState {
  usage: ClaudeUsage | null;
  error: UsageError | null;
  configDirLabel: string | null;
}

const EMPTY_USAGE_STATE: ClaudeUsageState = {
  usage: null,
  error: null,
  configDirLabel: null
};

export function useClaudeUsage(intervalMs: number = DEFAULT_INTERVAL_MS): ClaudeUsageState {
  const [state, setState] = useState<ClaudeUsageState>(EMPTY_USAGE_STATE);

  useEffect(() => {
    if (!isTauri()) {
      return;
    }

    let cancelled = false;
    let unlistenProfileChanged: (() => void) | undefined;

    async function refresh() {
      try {
        const raw = await invoke('get_claude_usage');
        const payload = parseUsagePayload(raw);
        if (cancelled || !payload) {
          return;
        }

        setState((previous) => {
          const configDirLabel = payload.configDirLabel || previous.configDirLabel;

          if (payload.usage) {
            return { usage: payload.usage, error: null, configDirLabel };
          }

          if (payload.error?.kind === 'no_active_login') {
            return { usage: null, error: payload.error, configDirLabel };
          }

          // Transient failures (network, rate limit, server errors) keep the
          // last good value instead of flashing an error.
          return { ...previous, configDirLabel };
        });
      } catch {
        // Keep the last good value on invoke failures.
      }
    }

    async function bindProfileChanges() {
      unlistenProfileChanged = await listen('active-profile-changed', () => {
        // The account switched: drop the previous account's numbers right
        // away so they are never shown against the new label.
        setState(EMPTY_USAGE_STATE);
        void refresh();
      });
    }

    void refresh();
    void bindProfileChanges();
    const timer = window.setInterval(() => {
      void refresh();
    }, intervalMs);

    return () => {
      cancelled = true;
      window.clearInterval(timer);
      if (unlistenProfileChanged) {
        unlistenProfileChanged();
      }
    };
  }, [intervalMs]);

  return state;
}
