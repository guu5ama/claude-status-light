import { useEffect, useRef, useState } from 'react';
import { createDefaultState } from '../lib/default-state';
import { loadState } from '../lib/read-state';
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
      const next = await loadState(url, latestState.current);
      if (!cancelled) {
        latestState.current = next;
        setState(next);
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
  }, [intervalMs, url]);

  return state;
}
