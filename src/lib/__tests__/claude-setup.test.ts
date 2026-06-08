import { describe, expect, it } from 'vitest';
import {
  getSetupNotice,
  getStatusLabelText,
  shouldDismissSetupNoticeForStatus
} from '../claude-setup';

describe('getSetupNotice', () => {
  it('returns a success note with the active bridge path and backup path after automatic configuration writes changes', () => {
    expect(
      getSetupNotice({
        kind: 'configured',
        message: 'Claude hook bridge was configured successfully.',
        settingsPath: 'C:/Users/shan/.claude/settings.json',
        activeBridgePath: 'C:/Users/shan/.claude/hooks/claude-status-light-bridge.sh',
        backupPath: 'C:/Users/shan/.claude/settings.json.bak-20260607-182000',
        wroteChanges: true,
        requiresClaudeRestart: true
      })
    ).toEqual({
      tone: 'success',
      title: 'HOOKS UPDATED',
      detail:
        'Active bridge: C:/Users/shan/.claude/hooks/claude-status-light-bridge.sh\nBackup: C:/Users/shan/.claude/settings.json.bak-20260607-182000'
    });
  });

  it('returns no setup note when the current hooks were already configured', () => {
    expect(
      getSetupNotice({
        kind: 'already_configured',
        message: 'Claude hook bridge is already configured.',
        settingsPath: 'C:/Users/shan/.claude/settings.json',
        activeBridgePath: 'C:/Users/shan/.claude/hooks/claude-status-light-bridge.sh',
        backupPath: null,
        wroteChanges: false,
        requiresClaudeRestart: false
      })
    ).toBeNull();
  });

  it('keeps the normal status label when setup status is already_configured', () => {
    expect(
      getStatusLabelText('idle_unbound', {
        kind: 'already_configured',
        message: 'Claude hook bridge is already configured.',
        settingsPath: 'C:/Users/shan/.claude/settings.json',
        activeBridgePath: 'C:/Users/shan/.claude/hooks/claude-status-light-bridge.sh',
        backupPath: null,
        wroteChanges: false,
        requiresClaudeRestart: false
      })
    ).toBe('IDLE UNBOUND');
  });

  it('dismisses HOOKS UPDATED once a real Claude session status arrives', () => {
    const setupStatus = {
      kind: 'configured' as const,
      message: 'Claude hook bridge was configured successfully.',
      settingsPath: 'C:/Users/shan/.claude/settings.json',
      activeBridgePath: 'C:/Users/shan/.claude/hooks/claude-status-light-bridge.sh',
      backupPath: 'C:/Users/shan/.claude/settings.json.bak-20260607-182000',
      wroteChanges: true,
      requiresClaudeRestart: true
    };

    expect(shouldDismissSetupNoticeForStatus(setupStatus, 'idle_unbound')).toBe(false);
    expect(shouldDismissSetupNoticeForStatus(setupStatus, 'bridge_disconnected')).toBe(false);
    expect(shouldDismissSetupNoticeForStatus(setupStatus, 'running')).toBe(true);
    expect(shouldDismissSetupNoticeForStatus(setupStatus, 'pending_user')).toBe(true);
    expect(shouldDismissSetupNoticeForStatus(setupStatus, 'done')).toBe(true);
  });

  it('never auto-dismisses setup failures based on session status', () => {
    const failedSetupStatus = {
      kind: 'failed' as const,
      message: 'Could not write Claude settings.json.',
      settingsPath: 'C:/Users/shan/.claude/settings.json',
      activeBridgePath: null,
      backupPath: null,
      wroteChanges: false,
      requiresClaudeRestart: false
    };

    expect(shouldDismissSetupNoticeForStatus(failedSetupStatus, 'running')).toBe(false);
    expect(shouldDismissSetupNoticeForStatus(failedSetupStatus, 'done')).toBe(false);
  });
});
