import test from 'node:test';
import assert from 'node:assert/strict';
import { classifyHookEvent } from '../classify-event.mjs';

test('maps UserPromptSubmit to running', () => {
  const result = classifyHookEvent({
    session_id: 's1',
    hook_event_name: 'UserPromptSubmit'
  });

  assert.equal(result.status, 'running');
  assert.equal(result.doneReason, 'user_prompt_submit');
});

test('maps AskUserQuestion tool requests to pending_user before the tool executes', () => {
  const result = classifyHookEvent({
    session_id: 's1',
    hook_event_name: 'PreToolUse',
    tool_name: 'AskUserQuestion'
  });

  assert.equal(result.status, 'pending_user');
  assert.equal(result.doneReason, 'tool_waiting_for_user_input');
});

test('maps PostToolUse to running so the light leaves red after the user answers a tool', () => {
  const answered = classifyHookEvent({
    session_id: 's1',
    hook_event_name: 'PostToolUse',
    tool_name: 'AskUserQuestion'
  });

  assert.equal(answered.status, 'running');
  assert.equal(answered.doneReason, 'tool_completed');

  const otherTool = classifyHookEvent({
    session_id: 's1',
    hook_event_name: 'PostToolUse',
    tool_name: 'Bash'
  });

  assert.equal(otherTool.status, 'running');
});

test('maps Stop with completion text to done', () => {
  const result = classifyHookEvent({
    session_id: 's1',
    hook_event_name: 'Stop',
    transcript_path: '',
    last_assistant_message: 'Implemented the fix and tests passed.'
  });

  assert.equal(result.status, 'done');
  assert.equal(result.doneReason, 'assistant_signaled_completion');
});

test('treats a direct factual answer as done', () => {
  const result = classifyHookEvent({
    session_id: 's1',
    hook_event_name: 'Stop',
    last_assistant_message:
      'Singapore National Day is August 9. It commemorates Singapore becoming independent in 1965.'
  });

  assert.equal(result.status, 'done');
  assert.equal(result.doneReason, 'assistant_answered_without_followup');
});

test('treats implemented permission handling and tests passed as done', () => {
  const result = classifyHookEvent({
    session_id: 's1',
    hook_event_name: 'Stop',
    last_assistant_message: 'Implemented permission handling and tests passed.'
  });

  assert.equal(result.status, 'done');
  assert.equal(result.doneReason, 'assistant_signaled_completion');
});

test('maps Stop without completion text to pending_user', () => {
  const result = classifyHookEvent({
    session_id: 's1',
    hook_event_name: 'Stop',
    last_assistant_message: 'Which option do you want me to apply?'
  });

  assert.equal(result.status, 'pending_user');
  assert.equal(result.doneReason, 'assistant_waiting_for_input');
});

test('maps pending notifications to pending_user', () => {
  const result = classifyHookEvent({
    session_id: 's1',
    hook_event_name: 'Notification',
    notification_type: 'permission_prompt'
  });

  assert.equal(result.status, 'pending_user');
  assert.equal(result.doneReason, 'notification_pending_user');
});

test('maps Stop without assistant text to pending_user', () => {
  const result = classifyHookEvent({
    session_id: 's1',
    hook_event_name: 'Stop'
  });

  assert.equal(result.status, 'pending_user');
  assert.equal(result.doneReason, 'missing_assistant_text');
});

test('does not treat negated completion text as done', () => {
  const notCompleted = classifyHookEvent({
    session_id: 's1',
    hook_event_name: 'Stop',
    last_assistant_message: 'This is not completed yet.'
  });

  const unfinished = classifyHookEvent({
    session_id: 's1',
    hook_event_name: 'Stop',
    last_assistant_message: 'The task is unfinished.'
  });

  assert.equal(notCompleted.status, 'pending_user');
  assert.notEqual(notCompleted.status, 'done');
  assert.equal(unfinished.status, 'pending_user');
  assert.notEqual(unfinished.status, 'done');
});
