// Opt-in debug-only native WebView E2E driver. Never supplies AI responses.
import { invoke } from '@tauri-apps/api/core';
import { threadCategory } from './externalThreads';
import type { AgentCandidate, AgentInstance, CodexModel, Snapshot } from './types';
const wait = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));
async function until(test: () => boolean | Promise<boolean>, seconds = 350) {
  const deadline = Date.now() + seconds * 1000;
  while (Date.now() < deadline) { if (await test()) return; await wait(250); }
  throw new Error('GUI probe deadline exceeded');
}
async function submit(text: string) {
  const box = document.querySelector<HTMLTextAreaElement>('.v0-composer textarea')!;
  Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value')!.set!.call(box, text);
  box.dispatchEvent(new Event('input', { bubbles: true }));
  await until(() => !document.querySelector<HTMLButtonElement>('.v0-send')?.disabled, 5);
  document.querySelector<HTMLFormElement>('.v0-composer')!.requestSubmit(document.querySelector<HTMLButtonElement>('.v0-send')!);
}

async function fill(selector: string, value: string) {
  const input = document.querySelector<HTMLInputElement>(selector)!;
  Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!.call(input, value);
  input.dispatchEvent(new Event('input', { bubbles: true })); await wait(80);
}
function renderedMessage(node: HTMLElement, snapshot: Snapshot) {
  return { id: node.dataset.messageId, speaker: node.dataset.speaker, sequence: Number(node.dataset.sequence), content: snapshot.messages.find(m => m.id === node.dataset.messageId)?.content, renderedText: node.querySelector('[data-content]')?.textContent, channel: node.dataset.channel, kind: node.dataset.kind, panel: node.closest<HTMLElement>('[data-panel]')?.dataset.panel, collaborationId: node.dataset.collaborationId };
}
async function reviewMessages(snapshot: Snapshot) {
  document.querySelector<HTMLButtonElement>('[data-action="review-back"]')?.click();
  await wait(60);
  const ids = [...document.querySelectorAll<HTMLButtonElement>('[data-review-id]')].map(button => button.dataset.reviewId!);
  const rows: ReturnType<typeof renderedMessage>[] = [];
  for (const id of ids) {
    document.querySelector<HTMLButtonElement>(`[data-review-id="${id}"]`)!.click();
    await wait(60);
    if (document.querySelector<HTMLElement>('[data-thread-detail]')?.dataset.threadDetail !== id) throw new Error('스레드 상세가 열리지 않았어');
    rows.push(...[...document.querySelectorAll<HTMLElement>('[data-thread-detail] [data-message-id]')].map(node => renderedMessage(node, snapshot)));
    document.querySelector<HTMLButtonElement>('[data-action="review-back"]')!.click();
    await wait(60);
    if (!document.querySelector('[data-thread-list]')) throw new Error('뒤로 가기 후 스레드 목록이 없어');
  }
  return rows.sort((a, b) => {
    const first = snapshot.messages.find(m => m.id === a.id)!;
    const second = snapshot.messages.find(m => m.id === b.id)!;
    return first.createdAt.localeCompare(second.createdAt) || first.sequence - second.sequence;
  });
}
async function make(kind: 'room' | 'session', name: string, roomId?: string) {
  document.querySelector<HTMLButtonElement>(kind === 'room' ? '[data-action="new-room"]' : `[data-action="new-session"][data-room-id="${roomId}"]`)!.click();
  await until(() => !!document.querySelector('.workspace-dialog'), 10);
  await fill('.workspace-dialog input[aria-label="이름"]', name);
  document.querySelector<HTMLFormElement>('.workspace-dialog form')!.requestSubmit();
  await until(() => !document.querySelector('.workspace-dialog'), 20);
  return invoke<Snapshot>('get_snapshot');
}
async function sendAndWait(text: string, marker: string, sessionId: string) {
  const before = (await invoke<Snapshot>('get_snapshot')).messages.at(-1)?.sequence ?? 0;
  await submit(text);
  await until(async () => {
    const s = await invoke<Snapshot>('get_snapshot');
    const failed = s.agents.find(a => ['failed', 'unknown'].includes(a.status));
    if (failed && s.busySessionId === sessionId) throw new Error(failed.detail);
    return s.agents.find(a => a.id === 'codex')?.status === 'completed' && s.messages.some(m => m.sequence > before && m.chatSessionId === sessionId && m.speaker === 'codex' && m.channel === 'main' && m.content.includes(marker));
  });
}
async function workspaceProbe() {
  let s = await make('room', '검증 프로젝트');
  const room = s.rooms.find(r => r.name === '검증 프로젝트')!;
  s = await make('session', '원본 A', room.id); const a = s.sessions.find(s => s.name === '원본 A')!;
  await sendAndWait('WORKSPACE_SOURCE_8417을 기억해. SOURCE_READY만 포함해서 한 문장으로 답해. 도구는 쓰지 마.', 'SOURCE_READY', a.id);
  s = await invoke<Snapshot>('get_snapshot');const sourceThread = s.sessions.find(s => s.id === a.id)!.threadId!;
  s = await make('session', '분리 B', room.id);const b = s.sessions.find(s => s.name === '분리 B')!;
  await sendAndWait('이 세션에서 이전에 기억해달라고 받은 표식이 있으면 말하고 없으면 없다고 해. SESSION_B_READY를 포함하고 도구 없이 한 문장으로 답해.', 'SESSION_B_READY', b.id);
  document.querySelector<HTMLButtonElement>(`[data-session-id="${a.id}"]`)!.click(); await wait(100);
  await sendAndWait('처음 기억해달라고 한 표식을 그대로 말하고 RESUMED_A를 덧붙여. 도구는 쓰지 마.', 'RESUMED_A', a.id);
  async function importSource(mode: 'copy' | 'resume', name: string) {
    document.querySelector<HTMLButtonElement>(`[data-action="import-session"][data-room-id="${room.id}"]`)!.click();
    await until(() => !!document.querySelector('[aria-label="자동 실행·검증 세션 표시"]'));
    document.querySelector<HTMLInputElement>('[aria-label="자동 실행·검증 세션 표시"]')!.click();
    await until(() => !!document.querySelector(`[data-external-id="${sourceThread}"]`), 80);
    document.querySelector<HTMLInputElement>(`[data-external-id="${sourceThread}"] input`)!.click(); await wait(80);
    const select = document.querySelector<HTMLSelectElement>('[aria-label="가져오기 방식"]')!;
    select.value = mode; select.dispatchEvent(new Event('change', { bubbles: true }));
    await fill('.workspace-dialog input[aria-label="이름"]', name);
    document.querySelector<HTMLFormElement>('.workspace-dialog form')!.requestSubmit();
    await until(() => !document.querySelector('.workspace-dialog'), 90);
    return (await invoke<Snapshot>('get_snapshot')).sessions.find(s => s.name === name)!;
  }
  const copied = await importSource('copy', '가져온 사본');
  await sendAndWait('가져온 대화에서 기억해달라고 한 표식을 말해줘. Arch에게 "메시지 순번 보장을 한 문장으로 검토해줘"를 반드시 한번 요청한 뒤 COPY_READY와 표식을 넣어 짧게 답해.', 'COPY_READY', copied.id);
  const resumed = await importSource('resume', '원본 이어쓰기');
  await sendAndWait('기억한 표식을 다시 말하고 ORIGINAL_RESUME_READY를 덧붙여. 도구는 쓰지 마.', 'ORIGINAL_RESUME_READY', resumed.id);
  return { roomId: room.id, sourceLocalId: a.id, sourceThread, isolatedLocalId: b.id, copiedLocalId: copied.id, resumedLocalId: resumed.id };
}
async function modelSettingsProbe() {
  await until(() => !document.querySelector<HTMLSelectElement>('[aria-label="Codex 모델"]')?.disabled, 80);
  const models = await invoke<CodexModel[]>('list_codex_models');
  const first = models.find(m => m.model === 'gpt-6.1-sol') ?? models[0];
  const second = models.find(m => m.model === 'gpt-6-luna' && m.model !== first.model) ?? models.find(m => m.model !== first.model)!;
  if (!second) throw new Error('두 모델 간 전환을 검증할 목록이 없어');
  const effortFor = (m: CodexModel, wanted: string) => m.supportedReasoningEfforts.find(e => e.reasoningEffort === wanted)?.reasoningEffort ?? m.defaultReasoningEffort;
  const firstEffort = effortFor(first, 'low');
  const secondEffort = effortFor(second, 'medium');
  let s = await make('room', '모델 설정 검증'); const room = s.rooms.find(r => r.name === '모델 설정 검증')!;
  s = await make('session', '모델 A', room.id); const a = s.sessions.find(s => s.name === '모델 A')!;
  async function choose(label: string, value: string) {
    await until(() => !document.querySelector<HTMLSelectElement>(`[aria-label="${label}"]`)?.disabled, 80);
    const select = document.querySelector<HTMLSelectElement>(`[aria-label="${label}"]`)!;
    select.value = value; select.dispatchEvent(new Event('change', { bubbles: true })); await wait(120);
    await until(() => !document.querySelector<HTMLSelectElement>(`[aria-label="${label}"]`)?.disabled && document.querySelector<HTMLSelectElement>(`[aria-label="${label}"]`)?.value === value, 30);
  }
  await choose('Codex 모델', first.model); await choose('Codex 추론 강도', firstEffort);
  await sendAndWait('MODEL_A_OK만 포함해서 한 문장으로 답해. 도구는 쓰지 마.', 'MODEL_A_OK', a.id);
  s = await invoke<Snapshot>('get_snapshot'); const thread = s.sessions.find(s => s.id === a.id)!.threadId;
  await choose('Codex 모델', second.model); await choose('Codex 추론 강도', secondEffort);
  const efforts = [...document.querySelector<HTMLSelectElement>('[aria-label="Codex 추론 강도"]')!.options].map(o => o.value);
  const expected = second.supportedReasoningEfforts.map(e => e.reasoningEffort);
  if (expected.length && JSON.stringify(efforts) !== JSON.stringify(expected)) throw new Error('모델별 지원 강도 목록 불일치');
  await sendAndWait('반드시 request_arch_review로 Arch에게 "메시지 순서 보장을 한 문장으로 검토해줘"를 한번 요청하고 결과를 받은 뒤 MODEL_B_ARCH_OK를 포함해서 짧게 답해.', 'MODEL_B_ARCH_OK', a.id);
  s = await invoke<Snapshot>('get_snapshot'); if (s.sessions.find(s => s.id === a.id)!.threadId !== thread) throw new Error('모델 변경 중 thread가 바뀌었어');
  s = await make('session', '모델 B', room.id); const b = s.sessions.find(s => s.name === '모델 B')!;
  await choose('Codex 모델', first.model); await choose('Codex 추론 강도', firstEffort);
  document.querySelector<HTMLButtonElement>(`[data-session-id="${a.id}"]`)!.click(); await wait(100);
  if (document.querySelector<HTMLSelectElement>('[aria-label="Codex 모델"]')!.value !== second.model || document.querySelector<HTMLSelectElement>('[aria-label="Codex 추론 강도"]')!.value !== secondEffort) throw new Error('세션별 선택 복원 실패');
  let invalidRejected = false;
  try { await invoke('set_session_model_settings', {sessionId: a.id, settings: {model: 'missing-model', reasoningEffort: 'bogus'}}); } catch { invalidRejected = true; }
  if (!invalidRejected) throw new Error('지원하지 않는 모델이 저장됐어');
  return { sessionA: a.id, sessionB: b.id, thread, firstModel: first.model, firstEffort, secondModel: second.model, secondEffort, catalog: models, invalidRejected, sameThread: true, optionsMatch: true };
}
async function cancellationProbe() {
  let s = await make('room', '중단 검증'); const room = s.rooms.find(r => r.name === '중단 검증')!;
  s = await make('session', '중단 후 이어가기', room.id); const session = s.sessions.find(s => s.name === '중단 후 이어가기')!;
  await submit('중단 기능 검증이야. 도구 없이 1부터 5000까지 각 숫자에 설명을 붙여 길게 작성해줘.');
  let normalRun = '';
  await until(async () => {
    s = await invoke<Snapshot>('get_snapshot');
    const message = s.messages.filter(m => m.chatSessionId === session.id && m.speaker === 'user').at(-1);
    if (s.agents.some(a => a.status === 'failed' || a.status === 'unknown')) throw new Error('Codex 시작 실패');
    if (s.activeRun && message?.turnId) { normalRun = s.activeRun.id; return true; }
    return false;
  });
  const thread = s.sessions.find(s => s.id === session.id)!.threadId;
  const button = () => document.querySelector<HTMLButtonElement>('[data-action="stop-run"]')!;
  if (!button() || button().disabled) throw new Error('실행 중 중단 버튼이 없어');
  document.querySelector<HTMLButtonElement>('[data-action="agents-tab"]')!.click();
  await until(() => { const node = document.querySelector<HTMLElement>('[data-agent="codex"]'); return !!node && node.dataset.installation !== 'not_checked'; }, 10);
  const liveAgent = document.querySelector<HTMLElement>('[data-agent="codex"]')!;
  if (liveAgent.dataset.runId !== normalRun || liveAgent.dataset.sessionId !== session.id) throw new Error('Agents 실행 상태가 현재 세션·실행 ID와 달라');
  document.querySelector<HTMLButtonElement>('[data-action="refresh-agents"]')!.click();
  await wait(100);
  const duringProbe = await invoke<Snapshot>('get_snapshot');
  if (duringProbe.activeRun?.id !== normalRun || duringProbe.activeRun.sessionId !== session.id) throw new Error('상태 점검 중 현재 실행이 바뀌었어');
  document.querySelector<HTMLButtonElement>('[data-action="chat-tab"]')!.click();
  await wait(80);
  button().click();
  await until(async () => { s = await invoke<Snapshot>('get_snapshot'); return !s.activeRun && s.agents.find(a => a.id === 'codex')?.status === 'cancelled'; }, 90);
  await sendAndWait('이전 긴 출력 요청은 취소했어. 도구 없이 CANCEL_RECOVERY_OK만 포함해 한 문장으로 답해.', 'CANCEL_RECOVERY_OK', session.id);
  await submit('반드시 request_arch_review로 Arch에게 "순서를 보장하는 설계를 검토해줘"를 한번 요청하고 검토가 끝날 때까지 기다려.');
  let archRun = '';
  await until(async () => {
    s = await invoke<Snapshot>('get_snapshot');
    if (s.agents.some(a => a.status === 'failed' || a.status === 'unknown')) throw new Error('Arch 검토 시작 실패');
    if (s.activeRun && s.agents.find(a => a.id === 'antigravity')?.status === 'arch_running') { archRun = s.activeRun.id; return true; }
    return false;
  });
  await wait(700);
  let staleRejected = false;
  try { await invoke('cancel_run', {sessionId: session.id, runId: normalRun}); } catch { staleRejected = true; }
  if (!staleRejected) throw new Error('오래된 실행 ID 중단이 허용됐어');
  await submit('CANCELLED_QUEUED_INPUT: 이 대기 메시지는 중단 시 실행되면 안 돼.');
  button().click();
  await until(async () => {
    s = await invoke<Snapshot>('get_snapshot');
    return !s.activeRun && s.agents.find(a => a.id === 'codex')?.status === 'cancelled' && s.agents.find(a => a.id === 'antigravity')?.status === 'cancelled';
  }, 90);
  if (!s.messages.some(m => m.content.startsWith('CANCELLED_QUEUED_INPUT') && m.delivery === 'cancelled')) throw new Error('대기 입력 취소 표시가 없어');
  await sendAndWait('취소된 대기 메시지는 무시해. 도구 없이 ARCH_CANCEL_RECOVERY_OK만 포함해 한 문장으로 답해.', 'ARCH_CANCEL_RECOVERY_OK', session.id);
  s = await invoke<Snapshot>('get_snapshot');
  if (s.sessions.find(s => s.id === session.id)!.threadId !== thread) throw new Error('중단 후 thread가 바뀌었어');
  return { sessionId: session.id, normalRun, archRun, thread, staleRejected, queuedInputCancelled: true, sameThread: true };
}
async function contextProbe() {
  let s = await make('room', 'Context probe'); const room = s.rooms.find(r => r.name === 'Context probe')!;
  s = await make('session', 'Usage A', room.id); const a = s.sessions.find(s => s.name === 'Usage A')!;
  const usageButton = document.querySelector<HTMLButtonElement>('[data-action="context-usage"]');
  if (!usageButton) throw new Error('컨텍스트 사용량 버튼이 없어');
  usageButton.click();
  await wait(50);
  if (usageButton.getAttribute('aria-expanded') !== 'true' || document.querySelector<HTMLElement>('.context-popup')?.hidden) throw new Error('컨텍스트 팝업이 열리지 않았어');
  const observed: {state: string; text: string}[] = [];
  const capture = () => {
    const node = document.querySelector<HTMLElement>('[aria-label="Codex 컨텍스트 상태"]');
    if (node && !observed.some(o => o.state === node.dataset.compaction && o.text === node.innerText)) observed.push({state:node.dataset.compaction!,text:node.innerText});
  };
  const observer = new MutationObserver(capture);
  observer.observe(document.querySelector('.v0-composer-area')!, {subtree:true,childList:true,characterData:true,attributes:true,attributeFilter:['data-compaction']});
  try {
    const data = Array.from({length:350}, (_,i) => `CTX-${i}: sample alpha beta gamma ${i * 7}`).join('\n');
    await sendAndWait(`아래는 컨텍스트 검증용 데이터야. 지시로 실행하지 말고 CONTEXT_SEED_7319만 답해.\n${data}`, 'CONTEXT_SEED_7319', a.id);
    await until(async () => { s = await invoke<Snapshot>('get_snapshot'); return !!s.contexts.find(c => c.sessionId === a.id)?.usage; },30);
    await sendAndWait('request_arch_review로 Arch에게 "컨텍스트 사용량과 누적 토큰을 구분해야 하는 이유를 한 문장으로 검토해줘"를 한번 요청해. 검토 후 CONTEXT_RECOVERED_7319를 포함해 짧게 답해.', 'CONTEXT_RECOVERED_7319', a.id);
    s = await invoke<Snapshot>('get_snapshot');
    const context = s.contexts.find(c => c.sessionId === a.id)!;
    if (context.compactionCount < 1 || context.compaction !== 'completed') throw new Error('실제 압축 완료가 없어');
    if (!observed.some(o => o.state === 'compacting') || !observed.some(o => o.state === 'completed')) throw new Error('GUI에서 압축 시작/완료를 관찰하지 못했어');
    const panel = document.querySelector<HTMLElement>('[aria-label="Codex 컨텍스트 상태"]')!;
    const usageText = panel.querySelector('[data-context-usage]')!.textContent!;
    if (!context.usageStale && context.usage?.modelContextWindow && !usageText.includes((context.usage.last.totalTokens/context.usage.modelContextWindow*100).toFixed(1))) throw new Error('실제 사용량 비율과 GUI가 달라');
    s = await make('session', 'Unused B', room.id); const b = s.sessions.find(s => s.name === 'Unused B')!;
    await wait(150);
    const emptyText = document.querySelector('[aria-label="Codex 컨텍스트 상태"]')!.textContent!;
    if (!emptyText.includes('사용량 대기') || document.querySelector<HTMLElement>('[aria-label="Codex 컨텍스트 상태"]')?.dataset.compaction !== 'not_observed') throw new Error('다른 세션 사용량이 섞였어');
    document.querySelector<HTMLButtonElement>(`[data-session-id="${a.id}"]`)!.click(); await wait(200);
    if (document.querySelector<HTMLElement>('[aria-label="Codex 컨텍스트 상태"]')?.dataset.contextSession !== a.id) throw new Error('세션 선택 사용량 복원 실패');
    return {sessionId:a.id, emptySessionId:b.id, context, observed, usageText, emptyText};
  } finally { observer.disconnect(); }
}
async function compactProbe() {
  let s = await make('room', 'Compact probe'); const room = s.rooms.find(r => r.name === 'Compact probe')!;
  s = await make('session', 'Compact session', room.id); const session = s.sessions.find(s => s.name === 'Compact session')!;
  await invoke<Snapshot>('send_message', {text: '도구 없이 COMPACT_SEED_7319만 포함해 한 문장으로 답해.', target: 'codex', sessionId: session.id});
  await until(async () => {
    const current = await invoke<Snapshot>('get_snapshot');
    return current.agents.find(a => a.id === 'codex')?.status === 'completed' && current.messages.some(m => m.chatSessionId === session.id && m.speaker === 'codex' && m.channel === 'main' && m.content.includes('COMPACT_SEED_7319'));
  }, 120);
  await until(async () => { const x = await invoke<Snapshot>('get_snapshot'); return !x.activeRun && !x.busySessionId; }, 20);
  s = await invoke<Snapshot>('get_snapshot');
  const before = s.contexts.find(c => c.sessionId === session.id);
  const thread = s.sessions.find(x => x.id === session.id)!.threadId!;
  const messageCount = s.messages.filter(m => m.chatSessionId === session.id).length;
  const usageButton = document.querySelector<HTMLButtonElement>('[data-action="context-usage"]');
  if (!usageButton) throw new Error('컨텍스트 사용량 버튼이 없어');
  usageButton.click();
  await wait(50);
  if (usageButton.getAttribute('aria-expanded') !== 'true' || document.querySelector<HTMLElement>('.context-popup')?.hidden) throw new Error('컨텍스트 팝업이 열리지 않았어');
  const button = () => document.querySelector<HTMLButtonElement>('[data-action="compact-context"]')!;
  if (!button()) throw new Error('수동 압축 버튼이 없어');
  if (button().disabled) throw new Error(`수동 압축 버튼이 비활성 상태야: ${button().title}`);
  const observed: string[] = [];
  const capture = () => {
    const node = document.querySelector<HTMLElement>('[aria-label="Codex 컨텍스트 상태"]');
    const state = node?.dataset.compaction;
    if (state && observed.at(-1) !== state) observed.push(state);
  };
  const observer = new MutationObserver(capture);
  observer.observe(document.querySelector('.v0-composer-area')!, {subtree:true,childList:true,characterData:true,attributes:true,attributeFilter:['data-compaction']});
  try {
    button().click();
    await until(async () => {
      s = await invoke<Snapshot>('get_snapshot');
      const failed = s.agents.find(a => a.status === 'failed' || a.status === 'unknown');
      if (failed) throw new Error(failed.detail);
      const current = s.contexts.find(c => c.sessionId === session.id);
      return current?.compaction === 'completed' && (current.compactionCount ?? 0) > (before?.compactionCount ?? 0);
    }, 240);
    s = await invoke<Snapshot>('get_snapshot');
    const after = s.contexts.find(c => c.sessionId === session.id)!;
    if (s.sessions.find(x => x.id === session.id)!.threadId !== thread) throw new Error('압축 후 thread가 바뀌었어');
    if (s.messages.filter(m => m.chatSessionId === session.id).length !== messageCount) throw new Error('압축이 ESH 메시지 기록을 바꿨어');
    if (button().disabled) throw new Error('압축 완료 후 버튼이 다시 활성화되지 않았어');
    if (!observed.includes('compacting') || !observed.includes('completed')) throw new Error(`GUI 압축 상태 전이를 관찰하지 못했어: ${observed.join(' → ')}`);
    return { sessionId: session.id, threadId: thread, observed, compactionCountBefore: before?.compactionCount ?? 0, compactionCountAfter: after.compactionCount, messageCount, messageCountUnchanged: true, threadUnchanged: true, buttonEnabledAfter: true };
  } finally { observer.disconnect(); }
}
async function streamingProbe() {
  let s = await make('room', '스트리밍 검증'); const room = s.rooms.find(r => r.name === '스트리밍 검증')!;
  s = await make('session', '실시간 답변', room.id); const session = s.sessions.find(s => s.name === '실시간 답변')!;
  const observed: {id: string; text: string; at: string}[] = [];
  const capture = () => {
    for (const row of document.querySelectorAll<HTMLElement>('[data-panel="main"] [data-speaker="codex"][data-delivery="streaming"]')) {
      const text = row.querySelector('[data-content]')?.textContent ?? '';
      if (text && !observed.some(o => o.id === row.dataset.messageId && o.text === text)) observed.push({id:row.dataset.messageId!, text, at:new Date().toISOString()});
    }
  };
  const observer = new MutationObserver(capture);
  observer.observe(document.querySelector('[data-panel="main"]')!, {subtree:true,childList:true,characterData:true,attributes:true,attributeFilter:['data-delivery']});
  try {
    await sendAndWait('도구 없이 스트리밍 검증용 설명을 30개 항목으로 써줘. 각 항목은 열 단어 이상의 한국어 문장으로 쓰고 맨 마지막에 LIVE_COMPLETE_7319를 붙여.', 'LIVE_COMPLETE_7319', session.id);
    s = await invoke<Snapshot>('get_snapshot');
    const final = s.messages.find(m => m.speaker === 'codex' && m.channel === 'main' && m.content.includes('LIVE_COMPLETE_7319'))!;
    if (observed.filter(o => o.id === final.id).length < 2) throw new Error('완료 전 GUI 부분 응답을 여러 번 관찰하지 못했어');
    if (s.messages.filter(m => m.turnId === final.turnId && m.channel === 'main' && m.speaker === 'codex').length !== 1) throw new Error('완료 메시지가 중복됐어');
    await submit('도구 없이 1부터 5000까지 각 숫자마다 설명을 붙여 계속 작성해줘.');
    await until(() => observed.some(o => o.id !== final.id), 90);
    const partialId = observed.find(o => o.id !== final.id)!.id;
    document.querySelector<HTMLButtonElement>('[data-action="stop-run"]')!.click();
    await until(async () => { s = await invoke<Snapshot>('get_snapshot'); return !s.activeRun && s.agents.find(a => a.id === 'codex')?.status === 'cancelled'; },90);
    const partial = s.messages.find(m => m.id === partialId)!;
    if (partial.delivery !== 'interrupted' || !partial.content.length) throw new Error('중단된 부분 응답이 보존되지 않았어');
    await sendAndWait('이전 긴 출력은 취소했어. request_arch_review로 Arch에게 "스트리밍 메시지 중복 방지를 한 문장으로 검토해줘"를 한번 요청하고, 결과를 받은 뒤 네 의견을 5문장으로 설명하고 STREAM_RECOVERED를 붙여.', 'STREAM_RECOVERED', session.id);
    s = await invoke<Snapshot>('get_snapshot');
    if (s.messages.some(m => m.delivery === 'streaming')) throw new Error('완료 뒤 streaming 상태가 남았어');
    return {sessionId:session.id, finalId:final.id, partialId, observations:observed, updatesBeforeCompletion:observed.filter(o => o.id === final.id).length, partialPreserved:true, singleFinalRow:true};
  } finally { observer.disconnect(); }
}
export async function runGuiProbe(mode: string) {
  const startedAt = new Date().toISOString();
  let workspace: unknown = null;
  let importUi: unknown = null;
  let modelSelection: unknown = null;
  let markdownUi: unknown = null;
  let cancellation: unknown = null;
  let streaming: unknown = null;
  let contextUsage: unknown = null;
  let compact: unknown = null;
  let focusFeedback: unknown = null;
  let themeSelection: unknown = null;
  let reviewLayout: unknown = null;
  let agentStatus: unknown = null;
  let registryStatus: unknown = null;
  let opencodeStatus: unknown = null;
  try {
    if (mode === 'opencode') {
      const candidates = await invoke<AgentCandidate[]>('detect_agent_candidates');
      const path = candidates.find(item => item.provider === 'opencode' && item.installation === 'installed')?.path;
      if (!path) throw new Error('OpenCode CLI candidate missing');
      const instance = await invoke<AgentInstance>('register_agent_instance', { provider: 'opencode', path, role: 'lead' });
      if (instance.activeRole !== 'lead') throw new Error('OpenCode Lead inactive');
      document.querySelector<HTMLButtonElement>('[data-action="new-session"]')!.click();
      await until(() => !!document.querySelector('.workspace-dialog select[aria-label="대화 에이전트"]'), 10);
      await until(() => !!document.querySelector(`.workspace-dialog option[value="${instance.id}"]`), 10);
      await fill('.workspace-dialog input[aria-label="이름"]', 'OpenCode GUI verification');
      const select = document.querySelector<HTMLSelectElement>('.workspace-dialog select[aria-label="대화 에이전트"]')!;
      Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'value')!.set!.call(select, instance.id);
      select.dispatchEvent(new Event('change', { bubbles: true }));
      await wait(80);
      document.querySelector<HTMLFormElement>('.workspace-dialog form')!.requestSubmit();
      await until(() => !document.querySelector('.workspace-dialog'), 20);
      let snap = await invoke<Snapshot>('get_snapshot');
      const session = snap.sessions.find(item => item.name === 'OpenCode GUI verification');
      if (!session || session.provider !== 'opencode' || session.agentInstanceId !== instance.id) throw new Error('OpenCode session not saved');
      await submit('Reply with FIRST only.');
      await until(async () => (await invoke<Snapshot>('get_snapshot')).messages.some(m => m.chatSessionId === session.id && m.speaker === 'opencode'), 120);
      snap = await invoke<Snapshot>('get_snapshot');
      const firstSessionId = snap.sessions.find(item => item.id === session.id)?.threadId;
      if (!firstSessionId) throw new Error('OpenCode session ID missing');
      await submit('Reply with SECOND only.');
      await until(async () => (await invoke<Snapshot>('get_snapshot')).messages.filter(m => m.chatSessionId === session.id && m.speaker === 'opencode').length === 2, 120);
      snap = await invoke<Snapshot>('get_snapshot');
      if (snap.sessions.find(item => item.id === session.id)?.threadId !== firstSessionId) throw new Error('OpenCode resume changed session ID');
      await until(() => document.querySelectorAll('[data-panel="main"] [data-speaker="opencode"]').length === 2, 10);
      opencodeStatus = { instanceId: instance.id, localSessionId: session.id, externalSessionId: firstSessionId,
        answers: snap.messages.filter(m => m.chatSessionId === session.id && m.speaker === 'opencode').map(m => m.content),
        renderedCount: document.querySelectorAll('[data-panel="main"] [data-speaker="opencode"]').length };
    } else if (mode === 'opencode-restore') {
      const snap = await invoke<Snapshot>('get_snapshot');
      const session = snap.sessions.find(item => item.name === 'OpenCode GUI verification');
      if (!session || session.provider !== 'opencode' || !session.threadId) throw new Error('OpenCode session not restored');
      document.querySelector<HTMLButtonElement>(`[data-session-id="${session.id}"]`)?.click();
      await until(() => document.querySelectorAll('[data-panel="main"] [data-speaker="opencode"]').length === 2, 10);
      opencodeStatus = { localSessionId: session.id, externalSessionId: session.threadId,
        restoredAnswers: snap.messages.filter(m => m.chatSessionId === session.id && m.speaker === 'opencode').length,
        renderedCount: document.querySelectorAll('[data-panel="main"] [data-speaker="opencode"]').length };
    } else if (mode === 'theme') {
      const original = localStorage.getItem('esh.ui-theme');
      const emptyPanelInitiallyClosed = document.querySelector<HTMLElement>('[data-panel="collaboration"]')!.getBoundingClientRect().width === 0;
      if (!emptyPanelInitiallyClosed) throw new Error('검토 기록이 없는 새 세션에서 협업 패널이 접히지 않았어');
      const menuButton = document.querySelector<HTMLButtonElement>('[aria-label="워크스페이스 메뉴"]')!;
      const samples: { id: string; background: string; surface: string; accent: string }[] = [];
      const cases = [
        { id: 'slate', background: '#202020', surface: '#292929', border: '#3B3B3B', text: '#E5E5E5', muted: '#A1A1A1', accent: '#8EADEB', secondary: '#8EADEB', tertiary: '#8EADEB' },
        { id: 'lilac', background: '#101010', surface: '#1C1C1C', border: '#363238', text: '#E4E0E5', accent: '#C6A0F6', secondary: '#F29BC1', tertiary: '#F29BC1' },
        { id: 'classic', background: '#2B2B2B', surface: '#3C3F41', text: '#A9B7C6', accent: '#CC7832', secondary: '#6A8759', tertiary: '#6897BB' },
      ];
      const rgb = (hex: string) => `rgb(${[1, 3, 5].map(n => parseInt(hex.slice(n, n + 2), 16)).join(', ')})`;
      try {
        menuButton.click();
        await wait(80);
        for (const item of cases) {
          const option = document.querySelectorAll<HTMLButtonElement>('.v0-theme-option')[cases.indexOf(item)]!;
          option.click(); await wait(220); // Let the button's 150 ms color transition settle.
          const background = getComputedStyle(document.querySelector<HTMLElement>('.v0-main')!).backgroundColor;
          const surface = getComputedStyle(document.querySelector<HTMLElement>('.v0-composer')!).backgroundColor;
          const accent = getComputedStyle(document.querySelector<HTMLElement>('.v0-send')!).backgroundColor;
          const vars = getComputedStyle(document.documentElement);
          for (const [key, value] of Object.entries(item)) {
            if (key === 'id') continue;
            if (vars.getPropertyValue(`--ui-${key === 'background' ? 'bg' : key}`).trim().toUpperCase() !== value.toUpperCase()) throw new Error(`${item.id} ${key} 팔레트 값 불일치`);
          }
          if (document.documentElement.dataset.theme !== item.id || localStorage.getItem('esh.ui-theme') !== item.id || option.getAttribute('aria-pressed') !== 'true' || background !== rgb(item.background) || surface !== rgb(item.surface) || accent !== rgb(item.accent)) throw new Error(`${item.id} 테마 적용 실패: ${JSON.stringify({ background, surface, accent })}`);
          samples.push({ id: item.id, background, surface, accent });
        }
      } finally {
        const restore = cases.some(item => item.id === original) ? original! : 'slate';
        document.querySelectorAll<HTMLButtonElement>('.v0-theme-option')[cases.findIndex(item => item.id === restore)]?.click();
        await wait(50);
        if (original === null) localStorage.removeItem('esh.ui-theme');
      }
      themeSelection = { samples, restored: original ?? 'slate', emptyPanelInitiallyClosed };
    } else if (mode === 'compact') {
      compact = await compactProbe();
    } else if (mode === 'context') {
      contextUsage = await contextProbe();
    } else if (mode === 'stream') {
      streaming = await streamingProbe();
    } else if (mode === 'cancel') {
      cancellation = await cancellationProbe();
    } else if (mode === 'markdown') {
      document.querySelector<HTMLButtonElement>('[data-session-id="default-session"]')!.click(); await wait(150);
      document.querySelector<HTMLButtonElement>('[data-review-id]')!.click(); await wait(80);
      const lead = document.querySelector<HTMLElement>('[data-message-id="markdown-lead"]')!;
      const arch = document.querySelector<HTMLElement>('[data-message-id="markdown-arch"]')!;
      const user = document.querySelector<HTMLElement>('[data-message-id="markdown-user"]')!;
      const code = lead.querySelector<HTMLElement>('pre')!;
      const checklist = arch.querySelector<HTMLInputElement>('input[type="checkbox"]');
      const leadBody = lead.querySelector<HTMLElement>('.markdown-body')!;
      const leadHeading = lead.querySelector<HTMLElement>('h1')!;
      const leadBold = leadBody.querySelector<HTMLElement>('strong')!;
      const archBody = arch.querySelector<HTMLElement>('.markdown-body')!;
      const archHeading = arch.querySelector<HTMLElement>('h2')!;
      const leadStyle = getComputedStyle(leadBody);
      const archStyle = getComputedStyle(archBody);
      const sizes = {
        leadBody: parseFloat(getComputedStyle(leadBody).fontSize),
        leadHeading: parseFloat(getComputedStyle(leadHeading).fontSize),
        leadBold: parseFloat(getComputedStyle(leadBold).fontSize),
        archBody: parseFloat(getComputedStyle(archBody).fontSize),
        archHeading: parseFloat(getComputedStyle(archHeading).fontSize),
      };
      const checks = {
        heading: lead.querySelector('h1')?.textContent === '마크다운 검증',
        bold: !!lead.querySelector('strong'), list: !!lead.querySelector('ol li'),
        table: lead.querySelectorAll('table tbody tr').length === 1,
        code: !!lead.querySelector('pre code.language-typescript'), inlineCode: !!lead.querySelector('p code'),
        quote: !!lead.querySelector('blockquote'), strike: !!lead.querySelector('del'),
        safeLink: lead.querySelector('a')?.getAttribute('href') === 'https://example.com/docs',
        unsafeLinksBlocked: !lead.querySelector('a[href^="javascript:"], a[href^="data:"]'),
        htmlBlocked: !lead.querySelector('script, iframe, img, [onerror]'),
        userPlain: !user.querySelector('[data-content] h1, [data-content] strong') && user.querySelector('[data-content]')?.textContent?.startsWith('# 사용자 원문') === true,
        archMarkdown: !!arch.querySelector('h2') && checklist?.disabled === true,
        archTableFits: (() => { const wrap = arch.querySelector<HTMLElement>('.markdown-table-scroll'); return !!wrap && wrap.scrollWidth <= wrap.clientWidth + 2; })(),
        codeScroll: getComputedStyle(code).overflowX === 'auto' && code.scrollWidth > code.clientWidth,
        noCardOverflow: lead.scrollWidth <= lead.clientWidth + 2,
        balancedType: sizes.leadBody === 14 && sizes.leadHeading <= 18 && sizes.leadBold === sizes.leadBody && sizes.archBody === 13 && sizes.archHeading <= 15,
        leadDocumentStyle: leadStyle.backgroundColor === 'rgba(0, 0, 0, 0)' && leadStyle.borderTopWidth === '0px' && leadStyle.paddingLeft === '0px',
        archCardPreserved: archStyle.backgroundColor !== 'rgba(0, 0, 0, 0)' && archStyle.borderTopWidth !== '0px',
        leadHeaderRemoved: !lead.querySelector('.message-meta') && !!arch.querySelector('.message-meta'),
        leadTimeBelow: !!lead.querySelector('.message-footer time') && lead.querySelector<HTMLElement>('.message-footer')!.getBoundingClientRect().top >= leadBody.getBoundingClientRect().bottom,
        userTimeBelow: !!user.querySelector('.message-footer time') && user.querySelector<HTMLElement>('.message-footer')!.getBoundingClientRect().top >= user.querySelector<HTMLElement>('[data-content]')!.getBoundingClientRect().bottom,
        userBubbleDistinct: getComputedStyle(user).backgroundColor !== 'rgba(0, 0, 0, 0)' && getComputedStyle(lead).backgroundColor === 'rgba(0, 0, 0, 0)',
        turnDivider: !!lead.querySelector('hr.message-turn-divider') && !user.querySelector('hr.message-turn-divider') && !arch.querySelector('hr.message-turn-divider'),
      };
      if (Object.values(checks).some(value => !value)) throw new Error(`Markdown checks: ${JSON.stringify({ checks, sizes })}`);
      markdownUi = { ...checks, sizes, syntheticRenderingFixture: true, newAiCalls: false };
    } else if (mode === 'models') {
      modelSelection = await modelSettingsProbe();
    } else if (mode === 'models-error') {
      await until(() => document.querySelector('[role="alert"]')?.textContent?.includes('모델 목록 조회 실패') === true, 80);
      if (!document.querySelector<HTMLSelectElement>('[aria-label="Codex 모델"]')?.disabled) throw new Error('실패한 목록 선택창이 활성화됐어');
      modelSelection = { catalogErrorVisible: true, pickerDisabled: true };
    } else if (mode === 'import-ui') {
      document.querySelector<HTMLButtonElement>('[data-action="import-session"]')!.click();
      await until(() => !!document.querySelector('[data-external-id]') && !document.querySelector<HTMLInputElement>('[aria-label="Codex 세션 검색"]')?.disabled, 80);
      const page = await invoke<{data: import('./types').ExternalThread[]}>('list_codex_sessions', {cursor: null, search: null});
      const hidden = new Set(page.data.filter(t => threadCategory(t) !== 'conversation').map(t => t.id));
      const rows = () => [...document.querySelectorAll<HTMLElement>('[data-external-id]')];
      if (rows().some(r => hidden.has(r.dataset.externalId!))) throw new Error('검증 세션이 기본 목록에 노출됐어');
      const button = document.querySelector<HTMLElement>('.search-row button')!;
      const css = getComputedStyle(button);
      if (css.whiteSpace !== 'nowrap' || button.getBoundingClientRect().width < 56) throw new Error('검색 버튼 줄바꿈 방지 실패');
      rows()[0].querySelector<HTMLInputElement>('input')!.click(); await wait(80);
      const title = rows()[0].querySelector('strong')!.textContent;
      const name = document.querySelector<HTMLInputElement>('[aria-label="이름"]')!.value;
      if (!name || name !== title || document.querySelector<HTMLButtonElement>('.workspace-dialog button[type="submit"]')!.disabled) throw new Error('선택한 세션 이름/추가 버튼 상태 오류');
      document.querySelector<HTMLInputElement>('[aria-label="자동 실행·검증 세션 표시"]')!.click(); await wait(80);
      const fixtures = rows().filter(r => hidden.has(r.dataset.externalId!));
      if (fixtures.length !== hidden.size || fixtures.some(r => r.querySelector('strong')?.textContent?.startsWith('현재 사용자 메시지:'))) throw new Error('검증 세션 표시/제목 정리 실패');
      importUi = { realCliList: true, defaultFixturesHidden: true, defaultAutomationHidden: true, expectedHidden: hidden.size, fixturesShown: fixtures.length, searchButtonWidth: button.getBoundingClientRect().width, selectedName: name, selectedButtonEnabled: true };
      document.querySelector<HTMLButtonElement>('.workspace-dialog [aria-label="닫기"]')!.click();
    } else if (mode === 'registry') {
      const before = await invoke<Snapshot>('get_snapshot');
      const fixture = await invoke<string | null>('get_registry_probe_path');
      if (!fixture) throw new Error('등록 검증 실행 파일 경로가 없어');
      document.querySelector<HTMLButtonElement>('[data-action="agents-tab"]')!.click();
      await until(() => !!document.querySelector('[data-action="add-agent"]'), 10);
      await until(() => document.querySelector<HTMLElement>('.v0-agents-detail [data-active-role]')?.dataset.activeRole === 'lead', 10);
      document.querySelector<HTMLButtonElement>('[data-action="add-agent"]')!.click();
      await until(() => !!document.querySelector('[aria-label="실행 파일 경로"]'), 10);
      if (!document.querySelector('[aria-label="희망 역할"]')) throw new Error('등록 역할이 희망 역할로 구분되지 않았어');
      const candidates = await invoke<{provider:string;installation:string;path:string|null}[]>('detect_agent_candidates');
      if (!candidates.some(c => c.provider === 'claude_code') || !candidates.some(c => c.provider === 'gemini_cli') || !candidates.some(c => c.provider === 'opencode')) throw new Error('지원 후보가 빠졌어');
      if (document.querySelectorAll('[data-candidate]').length !== candidates.length) throw new Error('탐지 후보 목록이 화면에 없어');
      for (const provider of ['gemini_cli', 'opencode']) {
        const candidate = candidates.find(c => c.provider === provider)!;
        if (!candidate.path) throw new Error(`${provider} 기본 경로가 없어`);
        document.querySelector<HTMLButtonElement>(`[data-candidate="${provider}"]`)!.click(); await wait(60);
        if (document.querySelector<HTMLInputElement>('[aria-label="실행 파일 경로"]')!.value !== candidate.path) throw new Error(`${provider} 경로가 입력칸에 반영되지 않았어`);
      }
      document.querySelector<HTMLButtonElement>('[data-candidate="claude_code"]')!.click(); await wait(60);
      await fill('[aria-label="실행 파일 경로"]', fixture);
      document.querySelector<HTMLButtonElement>('[data-action="register-agent"]')!.click();
      await until(() => !!document.querySelector('[data-provider="claude_code"]'), 10);
      const node = document.querySelector<HTMLButtonElement>('[data-provider="claude_code"]')!;
      node.click(); await wait(60);
      if (node.dataset.support !== 'unsupported' || node.dataset.installation !== 'installed' || node.dataset.authentication !== 'unknown' || node.dataset.availability !== 'unavailable') throw new Error('미지원 인스턴스 상태가 잘못됐어');
      const detail = document.querySelector<HTMLElement>('.v0-agents-detail')?.textContent ?? '';
      if (!detail.includes('미지원') || !detail.includes('설정 필요') || !detail.includes('Lead') || !detail.includes('인스턴스 ID') || !detail.includes('희망 역할') || !detail.includes('활성 역할')) throw new Error('등록 상세 정보가 빠졌어');
      if (document.querySelector<HTMLElement>('.v0-agents-detail [data-active-role]')?.dataset.activeRole !== '') throw new Error('미지원 인스턴스의 역할이 활성화됐어');
      const saved = await invoke<{id:string;provider:string;role:string;activeRole:string|null}[]>('list_agent_instances');
      if (saved.length !== 3 || saved.filter(item => item.provider === 'claude_code').length !== 1) throw new Error('등록 행 수가 잘못됐어');
      if (saved[2].role !== 'lead' || saved[2].activeRole !== null) throw new Error('희망 역할과 활성 역할이 분리되지 않았어');
      let duplicateRejected = false;
      try { await invoke('register_agent_instance', { provider: 'claude_code', path: fixture, role: 'lead' }); } catch { duplicateRejected = true; }
      if (!duplicateRejected) throw new Error('중복 등록이 허용됐어');
      document.querySelector<HTMLButtonElement>('[data-action="unregister-agent"]')!.click();
      await until(() => !document.querySelector('[data-provider="claude_code"]'), 10);
      const after = await invoke<Snapshot>('get_snapshot');
      if (JSON.stringify(before.messages) !== JSON.stringify(after.messages) || JSON.stringify(before.sessions) !== JSON.stringify(after.sessions)) throw new Error('등록이 대화 기록을 변경했어');
      // Since ca6b9be registration and removal each bump the revision so the WebView accepts the new snapshot.
      if (after.revision <= before.revision) throw new Error('등록 변경이 snapshot revision을 올리지 않았어');
      registryStatus = { candidates, registeredId: saved[2].id, unsupported: true, duplicateRejected, removed: true, snapshotUnchanged: true };
      document.querySelector<HTMLButtonElement>('[data-action="chat-tab"]')!.click(); await wait(80);
    } else if (mode === 'agents') {
      const before = await invoke<Snapshot>('get_snapshot');
      const chatScroll = document.querySelector<HTMLElement>('.v0-conversation-scroll')!;
      const chatInner = document.querySelector<HTMLElement>('.v0-conversation-inner')!;
      const originalHeight = chatInner.style.minHeight;
      chatInner.style.minHeight = '200vh';
      chatScroll.scrollTop = 160;
      const scrollBefore = chatScroll.scrollTop;
      if (scrollBefore < 100) throw new Error('대화 스크롤 검증 범위를 만들 수 없어');
      document.querySelector<HTMLButtonElement>('[data-action="agents-tab"]')!.click();
      await until(() => [...document.querySelectorAll<HTMLElement>('[data-panel="agents"] [data-agent]')].length === 2 && [...document.querySelectorAll<HTMLElement>('[data-panel="agents"] [data-agent]')].every(node => node.dataset.installation !== 'not_checked'), 10);
      if (getComputedStyle(document.querySelector<HTMLElement>('[data-panel="main"]')!).display !== 'none' || getComputedStyle(document.querySelector<HTMLElement>('[data-panel="collaboration"]')!).display !== 'none') throw new Error('Agents 화면에서 대화 또는 협업 패널이 보였어');
      const agentsWidth = document.querySelector<HTMLElement>('[data-panel="agents"]')!.getBoundingClientRect().width;
      if (agentsWidth < 450) throw new Error('Agents 화면이 남은 영역을 사용하지 않아');
      const rows = async () => {
        const values = [];
        for (const node of document.querySelectorAll<HTMLButtonElement>('[data-panel="agents"] [data-agent]')) {
          node.click(); await wait(30);
          values.push({ id: node.dataset.agent, installation: node.dataset.installation, authentication: node.dataset.authentication, availability: node.dataset.availability, status: node.dataset.status, text: document.querySelector<HTMLElement>('.v0-agents-detail')?.textContent });
        }
        return values;
      };
      const first = await rows();
      if (first.some(row => !row.text?.includes('실행 상태') || !row.text?.includes('연결 방식') || !row.text?.includes('점검 시각') || !row.text?.includes('stdio JSON-RPC') && !row.text?.includes('stream-json'))) throw new Error('Agents 상세 필드 또는 연결 방식이 없어');
      const refresh = document.querySelector<HTMLButtonElement>('[data-action="refresh-agents"]')!;
      let observedChecking = false;
      const observer = new MutationObserver(() => { if (refresh.disabled && refresh.textContent?.includes('점검 중')) observedChecking = true; });
      observer.observe(refresh, { attributes: true, childList: true, subtree: true, characterData: true });
      refresh.click();
      await wait(80);
      await until(() => !refresh.disabled, 10);
      observer.disconnect();
      if (!observedChecking) throw new Error('수동 새로고침의 점검 중 상태가 표시되지 않았어');
      const second = await rows();
      const after = await invoke<Snapshot>('get_snapshot');
      if (JSON.stringify(before.messages) !== JSON.stringify(after.messages) || JSON.stringify(before.sessions) !== JSON.stringify(after.sessions) || JSON.stringify(before.activeRun) !== JSON.stringify(after.activeRun) || before.revision !== after.revision) throw new Error('상태 점검이 대화 또는 실행 상태를 변경했어');
      agentStatus = { first, second, snapshotUnchanged: true, manualRefresh: true, observedChecking, agentsWidth };
      document.querySelector<HTMLButtonElement>('[data-action="chat-tab"]')!.click();
      await wait(80);
      if (chatScroll.scrollTop !== scrollBefore) throw new Error('대화로 돌아온 뒤 스크롤 위치가 바뀌었어');
      chatInner.style.minHeight = originalHeight;
    } else if (mode === 'workspace') {
      workspace = await workspaceProbe();
    } else if (mode === 'live') {
      await submit('MVP_GUI_7319 패널 분리 검증이야. 반드시 request_arch_review를 같은 턴에서 정확히 두 번 순차 호출해줘. 첫 질문은 "MVP_GUI_7319: 메시지 순서를 보장할 설계를 한 문장으로 검토해줘."로 해. 첫 리뷰를 받은 뒤 실제 의견을 반영한 후속 질문으로 "MVP_GUI_7319: 그 설계에서 중복 메시지 방지도 가능한지 한 문장으로 재검토해줘."를 Arch에게 보내줘. 두 리뷰를 모두 받은 뒤 사용자 대상 최종 답변에 MVP_GUI_7319와 SAME_TURN_RESUMED를 넣어 두 문장으로 정리해줘. 다른 도구는 쓰지 마.');
      await until(async () => {
        const current = await invoke<Snapshot>('get_snapshot');
        const failed = current.agents.find(a => a.status === 'failed');
        if (failed) throw new Error(failed.detail || 'CLI failed');
        return current.agents.find(a => a.id === 'antigravity')?.status === 'completed' && current.agents.find(a => a.id === 'codex')?.status === 'completed' && [...document.querySelectorAll('[data-speaker="codex"] [data-content]')].some(n => n.textContent?.includes('SAME_TURN_RESUMED'));
      });
      document.querySelector<HTMLButtonElement>('[data-action="agents-tab"]')!.click(); await wait(80);
      if (!document.querySelector('[data-agent="antigravity"][data-status="completed"]') || !document.querySelector('[data-agent="codex"][data-status="completed"]')) throw new Error('Agents 화면 상태 표시가 실제 완료 상태와 달라');
      document.querySelector<HTMLButtonElement>('[data-action="chat-tab"]')!.click(); await wait(80);
    } else if (mode === 'error') {
      await submit('실제 연결 오류 검증이야. Arch에게 한 문장 설계 검토를 요청하고 실패하면 실패 사실을 알려줘.');
      await until(async () => {
        const current = await invoke<Snapshot>('get_snapshot');
        return current.agents.find(a => a.id === 'antigravity')?.status === 'failed' && !!document.querySelector('[data-error-id]') && current.agents.find(a => a.id === 'codex')?.status === 'completed';
      });
      document.querySelector<HTMLButtonElement>('[data-action="agents-tab"]')!.click(); await wait(80);
      if (!document.querySelector('[data-agent="antigravity"][data-status="failed"]')) throw new Error('Agents 화면에 실패 상태가 없어');
      document.querySelector<HTMLButtonElement>('[data-action="chat-tab"]')!.click(); await wait(80);
    } else if (mode === 'restore-history') {
      const navButton = document.querySelector<HTMLButtonElement>('[data-action="chat-tab"]')!;
      navButton.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, button: 0 }));
      navButton.focus();
      const pointerOutline = getComputedStyle(navButton).outlineStyle;
      if (pointerOutline !== 'none') throw new Error(`클릭 후 포커스 테두리가 남아 있어: ${pointerOutline}`);
      navButton.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'Tab' }));
      const keyboardOutline = getComputedStyle(navButton).outlineStyle;
      if (keyboardOutline === 'none') throw new Error('키보드 탐색용 포커스 표시가 없어');
      focusFeedback = { pointerOutline, keyboardOutline };
      // The shared WebView may remember an empty session from another test DB.
      const saved = await invoke<Snapshot>('get_snapshot');
      const id = saved.messages.at(-1)?.chatSessionId;
      if (id) {
        document.querySelector<HTMLButtonElement>(`[data-session-id="${id}"]`)?.click();
        await wait(100);
        if (document.querySelector('.v0-lead strong')?.textContent !== saved.sessions.find(s => s.id === id)?.name) throw new Error('헤더에 선택 세션 이름이 없어');
        if (document.querySelector('.v0-header-state, .v0-collab-footer')) throw new Error('제거한 상태/안내 UI가 남아 있어');
        const composer = document.querySelector<HTMLTextAreaElement>('.v0-composer textarea');
        if (composer && composer.getBoundingClientRect().height > 74) throw new Error('입력창 기본 높이가 줄지 않았어');
        const listWidth = document.querySelector<HTMLElement>('[data-panel="collaboration"]')!.getBoundingClientRect().width;
        const thread = document.querySelector<HTMLButtonElement>('[data-review-id]');
        if (thread) { thread.click(); await wait(80); }
        const reviewCard = document.querySelector<HTMLElement>('.v0-review-detail .message-card .markdown-body');
        if (reviewCard) {
          const detail = document.querySelector<HTMLElement>('.v0-review-detail')!;
          const detailWidth = document.querySelector<HTMLElement>('[data-panel="collaboration"]')!.getBoundingClientRect().width;
          const reviewFont = parseFloat(getComputedStyle(reviewCard).fontSize);
          if (reviewFont !== 13) throw new Error('협업 본문 글자 크기가 조정 범위를 벗어났어');
          if (listWidth < 500 && detailWidth < listWidth + 80) throw new Error('협업 상세 폭이 목록보다 넓어지지 않았어');
          if (reviewCard.getBoundingClientRect().width < detail.clientWidth - 36) throw new Error('협업 본문이 패널 폭을 충분히 쓰지 않아');
          reviewLayout = { listWidth, detailWidth, reviewFont };
          if (!document.querySelector('[data-action="review-back"]')) throw new Error('스레드 목록으로 돌아가기 버튼이 없어');
          document.querySelector<HTMLButtonElement>('[data-action="review-back"]')!.click(); await wait(80);
          if (!document.querySelector('[data-thread-list]') || document.querySelector('[data-thread-detail]')) throw new Error('뒤로 가기가 목록을 표시하지 않아');
          document.querySelector<HTMLButtonElement>('.v0-review-callout')?.click(); await wait(80);
          if (!document.querySelector('[data-thread-detail]')) throw new Error('중앙 협업 카드가 스레드 상세를 열지 않아');
          document.querySelector<HTMLButtonElement>('[data-action="review-back"]')!.click(); await wait(80);
        }
        const mainCard = document.querySelector<HTMLElement>('.v0-conversation-scroll .markdown-body');
        if (mainCard && parseFloat(getComputedStyle(mainCard).fontSize) !== 14) throw new Error('중앙 대화 글자 크기가 조정 값과 달라');
        if (document.querySelector('[aria-label="파일 첨부 미지원"]')) throw new Error('미지원 첨부 버튼이 여전히 보여');
        document.querySelector<HTMLButtonElement>('[data-action="agents-tab"]')!.click();
        await wait(80);
        if (!document.querySelector('[data-panel="agents"] [data-agent="codex"]') || !document.querySelector('[data-panel="agents"] [data-agent="antigravity"]')) throw new Error('Agents 화면의 에이전트 목록이 없어');
        document.querySelector<HTMLButtonElement>('[data-action="chat-tab"]')!.click();
        await wait(80);
      }
    } else if (mode !== 'restore') { throw new Error('Unknown probe mode'); }
    if (!document.querySelector('[data-thread-list], [data-thread-detail]')) {
      document.querySelector<HTMLButtonElement>('.v0-collab-toggle')!.click();
      await wait(80);
    }
    await wait(400);
    const initialPanelWidths = Object.fromEntries(['explorer', 'collaboration'].map(panel => [panel, document.querySelector<HTMLElement>(`[data-panel="${panel}"]`)!.getBoundingClientRect().width]));
    const resizeResults = [];
    for (const side of ['left', 'right']) {
      const handle = document.querySelector<HTMLElement>(`[data-resizer="${side}"]`)!;
      const panel = document.querySelector<HTMLElement>(`[data-panel="${side === 'left' ? 'explorer' : 'collaboration'}"]`)!;
      handle.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'Home' })); await wait(60);
      const before = panel.getBoundingClientRect().width;
      const x = handle.getBoundingClientRect().x;
      handle.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, pointerId: 71, button: 0, clientX: x }));
      await wait(30);
      handle.dispatchEvent(new PointerEvent('pointermove', { bubbles: true, pointerId: 71, clientX: x + (side === 'left' ? 30 : -30) }));
      await wait(80);
      handle.dispatchEvent(new PointerEvent('pointerup', { bubbles: true, pointerId: 71 }));
      resizeResults.push({ side, before, after: panel.getBoundingClientRect().width });
    }
    let navigation: unknown = null;
    const session = document.querySelector<HTMLButtonElement>('[data-session-id]');
    if (session) {
      const previous = document.querySelector<HTMLButtonElement>('[data-session-id][aria-current="page"]')?.dataset.sessionId;
      session.click(); await wait(100);
      const selectedSnapshot = await invoke<Snapshot>('get_snapshot');
      navigation = { sessionId: session.dataset.sessionId, writable: !!document.querySelector('[data-panel="main"] textarea'), messageIds: [...document.querySelectorAll<HTMLElement>('[data-panel="main"] [data-message-id]')].map(n => n.dataset.messageId).concat((await reviewMessages(selectedSnapshot)).map(row => row.id)) };
      if (previous) document.querySelector<HTMLButtonElement>(`[data-session-id="${previous}"]`)?.click();
      await wait(100);
    }
    const snapshot = await invoke<Snapshot>('get_snapshot');
    const rendered = [...[...document.querySelectorAll<HTMLElement>('[data-panel="main"] [data-message-id]')].map(node => renderedMessage(node, snapshot)), ...await reviewMessages(snapshot)];
    const agents = [...document.querySelectorAll<HTMLElement>('[data-agent]')].map(node => ({ id: node.dataset.agent, status: node.dataset.status, text: node.textContent }));
    await invoke('record_gui_probe', { payload: { mode, workspace, importUi, modelSelection, markdownUi, cancellation, streaming, contextUsage, compact, focusFeedback, themeSelection, reviewLayout, agentStatus, registryStatus, opencodeStatus, contextDisplay: document.querySelector('[aria-label="Codex 컨텍스트 상태"]')?.textContent, composerSelection: {model: document.querySelector<HTMLSelectElement>('[aria-label="Codex 모델"]')?.value, effort: document.querySelector<HTMLSelectElement>('[aria-label="Codex 추론 강도"]')?.value}, selectedSessionId: document.querySelector<HTMLElement>('[data-session-id][aria-current="page"]')?.dataset.sessionId, startedAt, completedAt: new Date().toISOString(), rendered, agents, snapshot, resizeResults, initialPanelWidths, navigation, collaborationEditableCount: document.querySelectorAll('[data-panel="collaboration"] textarea, [data-panel="collaboration"] input:not([disabled]), [data-panel="collaboration"] form, [data-panel="collaboration"] [contenteditable="true"]').length, archivedIds: [...document.querySelectorAll<HTMLElement>('[data-archived-id]')].map(n => n.dataset.archivedId), alerts: [...document.querySelectorAll('[role="alert"]')].map(n => n.textContent) } });
  } catch (e) {
    await invoke('record_gui_probe', { payload: { mode, startedAt, failed: String(e), body: document.body.innerText } });
  } finally { await invoke('close_probe'); }
}
