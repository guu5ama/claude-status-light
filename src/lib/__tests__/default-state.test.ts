import { describe, expect, it } from 'vitest';
import { createDefaultState } from '../default-state';

describe('createDefaultState', () => {
  it('returns gray idle state before any session is bound', () => {
    expect(createDefaultState()).toEqual({
      sessionId: null,
      status: 'idle_unbound',
      updatedAt: '',
      soundEnabled: true,
      lastEvent: null,
      lastMessageText: '',
      doneReason: 'not_bound',
      bridgeHealthy: false
    });
  });
});
