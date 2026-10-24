export interface Message {
  sequence: number;
  id: string;
  speaker: 'user' | 'codex' | 'antigravity' | 'opencode' | 'system';
  content: string;
  createdAt: string;
  delivery: string;
  version: number;
  isError: boolean;
  channel: 'main' | 'collaboration' | 'activity' | 'legacy' | 'system';
  kind: string;
  roomId: string;
  threadId: string | null;
  turnId: string | null;
  collaborationId: string | null;
  chatSessionId: string;
}
export interface Agent { id: string; status: string; detail: string; updatedAt: string }
export interface AgentProbe { id: string; connection: string; installation: 'installed' | 'missing' | 'unknown' | 'error'; authentication: 'authenticated' | 'required' | 'unknown' | 'error'; availability: 'ready' | 'unavailable' | 'unknown' | 'error'; checkedAt: string; failureReason: string | null }
export interface AgentCandidate { provider: string; name: string; installation: string; path: string | null; failureReason: string | null }
export interface AgentInstance { id: string; provider: string; name: string; path: string | null; connection: string; role: string; activeRole: string | null; builtin: boolean; support: string; installation: string; authentication: string; availability: string; execution: string; checkedAt: string; failureReason: string | null }
export interface Room { id: string; name: string; createdAt: string }
export interface Session { id: string; sessionId: string; createdAt: string; roomId: string; name: string; threadId: string | null; sourceThreadId: string | null; mode: 'new' | 'copy' | 'resume' | 'legacy'; model: string | null; reasoningEffort: string | null; provider: 'codex' | 'opencode'; agentInstanceId: string | null }
export interface ExternalThread { id: string; name?: string; preview?: string; cwd?: string; source?: string | Record<string, unknown>; threadSource?: string | null; agentRole?: string | null; parentThreadId?: string | null; status?: { type: string } }
export interface ActiveRun { id: string; sessionId: string; cancelling: boolean }
export interface Snapshot { contexts: Context[]; activeRun: ActiveRun | null; messages: Message[]; agents: Agent[]; conversationVersion: number; revision: number; sessions: Session[]; rooms: Room[]; busySessionId: string | null }
export interface Tokens { inputTokens: number; outputTokens: number; totalTokens: number; cachedInputTokens: number; reasoningOutputTokens: number }
export interface Context { sessionId: string; threadId: string; usage: { last: Tokens; total: Tokens; modelContextWindow: number | null } | null; usageUpdatedAt: string | null; usageStale: boolean; compaction: string; compactionItemId: string | null; compactionCount: number; compactedAt: string | null }

export interface CodexModel { model: string; displayName: string; defaultReasoningEffort: string; supportedReasoningEfforts: { reasoningEffort: string; description: string }[]; isDefault: boolean; hidden: boolean }
