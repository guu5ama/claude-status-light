import type { StatusKind } from './types';

export const CLAUDE_SETUP_STATUS_KINDS = [
  'configured',
  'already_configured',
  'failed'
] as const;

export type ClaudeSetupStatusKind = (typeof CLAUDE_SETUP_STATUS_KINDS)[number];

export interface ClaudeSetupStatus {
  kind: ClaudeSetupStatusKind;
  message: string;
  settingsPath: string;
  activeBridgePath: string | null;
  backupPath: string | null;
  wroteChanges: boolean;
  requiresClaudeRestart: boolean;
}

export interface ClaudeSetupNotice {
  tone: 'success' | 'error';
  title: string;
  detail: string | null;
}

export const SETUP_SUCCESS_NOTICE_DURATION_MS = 8000;

export function formatStatusLabel(status: StatusKind): string {
  return status.replace(/_/g, ' ').toUpperCase();
}

export function getStatusLabelText(
  status: StatusKind,
  setupStatus: ClaudeSetupStatus | null
): string {
  if (setupStatus?.kind === 'failed') {
    return 'SETUP NEEDED';
  }

  return formatStatusLabel(status);
}

export function shouldDismissSetupNoticeForStatus(
  setupStatus: ClaudeSetupStatus | null,
  status: StatusKind
): boolean {
  return (
    setupStatus?.kind === 'configured' &&
    (status === 'running' || status === 'pending_user' || status === 'done')
  );
}

export function getSetupNotice(
  setupStatus: ClaudeSetupStatus | null
): ClaudeSetupNotice | null {
  if (!setupStatus) {
    return null;
  }

  if (setupStatus.kind === 'configured') {
    const details = [];

    if (setupStatus.activeBridgePath) {
      details.push(`Active bridge: ${setupStatus.activeBridgePath}`);
    }

    if (setupStatus.backupPath) {
      details.push(`Backup: ${setupStatus.backupPath}`);
    }

    return {
      tone: 'success',
      title: 'HOOKS UPDATED',
      detail: details.length > 0 ? details.join('\n') : setupStatus.message
    };
  }

  if (setupStatus.kind === 'failed') {
    return {
      tone: 'error',
      title: 'SETUP NEEDED',
      detail: setupStatus.message
    };
  }

  return null;
}
