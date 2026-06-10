const WAITING_PATTERNS = [
  /\bplease confirm\b/,
  /\bdo you want me to continue\b/,
  /\bwhich option do you want\b/,
  /\bneed more information\b/,
  /\bneed (?:your )?permission\b/,
  /\bgrant permission\b/,
  /\bwaiting for (?:your )?(?:input|confirmation|permission)\b/,
  /\blet me know\b/
];

const NEGATED_COMPLETION_PATTERNS = [
  /\bnot\s+(?:yet\s+)?(?:done|completed|finished|fixed|implemented)\b/,
  /\b(?:isn't|isnt|aren't|arent|wasn't|wasnt)\s+(?:done|completed|finished|fixed|implemented)\b/,
  /\b(?:unfinished|incomplete)\b/
];

const COMPLETION_PATTERNS = [
  /\btests?\s+passed\b/,
  /\ball\s+tests?\s+pass(?:ed)?\b/,
  /\b(?:implemented|completed|finished|fixed|done)\b/
];

const PENDING_NOTIFICATION_TYPES = new Set([
  'permission_prompt',
  'idle_prompt',
  'elicitation_dialog'
]);

const USER_INTERACTION_TOOL_NAMES = new Set(['AskUserQuestion']);

function normalizeText(value) {
  return typeof value === 'string' ? value.trim().toLowerCase() : '';
}

function matchesAny(text, patterns) {
  return patterns.some((pattern) => pattern.test(text));
}

function createState(payload, status, doneReason) {
  return {
    sessionId: payload.session_id ?? null,
    status,
    updatedAt: new Date().toISOString(),
    soundEnabled: true,
    lastEvent: payload.hook_event_name ?? null,
    lastMessageText: payload.last_assistant_message ?? '',
    doneReason,
    bridgeHealthy: true
  };
}

function inferStopState(message) {
  const text = normalizeText(message);
  if (!text) {
    return {
      status: 'pending_user',
      doneReason: 'missing_assistant_text'
    };
  }

  if (matchesAny(text, WAITING_PATTERNS) || text.endsWith('?') || text.endsWith('？')) {
    return {
      status: 'pending_user',
      doneReason: 'assistant_waiting_for_input'
    };
  }

  if (matchesAny(text, NEGATED_COMPLETION_PATTERNS)) {
    return {
      status: 'pending_user',
      doneReason: 'assistant_waiting_for_input'
    };
  }

  if (matchesAny(text, COMPLETION_PATTERNS)) {
    return {
      status: 'done',
      doneReason: 'assistant_signaled_completion'
    };
  }

  return {
    status: 'done',
    doneReason: 'assistant_answered_without_followup'
  };
}

export function classifyHookEvent(payload) {
  const eventName = payload?.hook_event_name;

  if (eventName === 'UserPromptSubmit') {
    return createState(payload, 'running', 'user_prompt_submit');
  }

  if (
    eventName === 'Notification' &&
    PENDING_NOTIFICATION_TYPES.has(payload?.notification_type)
  ) {
    return createState(payload, 'pending_user', 'notification_pending_user');
  }

  if (
    eventName === 'PreToolUse' &&
    USER_INTERACTION_TOOL_NAMES.has(payload?.tool_name)
  ) {
    return createState(payload, 'pending_user', 'tool_waiting_for_user_input');
  }

  // After a tool finishes (including the user answering an AskUserQuestion or
  // approving a permission), Claude is working again. Claude Code does not fire
  // UserPromptSubmit for those, so PostToolUse is what moves the light off red.
  if (eventName === 'PostToolUse') {
    return createState(payload, 'running', 'tool_completed');
  }

  if (eventName === 'Stop') {
    const inferred = inferStopState(payload?.last_assistant_message);
    return createState(payload, inferred.status, inferred.doneReason);
  }

  return null;
}
