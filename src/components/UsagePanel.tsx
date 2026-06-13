import {
  clampUtilization,
  formatResetIn,
  type ClaudeUsage,
  type UsageError,
  type UsageWindow
} from '../lib/usage';

const HOT_THRESHOLD = 80;
const RADIUS = 15;
const CIRCUMFERENCE = 2 * Math.PI * RADIUS;

function UsageDial({ label, window }: { label: string; window: UsageWindow }) {
  const pct = clampUtilization(window.utilization);
  const hot = pct >= HOT_THRESHOLD;
  const dashOffset = CIRCUMFERENCE * (1 - pct / 100);
  const resetText = formatResetIn(window.resetsAt);

  return (
    <div className="usage-dial">
      <div className="usage-dial__label">{label}</div>
      <div className="usage-dial__ring">
        <svg className="usage-dial__svg" viewBox="0 0 40 40" aria-hidden="true">
          <circle className="usage-dial__track" cx="20" cy="20" r={RADIUS} />
          <circle
            className={`usage-dial__arc${hot ? ' usage-dial__arc--hot' : ''}`}
            cx="20"
            cy="20"
            r={RADIUS}
            strokeDasharray={CIRCUMFERENCE}
            strokeDashoffset={dashOffset}
            transform="rotate(-90 20 20)"
          />
        </svg>
        <span className={`usage-dial__pct${hot ? ' usage-dial__pct--hot' : ''}`}>{pct}%</span>
      </div>
      {resetText ? <div className="usage-dial__reset">{resetText}</div> : null}
    </div>
  );
}

export interface UsagePanelProps {
  usage: ClaudeUsage | null;
  error?: UsageError | null;
  configDirLabel?: string | null;
}

export function UsagePanel({ usage, error = null, configDirLabel = null }: UsagePanelProps) {
  const hasDials = Boolean(usage && (usage.fiveHour || usage.sevenDay));
  const loginError = error?.kind === 'no_active_login' ? error : null;

  if (!hasDials && !loginError) {
    return null;
  }

  return (
    <div className="usage-panel" data-testid="usage-panel">
      {configDirLabel ? (
        <div className="usage-panel__profile" title={configDirLabel}>
          {configDirLabel}
        </div>
      ) : null}
      {loginError ? (
        <div className="usage-panel__error" role="alert" title={loginError.message}>
          <div className="usage-panel__error-title">NO ACTIVE LOGIN</div>
          <div className="usage-panel__error-detail">{loginError.message}</div>
        </div>
      ) : (
        <div className="usage-panel__dials">
          {usage?.fiveHour ? <UsageDial label="5H" window={usage.fiveHour} /> : null}
          {usage?.sevenDay ? <UsageDial label="7D" window={usage.sevenDay} /> : null}
        </div>
      )}
    </div>
  );
}
