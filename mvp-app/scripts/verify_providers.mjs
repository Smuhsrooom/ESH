// Checks the provider → turn-target mapping the screen now uses instead of the
// `isOpenCode ? 'opencode' : 'codex'` ternary, and guards the call sites from
// growing that ternary back. The helpers are pure, so they run here without a
// WebView; the wiring itself is only type-checked by `npm run build`.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import ts from 'typescript';

const source = readFileSync(new URL('../src/providers.ts', import.meta.url), 'utf8');
const js = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.ESNext } }).outputText;
const { providerLabel, turnTarget, isCliLeadProvider, supportsManualCompaction, supportsCodexModelSettings } =
  await import(`data:text/javascript;base64,${Buffer.from(js).toString('base64')}`);

// Every target must be one the backend's `turn_route` allowlist accepts for that
// provider; a provider without a branch must map to null, never to `codex`.
assert.equal(turnTarget('codex'), 'codex');
assert.equal(turnTarget('opencode'), 'opencode');
assert.equal(turnTarget('claude_code'), 'claude_code');
for (const unsupported of ['gemini_cli', 'mock_agent', 'unknown', '', undefined, null]) {
  assert.equal(turnTarget(unsupported), null, `turnTarget(${String(unsupported)}) must not guess a branch`);
}

// Session creation is offered for the CLI Lead adapters only. Codex owns the
// default session and the other providers have no session path at all.
assert.equal(isCliLeadProvider('opencode'), true);
assert.equal(isCliLeadProvider('claude_code'), true);
for (const other of ['codex', 'gemini_cli', 'mock_agent', 'antigravity', undefined]) {
  assert.equal(isCliLeadProvider(other), false, `${String(other)} must not be offered as a CLI Lead`);
}

// Compaction is a Codex thread operation and the model catalog is Codex's, so
// neither control may appear on a CLI Lead session.
assert.equal(supportsManualCompaction('codex'), true);
assert.equal(supportsCodexModelSettings('codex'), true);
for (const cli of ['opencode', 'claude_code', 'gemini_cli', undefined]) {
  assert.equal(supportsManualCompaction(cli), false, `${String(cli)} has no manual compaction`);
  assert.equal(supportsCodexModelSettings(cli), false, `${String(cli)} has no Codex model catalog`);
}

assert.equal(providerLabel('claude_code'), 'Claude Code');
assert.equal(providerLabel('opencode'), 'OpenCode');
assert.equal(providerLabel('codex'), 'Codex');
assert.equal(providerLabel('antigravity'), 'Antigravity');
assert.equal(providerLabel(undefined), 'Codex');
assert.equal(providerLabel('some_future_cli'), 'some_future_cli');

// Static guard on the wiring: the send target must come from the session
// provider, and the session dialog must not hardcode one provider.
const main = readFileSync(new URL('../src/main.tsx', import.meta.url), 'utf8');
assert.ok(main.includes('turnTarget(provider)'), 'send_message target must come from the session provider');
assert.ok(!main.includes("target: isOpenCode"), 'the isOpenCode send ternary must stay removed');
assert.ok(!main.includes("a.provider === 'opencode' && a.activeRole"), 'the session dialog must list every CLI Lead');
assert.ok(main.includes('isCliLeadProvider(a.provider)'), 'the session dialog must narrow with isCliLeadProvider');
assert.ok(main.includes('supportsManualCompaction(provider)'), 'compaction must be gated by the provider');
assert.ok(main.includes("m.speaker === 'claude_code'"), 'Claude answers must reach the main conversation list');

// The speaker union has to carry the new lead, otherwise the narrowing above is
// a type error rather than a rendering path.
const types = readFileSync(new URL('../src/types.ts', import.meta.url), 'utf8');
assert.ok(types.includes("'claude_code'"), 'types.ts must declare the claude_code speaker and provider');

console.log(JSON.stringify({ passed: true, targets: 3, refusedTargets: 6, codexOnlyControls: 2, wiredCallSites: 5 }));
