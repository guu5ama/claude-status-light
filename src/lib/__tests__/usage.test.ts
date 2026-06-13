import { describe, expect, it } from 'vitest';
import { clampUtilization, formatResetIn, parseClaudeUsage, parseUsagePayload } from '../usage';

describe('parseClaudeUsage', () => {
  it('parses the snake_case shape returned by the backend', () => {
    const usage = parseClaudeUsage({
      five_hour: { utilization: 47, resets_at: '2026-06-09T08:00:00Z' },
      seven_day: { utilization: 67, resets_at: '2026-06-09T11:00:00Z' }
    });

    expect(usage).toEqual({
      fiveHour: { utilization: 47, resetsAt: '2026-06-09T08:00:00Z' },
      sevenDay: { utilization: 67, resetsAt: '2026-06-09T11:00:00Z' }
    });
  });

  it('keeps a window null when the backend reports it as null', () => {
    const usage = parseClaudeUsage({
      five_hour: { utilization: 47, resets_at: '2026-06-09T08:00:00Z' },
      seven_day: null
    });

    expect(usage?.fiveHour).not.toBeNull();
    expect(usage?.sevenDay).toBeNull();
  });

  it('returns null when neither window is present', () => {
    expect(parseClaudeUsage({ five_hour: null, seven_day: null })).toBeNull();
    expect(parseClaudeUsage(null)).toBeNull();
  });
});

describe('parseUsagePayload', () => {
  it('parses a successful payload with usage and config dir label', () => {
    const payload = parseUsagePayload({
      configDirLabel: '~/.claude-company',
      usage: {
        five_hour: { utilization: 47, resets_at: '2026-06-09T08:00:00Z' },
        seven_day: null
      },
      error: null
    });

    expect(payload?.configDirLabel).toBe('~/.claude-company');
    expect(payload?.usage?.fiveHour).toEqual({
      utilization: 47,
      resetsAt: '2026-06-09T08:00:00Z'
    });
    expect(payload?.error).toBeNull();
  });

  it('parses a no_active_login error payload', () => {
    const payload = parseUsagePayload({
      configDirLabel: '~/.claude-company',
      usage: null,
      error: { kind: 'no_active_login', message: 'No active login found for ~/.claude-company.' }
    });

    expect(payload?.usage).toBeNull();
    expect(payload?.error).toEqual({
      kind: 'no_active_login',
      message: 'No active login found for ~/.claude-company.'
    });
  });

  it('treats unknown error kinds as transient', () => {
    const payload = parseUsagePayload({
      configDirLabel: '~/.claude',
      usage: null,
      error: { kind: 'mystery', message: 'HTTP 500' }
    });

    expect(payload?.error?.kind).toBe('transient');
  });

  it('returns null for payloads without usage or error', () => {
    expect(parseUsagePayload({ configDirLabel: '~/.claude', usage: null, error: null })).toBeNull();
    expect(parseUsagePayload(null)).toBeNull();
    expect(parseUsagePayload('nope')).toBeNull();
  });
});

describe('clampUtilization', () => {
  it('rounds and bounds the value to 0-100', () => {
    expect(clampUtilization(46.7)).toBe(47);
    expect(clampUtilization(-5)).toBe(0);
    expect(clampUtilization(140)).toBe(100);
    expect(clampUtilization(Number.NaN)).toBe(0);
  });
});

describe('formatResetIn', () => {
  const now = Date.parse('2026-06-09T06:00:00Z');

  it('formats sub-hour windows in minutes', () => {
    expect(formatResetIn('2026-06-09T06:45:00Z', now)).toBe('resets in 45m');
  });

  it('formats multi-hour windows in hours', () => {
    expect(formatResetIn('2026-06-09T09:00:00Z', now)).toBe('resets in 3h');
  });

  it('formats multi-day windows in days', () => {
    expect(formatResetIn('2026-06-16T06:00:00Z', now)).toBe('resets in 7d');
  });

  it('returns an empty string for an unparseable timestamp', () => {
    expect(formatResetIn('not-a-date', now)).toBe('');
  });
});
