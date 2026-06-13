export interface UsageWindow {
  utilization: number;
  resetsAt: string;
}

export interface ClaudeUsage {
  fiveHour: UsageWindow | null;
  sevenDay: UsageWindow | null;
}

function parseWindow(value: unknown): UsageWindow | null {
  if (typeof value !== 'object' || value === null) {
    return null;
  }

  const obj = value as Record<string, unknown>;
  const { utilization, resets_at: resetsAt } = obj;

  if (typeof utilization !== 'number' || typeof resetsAt !== 'string') {
    return null;
  }

  return { utilization, resetsAt };
}

export function parseClaudeUsage(value: unknown): ClaudeUsage | null {
  if (typeof value !== 'object' || value === null) {
    return null;
  }

  const obj = value as Record<string, unknown>;
  const fiveHour = parseWindow(obj.five_hour);
  const sevenDay = parseWindow(obj.seven_day);

  if (!fiveHour && !sevenDay) {
    return null;
  }

  return { fiveHour, sevenDay };
}

export type UsageErrorKind = 'no_active_login' | 'transient';

export interface UsageError {
  kind: UsageErrorKind;
  message: string;
}

export interface UsagePayload {
  configDirLabel: string;
  usage: ClaudeUsage | null;
  error: UsageError | null;
}

function parseUsageError(value: unknown): UsageError | null {
  if (typeof value !== 'object' || value === null) {
    return null;
  }

  const obj = value as Record<string, unknown>;
  if (typeof obj.message !== 'string') {
    return null;
  }

  return {
    kind: obj.kind === 'no_active_login' ? 'no_active_login' : 'transient',
    message: obj.message
  };
}

export function parseUsagePayload(value: unknown): UsagePayload | null {
  if (typeof value !== 'object' || value === null) {
    return null;
  }

  const obj = value as Record<string, unknown>;
  const configDirLabel = typeof obj.configDirLabel === 'string' ? obj.configDirLabel : '';
  const usage = parseClaudeUsage(obj.usage);
  const error = parseUsageError(obj.error);

  if (!usage && !error) {
    return null;
  }

  return { configDirLabel, usage, error };
}

export function clampUtilization(utilization: number): number {
  if (!Number.isFinite(utilization)) {
    return 0;
  }
  return Math.min(100, Math.max(0, Math.round(utilization)));
}

export function formatResetIn(resetsAt: string, now: number = Date.now()): string {
  const target = Date.parse(resetsAt);
  if (Number.isNaN(target)) {
    return '';
  }

  const diffMs = target - now;
  if (diffMs <= 0) {
    return 'resets now';
  }

  const totalMinutes = Math.round(diffMs / 60000);
  if (totalMinutes < 60) {
    return `resets in ${Math.max(1, totalMinutes)}m`;
  }

  const totalHours = Math.round(totalMinutes / 60);
  if (totalHours < 24) {
    return `resets in ${totalHours}h`;
  }

  const days = Math.round(totalHours / 24);
  return `resets in ${days}d`;
}
