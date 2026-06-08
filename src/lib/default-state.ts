import type { StatusState } from './types';

export function createDefaultState(): StatusState {
  return {
    sessionId: null,
    status: 'idle_unbound',
    updatedAt: '',
    soundEnabled: true,
    lastEvent: null,
    lastMessageText: '',
    doneReason: 'not_bound',
    bridgeHealthy: false
  };
}
