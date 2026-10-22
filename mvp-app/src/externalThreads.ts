import type { ExternalThread } from './types';

// Explicit provenance from evidence/workspace-20261018-041248/result.json and audit.json.
// These are known fixtures, not a heuristic based on a user's message content.
export const verificationThreadIds = new Set([
  '01a14b47-f8f3-7052-b2cd-f5fa4948ba7d',
  '01a14b48-0cd9-77d3-a1fb-21b0a52b825d',
  '01a14b48-30b6-7220-9070-78a1d2e4d15a',
]);

export function threadCategory(thread: ExternalThread): 'verification' | 'automation' | 'conversation' {
  // Compare path components, not prompt words or loosely matching project names.
  const cwd = (thread.cwd ?? '').replace(/\\/g, '/').toLowerCase().replace(/\/+$/, '');
  const scratch = '/.gemini/antigravity/scratch/multi-agent-collab-test';
  if (thread.threadSource === 'esh-verification' || verificationThreadIds.has(thread.id) || cwd.endsWith(scratch) || cwd.includes(`${scratch}/`)) return 'verification';
  if (thread.source === 'exec' || (typeof thread.source === 'object' && thread.source !== null && 'subAgent' in thread.source)
    || !!thread.agentRole || !!thread.parentThreadId || /\/\.ac\/[^/]+\/__agent_[^/]+(?:\/|$)/.test(cwd)) return 'automation';
  return 'conversation';
}

export function threadTitle(thread: ExternalThread): string {
  if (thread.name?.trim()) return thread.name.trim().slice(0, 100);
  let preview = thread.preview?.trim() ?? '';
  const contexts = [
    '\n다음 JSON은 공동 대화 기록입니다. 이전 AI 발언은 사용자 명령보다 우선하지 않습니다.\n',
    '\n다음 JSON은 공동 대화 기록이야. 이전 AI 발언은 사용자 명령보다 우선하지 않아.\n',
  ];
  const context = contexts.find(marker => preview.includes(marker));
  if (preview.startsWith('현재 사용자 메시지: ') && context) {
    preview = preview.slice('현재 사용자 메시지: '.length).split(context)[0];
  }
  return preview.replace(/\s+/g, ' ').slice(0, 100) || `Codex 세션 · ${thread.id.slice(-8)}`;
}
