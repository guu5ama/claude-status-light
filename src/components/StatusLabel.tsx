import { formatStatusLabel } from '../lib/claude-setup';
import type { StatusKind } from '../lib/types';

export function StatusLabel({
  status,
  text
}: {
  status: StatusKind;
  text?: string;
}) {
  return (
    <div className="status-label" data-tauri-drag-region>
      {text ?? formatStatusLabel(status)}
    </div>
  );
}
