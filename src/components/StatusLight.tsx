import type { StatusKind } from '../lib/types';

const ACTIVE_LENS_BY_STATUS: Record<StatusKind, 'red' | 'yellow' | 'green' | null> = {
  idle_unbound: null,
  bridge_disconnected: null,
  running: 'yellow',
  pending_user: 'red',
  done: 'green'
};

const LENSES = [
  { color: 'red', testId: 'lens-red' },
  { color: 'yellow', testId: 'lens-yellow' },
  { color: 'green', testId: 'lens-green' }
] as const;

export function StatusLight({ status }: { status: StatusKind }) {
  const activeLens = ACTIVE_LENS_BY_STATUS[status];

  return (
    <div
      className="status-light"
      data-status={status}
      data-tauri-drag-region
      data-testid="status-light"
    >
      <div className="status-light__housing">
        {LENSES.map((lens) => (
          <div className="status-light__chamber" key={lens.color}>
            <div className="status-light__visor" aria-hidden="true" />
            <div
              className={`status-light__lens status-light__lens--${lens.color}`}
              data-active={String(activeLens === lens.color)}
              data-testid={lens.testId}
            />
          </div>
        ))}
      </div>
    </div>
  );
}
