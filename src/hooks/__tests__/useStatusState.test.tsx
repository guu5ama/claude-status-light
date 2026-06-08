import { renderHook, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { useStatusState } from '../useStatusState';

describe('useStatusState', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('loads state from the configured path', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue({
        ok: true,
        text: async () =>
          JSON.stringify({
            sessionId: 's1',
            status: 'running',
            updatedAt: '2026-06-07T10:30:00.000Z',
            soundEnabled: true,
            lastEvent: 'UserPromptSubmit',
            lastMessageText: '',
            doneReason: 'user_prompt_submit',
            bridgeHealthy: true
          })
      })
    );

    const { result } = renderHook(() => useStatusState('/state/state.json', 1000));

    await waitFor(() => {
      expect(result.current.status).toBe('running');
    });
  });
});
