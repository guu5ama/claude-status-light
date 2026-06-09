import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { appendDebugLog } from './append-debug-log.mjs';
import { classifyHookEvent } from './classify-event.mjs';
import { mergeSessionState, readCurrentState } from './read-current-state.mjs';
import { resolveDebugLogPath, resolveStatePath } from './runtime-paths.mjs';
import { writeStateAtomically } from './write-state.mjs';

const bridgeDir = path.dirname(fileURLToPath(import.meta.url));
const DEBUG_ENABLED_VALUES = new Set(['1', 'true', 'yes', 'on']);

function isDebugEnabled() {
  const value = process.env.CLAUDE_STATUS_LIGHT_DEBUG;
  if (typeof value !== 'string') {
    return false;
  }

  return DEBUG_ENABLED_VALUES.has(value.trim().toLowerCase());
}

async function writeDebugEntry(entry) {
  if (!isDebugEnabled()) {
    return;
  }

  await appendDebugLog(resolveDebugLogPath({ bridgeDir }), entry);
}

async function readStdin() {
  const chunks = [];

  for await (const chunk of process.stdin) {
    chunks.push(typeof chunk === 'string' ? Buffer.from(chunk) : chunk);
  }

  return Buffer.concat(chunks).toString('utf8');
}

async function main() {
  const raw = await readStdin();
  const payload = JSON.parse(raw);

  await writeDebugEntry({
    type: 'hook_received',
    at: new Date().toISOString(),
    hookEventName: payload?.hook_event_name ?? null,
    sessionId: payload?.session_id ?? null,
    notificationType: payload?.notification_type ?? null,
    hasLastAssistantMessage:
      typeof payload?.last_assistant_message === 'string' &&
      payload.last_assistant_message.trim().length > 0
  });

  const nextState = classifyHookEvent(payload);

  if (!nextState) {
    await writeDebugEntry({
      type: 'hook_ignored',
      at: new Date().toISOString(),
      hookEventName: payload?.hook_event_name ?? null,
      sessionId: payload?.session_id ?? null
    });
    return;
  }

  const statePath = resolveStatePath({ bridgeDir });
  const currentState = await readCurrentState(statePath);
  const mergedState = mergeSessionState(currentState, nextState);

  if (!mergedState) {
    await writeDebugEntry({
      type: 'state_merge_ignored',
      at: new Date().toISOString(),
      hookEventName: payload?.hook_event_name ?? null,
      currentSessionId: currentState?.sessionId ?? null,
      nextSessionId: nextState?.sessionId ?? null
    });
    return;
  }

  if (mergedState === currentState) {
    await writeDebugEntry({
      type: 'state_merge_ignored',
      at: new Date().toISOString(),
      hookEventName: payload?.hook_event_name ?? null,
      currentSessionId: currentState?.sessionId ?? null,
      nextSessionId: nextState?.sessionId ?? null
    });
    return;
  }

  await writeStateAtomically(statePath, mergedState);
  await writeDebugEntry({
    type: 'state_written',
    at: new Date().toISOString(),
    hookEventName: payload?.hook_event_name ?? null,
    sessionId: mergedState?.sessionId ?? null,
    status: mergedState?.status ?? null,
    doneReason: mergedState?.doneReason ?? null
  });
}

main().catch((error) => {
  void writeDebugEntry({
    type: 'hook_error',
    at: new Date().toISOString(),
    message: error instanceof Error ? error.message : String(error)
  });
  console.error(error);
  process.exit(1);
});
