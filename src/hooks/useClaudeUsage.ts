import { useEffect, useRef, useState } from 'react';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { parseClaudeUsage, type ClaudeUsage } from '../lib/usage';

// The /api/oauth/usage endpoint rate-limits aggressively, so poll sparingly
// and keep the last good value across failures.
const DEFAULT_INTERVAL_MS = 5 * 60 * 1000;

export function useClaudeUsage(intervalMs: number = DEFAULT_INTERVAL_MS): ClaudeUsage | null {
  const [usage, setUsage] = useState<ClaudeUsage | null>(null);
  const latestUsage = useRef<ClaudeUsage | null>(null);

  useEffect(() => {
    if (!isTauri()) {
      return;
    }

    let cancelled = false;

    async function refresh() {
      try {
        const raw = await invoke('get_claude_usage');
        const next = parseClaudeUsage(raw);
        if (!cancelled && next) {
          latestUsage.current = next;
          setUsage(next);
        }
      } catch {
        // Keep the last good value on network errors / rate limits.
      }
    }

    void refresh();
    const timer = window.setInterval(() => {
      void refresh();
    }, intervalMs);

    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [intervalMs]);

  return usage;
}
