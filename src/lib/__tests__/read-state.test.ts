import { describe, expect, it } from 'vitest';
import { parseStateFile } from '../read-state';
import type { StatusState } from '../types';

describe('parseStateFile', () => {
  it('accepts a valid state payload', () => {
    const parsed = parseStateFile(
      JSON.stringify({
        sessionId: 'session-1',
        status: 'running',
        updatedAt: '2026-06-07T10:30:00.000Z',
        soundEnabled: true,
        lastEvent: 'UserPromptSubmit',
        lastMessageText: '',
        doneReason: 'user_prompt_submit',
        bridgeHealthy: true
      })
    );

    expect(parsed.status).toBe('running');
    expect(parsed.sessionId).toBe('session-1');
  });

  it('falls back to previous state for malformed JSON', () => {
    const previous: StatusState = {
      sessionId: null,
      status: 'idle_unbound',
      updatedAt: '',
      soundEnabled: true,
      lastEvent: null,
      lastMessageText: '',
      doneReason: 'not_bound',
      bridgeHealthy: false
    };

    expect(parseStateFile('{bad json', previous)).toEqual(previous);
  });

  it('merges a partial valid payload over the provided fallback', () => {
    const previous: StatusState = {
      sessionId: 'session-previous',
      status: 'pending_user',
      updatedAt: '2026-06-07T10:00:00.000Z',
      soundEnabled: false,
      lastEvent: 'AgentMessageDelta',
      lastMessageText: 'Need input',
      doneReason: 'waiting_for_user',
      bridgeHealthy: true
    };

    const parsed = parseStateFile(
      JSON.stringify({
        status: 'running',
        updatedAt: '2026-06-07T10:30:00.000Z',
        bridgeHealthy: false,
        lastMessageText: 42
      }),
      previous
    );

    expect(parsed).toEqual({
      sessionId: 'session-previous',
      status: 'running',
      updatedAt: '2026-06-07T10:30:00.000Z',
      soundEnabled: false,
      lastEvent: 'AgentMessageDelta',
      lastMessageText: 'Need input',
      doneReason: 'waiting_for_user',
      bridgeHealthy: false
    });
  });
});
