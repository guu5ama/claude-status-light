import { clampUtilization, formatResetIn, type ClaudeUsage, type UsageWindow } from '../lib/usage';

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

export function UsagePanel({ usage }: { usage: ClaudeUsage | null }) {
  if (!usage || (!usage.fiveHour && !usage.sevenDay)) {
    return null;
  }

  return (
    <div className="usage-panel" data-testid="usage-panel">
      {usage.fiveHour ? <UsageDial label="5H" window={usage.fiveHour} /> : null}
      {usage.sevenDay ? <UsageDial label="7D" window={usage.sevenDay} /> : null}
    </div>
  );
}
