export const STATUS_KINDS = [
  'idle_unbound',
  'running',
  'pending_user',
  'done',
  'bridge_disconnected'
] as const;

export type StatusKind = (typeof STATUS_KINDS)[number];

export interface StatusState {
  sessionId: string | null;
  status: StatusKind;
  updatedAt: string;
  soundEnabled: boolean;
  lastEvent: string | null;
  lastMessageText: string;
  doneReason: string;
  bridgeHealthy: boolean;
}
