// Session provider decisions shared by the composer, the message list and the
// session dialog.
//
// The screen used to decide the message target with `isOpenCode ? 'opencode' :
// 'codex'`, so a session whose provider had no branch of its own sent `codex`
// and the backend refused the turn. These helpers are the single place that
// maps a session provider to a turn target and to the UI it owns, so a new Lead
// provider is one entry here instead of another boolean at every call site.

/** Targets the backend's `turn_route` accepts. */
export type TurnTarget = 'codex' | 'opencode' | 'claude_code';

const labels: Record<string, string> = {
  codex: 'Codex',
  opencode: 'OpenCode',
  claude_code: 'Claude Code',
  antigravity: 'Antigravity',
  gemini_cli: 'Gemini CLI',
};

/** Display name for a provider id. Unknown ids are shown as they are. */
export function providerLabel(provider: string | null | undefined): string {
  if (!provider) return 'Codex';
  return labels[provider] ?? provider;
}

/**
 * Target this provider's session may send to, or `null` when ESH has no branch
 * for it. Returning `null` instead of guessing `codex` keeps the screen from
 * offering a turn the backend would refuse, and keeps a provider that gains a
 * branch from inheriting one it was never wired for.
 */
export function turnTarget(provider: string | null | undefined): TurnTarget | null {
  if (provider === 'codex') return 'codex';
  if (provider === 'opencode') return 'opencode';
  if (provider === 'claude_code') return 'claude_code';
  return null;
}

/** Providers ESH can create a chat session for, i.e. the CLI Lead adapters. */
export function isCliLeadProvider(provider: string | null | undefined): boolean {
  return provider === 'opencode' || provider === 'claude_code';
}

/** Manual compaction is a Codex thread operation. */
export function supportsManualCompaction(provider: string | null | undefined): boolean {
  return provider === 'codex';
}

/** Model and reasoning-effort settings come from the Codex model catalog. */
export function supportsCodexModelSettings(provider: string | null | undefined): boolean {
  return provider === 'codex';
}
