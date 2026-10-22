import React, { useEffect, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { CodexModel, ExternalThread, Message, Room, Session, Snapshot } from './types';
import { usePanelWidths } from './panels';
import { threadTitle, threadCategory } from './externalThreads';
import './style.css';
import './v0.css';
import { MarkdownMessage } from './MarkdownMessage';
import { ContextStatus } from './ContextStatus';
import { Icon } from './Icons';

const states: Record<string, string> = { idle: '대기', cancelling: '중단 중', running: '답변 중', compacting: '컨텍스트 압축 중', waiting_for_arch: 'Arch 검토 대기', arch_running: '검토 중', resuming: 'Codex 재개 중', completed: '답변 완료', failed: '실패', cancelled: '취소됨', unknown: '상태 확인 필요' };
const effortLabels: Record<string, string> = { none: '없음', minimal: '최소', low: '낮음', medium: '보통', high: '높음', xhigh: '매우 높음', max: '최대', ultra: '울트라' };
const initial: Snapshot = { contexts: [], activeRun: null, messages: [], agents: [], conversationVersion: 0, revision: -1, sessions: [], rooms: [], busySessionId: null };
const time = (at: string) => new Date(at).toLocaleTimeString('ko-KR', { hour: '2-digit', minute: '2-digit' });

function MessageCard({ message: m, collaboration = false }: { message: Message; collaboration?: boolean }) {
  const title = collaboration ? m.speaker === 'antigravity' ? 'Antigravity' : 'Codex' : m.speaker === 'user' ? 'You' : 'Codex · Lead';
  const label = m.kind === 'review_request' ? '검토 요청' : m.kind === 'review_followup' ? '후속 검토 요청' : m.kind === 'review_reply' ? '리뷰 응답' : null;
  return <article data-message-id={m.id} data-delivery={m.delivery} aria-busy={m.delivery === 'streaming'} data-speaker={m.speaker} data-sequence={m.sequence} data-channel={m.channel} data-kind={m.kind} data-collaboration-id={m.collaborationId ?? ''} className={`message-card ${collaboration ? 'collaboration-card' : m.speaker === 'user' ? 'user-card' : ''}`}>
    <div className="message-meta"><strong className={m.speaker === 'antigravity' ? 'text-violet-300' : m.speaker === 'codex' ? 'text-teal-300' : 'text-slate-200'}>{title}</strong><time>{time(m.createdAt)}</time>{m.speaker === 'user' && <span className="delivery">{m.delivery === 'queued' ? '전달 대기' : m.delivery === 'cancelled' ? '취소됨' : m.delivery === 'interrupted' ? '이전 실행에서 중단됨' : '전달됨'}</span>}</div>
    {m.speaker === 'codex' && ['streaming', 'interrupted'].includes(m.delivery) && <div className="stream-label">{m.delivery === 'streaming' ? '작성 중…' : '중단된 답변'}</div>}{label && <div className="review-label">{label}</div>}{m.speaker === 'user' ? <p data-content>{m.content}</p> : <MarkdownMessage content={m.content}/>}
  </article>;
}

export function App() {
  const [snapshot, setSnapshot] = useState<Snapshot>(initial);
  const [text, setText] = useState('');
  const [error, setError] = useState('');
  const [ready, setReady] = useState(false);
  const [sending, setSending] = useState(false);
  const [stopping, setStopping] = useState(false);
  const [models, setModels] = useState<CodexModel[]>([]);
  const [modelsLoading, setModelsLoading] = useState(false);
  const [modelsError, setModelsError] = useState('');
  const [settingsError, setSettingsError] = useState('');
  const [settingsSaving, setSettingsSaving] = useState(false);
  const [selectedSession, setSelectedSession] = useState<string | null>('default-session');
  const [selectedRoom, setSelectedRoom] = useState('default');
  const [dialog, setDialog] = useState<'room' | 'session' | 'import' | null>(null);
  const [name, setName] = useState('');
  const [dialogError, setDialogError] = useState('');
  const [working, setWorking] = useState(false);
  const [external, setExternal] = useState<ExternalThread[]>([]);
  const [cursor, setCursor] = useState<string | null>(null);
  const [search, setSearch] = useState('');
  const [source, setSource] = useState<string | null>(null);
  const [showVerification, setShowVerification] = useState(false);
  const listRequest = useRef(0);
  const [importMode, setImportMode] = useState<'copy' | 'resume'>('copy');
  const [explorerOpen, setExplorerOpen] = useState(true);
  const [collaborationOpen, setCollaborationOpen] = useState(true);
  const [searchOpen, setSearchOpen] = useState(false);
  const [sessionSearch, setSessionSearch] = useState('');
  const [agentsOpen, setAgentsOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [selectedReview, setSelectedReview] = useState<string | null>(null);
  const bottom = useRef<HTMLDivElement>(null);
  const collaborationBottom = useRef<HTMLDivElement>(null);
  const root = useRef<HTMLDivElement>(null);
  const probeStarted = useRef(false);
  const panels = usePanelWidths(root);
  const accept = (next: Snapshot) => setSnapshot(prev => next.revision >= prev.revision ? next : prev);
  const session = snapshot.sessions.find(s => s.id === selectedSession);
  const activeRun = snapshot.activeRun?.sessionId === selectedSession ? snapshot.activeRun : null;
  const isCancelling = stopping || !!activeRun?.cancelling;
  const selectedModel = models.find(m => m.model === session?.model);
  const effortOptions = selectedModel ? (selectedModel.supportedReasoningEfforts.length ? selectedModel.supportedReasoningEfforts : [{ reasoningEffort: selectedModel.defaultReasoningEffort, description: '모델 기본 추론 강도' }]) : [];
  const room = snapshot.rooms.find(r => r.id === selectedRoom);
  const visible = snapshot.messages.filter(m => selectedSession ? m.chatSessionId === selectedSession : m.roomId === selectedRoom);
  const main = visible.filter(m => m.channel === 'main' && (m.speaker === 'user' || m.speaker === 'codex'));
  const collaboration = visible.filter(m => m.channel === 'collaboration' && ['review_request', 'review_followup', 'review_reply'].includes(m.kind) && m.speaker !== 'user').sort((a, b) => a.createdAt.localeCompare(b.createdAt) || a.sequence - b.sequence);
  const archived = visible.filter(m => m.channel === 'legacy' || m.channel === 'activity');
  const failures = visible.filter(m => m.isError);
  const contextState = snapshot.contexts.find(c => c.sessionId === selectedSession && c.threadId === session?.threadId);
  const reviews = (() => {
    const groups = new Map<string, Message[]>();
    for (const message of collaboration) {
      const key = message.collaborationId ?? message.id;
      groups.set(key, [...(groups.get(key) ?? []), message]);
    }
    return [...groups.entries()].map(([id, messages]) => ({ id, messages, first: messages[0], last: messages.at(-1)! })).sort((a, b) => b.last.sequence - a.last.sequence);
  })();
  const currentReview = selectedReview ? reviews.find(r => r.id === selectedReview) : undefined;
  const running = !!activeRun || snapshot.busySessionId === selectedSession;
  const pendingReview = reviews.some(r => r.last.kind !== 'review_reply' && !r.last.isError);
  const compacting = contextState?.compaction === 'compacting';
  const compactBlocked = compacting ? undefined : !session?.threadId ? '이 세션에는 아직 Codex 스레드가 없습니다. 먼저 메시지를 한 번 보내 주세요.' : snapshot.busySessionId ? 'Codex가 실행 중입니다. 완료된 뒤 압축할 수 있습니다.' : undefined;

  useEffect(() => {
    if (!isTauri()) { setError('실제 CLI 연결은 Tauri 데스크톱 앱에서 사용할 수 있습니다.'); return; }
    let disposed = false; let off: (() => void) | undefined;
    (async () => {
      off = await listen<Snapshot>('chat-updated', ({ payload }) => { if (!disposed) accept(payload); });
      const stored = await invoke<Snapshot>('get_snapshot');
      if (!disposed) {
        accept(stored);
        let saved: string | null = null; try { saved = localStorage.getItem('esh.selected-session'); } catch { /* optional preference */ }
        const chosen = stored.sessions.find(s => s.id === saved) ?? stored.sessions.find(s => s.id === stored.messages.at(-1)?.chatSessionId) ?? stored.sessions[0];
        setSelectedSession(chosen?.id ?? null); setSelectedRoom(chosen?.roomId ?? stored.rooms[0]?.id ?? 'default'); setReady(true);
      } else off();
    })().catch(e => setError(String(e)));
    return () => { disposed = true; off?.(); };
  }, []);
  useEffect(() => { if (ready) void loadModels(); }, [ready]);
  useEffect(() => { setSettingsError(''); }, [selectedSession]);
  useEffect(() => { setSelectedReview(null); }, [selectedSession]);
  useEffect(() => { bottom.current?.scrollIntoView({ behavior: main.at(-1)?.delivery === 'streaming' ? 'auto' : 'smooth', block: 'end' }); }, [main.length, main.at(-1)?.content.length, selectedSession]);
  useEffect(() => { collaborationBottom.current?.scrollIntoView({ behavior: 'smooth', block: 'end' }); }, [collaboration.length, selectedSession]);
  useEffect(() => { if (ready && selectedSession) try { localStorage.setItem('esh.selected-session', selectedSession); } catch { /* optional preference */ } }, [ready, selectedSession]);
  useEffect(() => {
    if (!ready || probeStarted.current) return;
    probeStarted.current = true;
    invoke<string | null>('get_probe_mode').then(async mode => { if (mode) { const { runGuiProbe } = await import('./guiProbe'); await runGuiProbe(mode); } }).catch(e => setError(String(e)));
  }, [ready]);

  async function send(e: React.FormEvent) {
      e.preventDefault(); if (!text.trim() || sending || isCancelling || settingsSaving || !ready || !selectedSession) return;
      setSending(true); setError('');
      try { accept(await invoke<Snapshot>('send_message', { text, target: 'codex', sessionId: selectedSession })); setText(''); }
      catch (e) { setError(String(e)); } finally { setSending(false); }
    }
    function retryArchReview() {
      // Arch timeout/failure is reported as an error banner; the user can resend the same text to re-run the turn.
      setError('');
      const form = document.querySelector('form.v0-composer');
      if (form) { (form as HTMLFormElement).requestSubmit(); }
    }

  async function stopRun() {
    if (!activeRun || isCancelling) return;
    setStopping(true); setError('');
    try { accept(await invoke<Snapshot>('cancel_run', {sessionId: activeRun.sessionId, runId: activeRun.id})); }
    catch (e) { setError(String(e)); }
    finally { setStopping(false); }
  }
  async function compactContext() {
    if (!selectedSession || compacting) return;
    setError('');
    try { accept(await invoke<Snapshot>('compact_session', { sessionId: selectedSession })); }
    catch (e) { setError(String(e)); }
  }
  async function loadModels() {
    setModelsLoading(true); setModelsError('');
    try { setModels(await invoke<CodexModel[]>('list_codex_models')); }
    catch (e) { setModelsError(`모델 목록 조회 실패: ${String(e)}`); }
    finally { setModelsLoading(false); }
  }
  async function saveModelSettings(model: string, reasoningEffort: string) {
    if (!selectedSession || settingsSaving) return;
    setSettingsSaving(true); setSettingsError('');
    try { accept(await invoke<Snapshot>('set_session_model_settings', { sessionId: selectedSession, settings: { model, reasoningEffort } })); }
    catch (e) { setSettingsError(`설정 저장 실패: ${String(e)}`); }
    finally { setSettingsSaving(false); }
  }

  function select(s: Session) { setSelectedRoom(s.roomId); setSelectedSession(s.id); setText(''); setError(''); }
  function openDialog(value: 'room' | 'session' | 'import') { setDialog(value); setName(''); setDialogError(''); setSource(null); setShowVerification(false); setImportMode('copy'); if (value === 'import') { setExternal([]); setCursor(null); setSearch(''); void loadExternal(false, ''); } }
  async function loadExternal(more: boolean, term = search) {
    const request = ++listRequest.current;
    if (!more) { setSource(null); setName(''); setCursor(null); setExternal([]); }
    setWorking(true); setDialogError('');
    try { const page = await invoke<{ data: ExternalThread[]; nextCursor: string | null }>('list_codex_sessions', { cursor: more ? cursor : null, search: term || null }); if (request !== listRequest.current) return; setExternal(prev => more ? [...prev, ...page.data.filter(t => !prev.some(p => p.id === t.id))] : page.data); setCursor(page.nextCursor); }
    catch (e) { if (request === listRequest.current) setDialogError(String(e)); } finally { if (request === listRequest.current) setWorking(false); }
  }
  async function saveDialog(e: React.FormEvent) {
    e.preventDefault(); if (working) return; setWorking(true); setDialogError('');
    try {
      if (dialog === 'room') { const created = await invoke<Room>('create_room', { name }); accept(await invoke<Snapshot>('get_snapshot')); setSelectedRoom(created.id); setSelectedSession(null); }
      else { const created = dialog === 'session' ? await invoke<Session>('create_session', { roomId: selectedRoom, name }) : await invoke<Session>('import_codex_session', { roomId: selectedRoom, threadId: source, name: name || external.find(t => t.id === source)?.name || '가져온 Codex 세션', mode: importMode }); accept(await invoke<Snapshot>('get_snapshot')); select(created); }
      setDialog(null);
    } catch (e) { setDialogError(String(e)); } finally { setWorking(false); }
  }

  return <div ref={root} className="app-shell v0-shell" data-focus-mode="pointer" onPointerDownCapture={e => { e.currentTarget.dataset.focusMode = 'pointer'; }} onKeyDownCapture={e => { if (e.key === 'Tab' || e.key.startsWith('Arrow')) e.currentTarget.dataset.focusMode = 'keyboard'; }} style={{ gridTemplateColumns: `${explorerOpen ? panels.widths.left : 42}px ${explorerOpen ? 6 : 0}px minmax(360px, 1fr) ${collaborationOpen ? 6 : 0}px ${collaborationOpen ? panels.widths.right : 42}px` }}>
    <aside data-panel="explorer" className={`v0-explorer ${explorerOpen ? '' : 'v0-collapsed'}`} aria-label="프로젝트·룸·세션 탐색기">
      {explorerOpen ? <>
        <header className="v0-side-header"><strong>ESH Studio</strong><button type="button" className="v0-icon-button" aria-label="검색" title="세션 검색" onClick={() => setSearchOpen(v => !v)}><Icon name="search"/></button></header>
        {searchOpen && <div className="v0-search"><input autoFocus aria-label="세션 검색" placeholder="세션 이름 검색" value={sessionSearch} onChange={e => setSessionSearch(e.target.value)}/><button type="button" aria-label="검색 닫기" onClick={() => { setSearchOpen(false); setSessionSearch(''); }}><Icon name="x"/></button></div>}
        <div className="v0-new-row"><button type="button" data-action="new-session" onClick={() => openDialog('session')} disabled={!ready}><Icon name="plus"/> 새 세션</button></div>
        <nav className="v0-primary-nav" aria-label="주 메뉴"><button type="button" data-action="chat-tab" className={!agentsOpen ? 'active' : ''} onClick={() => setAgentsOpen(false)}><Icon name="chat"/> Chat</button><button type="button" data-action="agents-tab" className={agentsOpen ? 'active' : ''} onClick={() => setAgentsOpen(true)}><Icon name="bot"/> Agents <small>{snapshot.agents.length}</small></button></nav>

        {agentsOpen ? <section className="v0-explorer-body"><div className="v0-kicker">AVAILABLE AGENTS</div><section className="v0-connections" aria-label="에이전트 연결 상태"><div className="v0-kicker">CONNECTIONS <span>{snapshot.agents.length} agents</span></div>{snapshot.agents.map(a => <div key={a.id} data-agent={a.id} data-status={a.status} title={a.detail} className="v0-agent-row"><span className={`v0-dot ${a.id}`}/><span>{a.id === 'antigravity' ? 'Antigravity' : 'Codex'}</span><small>{states[a.status] ?? a.status}</small></div>)}</section>{snapshot.agents.map(a => <div key={a.id} className="v0-agent-detail"><span className={`v0-dot ${a.id}`}/><div><strong>{a.id === 'antigravity' ? 'Antigravity Arch' : 'Codex Lead'}</strong><small>{states[a.status] ?? a.status}{a.detail ? ` · ${a.detail}` : ''}</small></div></div>)}<p className="v0-muted-note">현재 ESH는 Codex와 Antigravity만 연결합니다.</p></section> : <nav className="v0-explorer-body" aria-label="최근 세션"><div className="v0-section-head"><span className="v0-kicker">RECENT</span><button type="button" data-action="new-room" onClick={() => openDialog('room')} disabled={!ready} title="새 프로젝트·룸" aria-label="새 프로젝트·룸"><Icon name="plus"/></button></div>{snapshot.rooms.map(r => <details key={r.id} open className="v0-room"><summary data-room={r.id} onClick={() => { if (selectedRoom !== r.id) { setSelectedRoom(r.id); setSelectedSession(snapshot.sessions.find(s => s.roomId === r.id)?.id ?? null); setText(''); setError(''); } }}><Icon name="folder"/><span>{r.name}</span><small>{snapshot.sessions.filter(s => s.roomId === r.id).length}</small></summary><div className="v0-room-actions"><button type="button" data-action="new-session" data-room-id={r.id} aria-label="새 세션" title="새 세션" onClick={() => { setSelectedRoom(r.id); openDialog('session'); }}><Icon name="plus"/></button><button type="button" data-action="import-session" data-room-id={r.id} aria-label="Codex 추가" title="Codex 추가" onClick={() => { setSelectedRoom(r.id); openDialog('import'); }}><Icon name="link"/></button></div>{snapshot.sessions.filter(s => s.roomId === r.id && s.name.toLocaleLowerCase().includes(sessionSearch.toLocaleLowerCase())).map(s => <button type="button" key={s.id} data-session-id={s.id} title={s.threadId ?? s.sourceThreadId ?? s.id} className={`v0-session ${selectedSession === s.id ? 'selected' : ''}`} onClick={() => select(s)} aria-current={selectedSession === s.id ? 'page' : undefined}><span>{s.name}</span></button>)}{!snapshot.sessions.some(s => s.roomId === r.id) && <p className="v0-muted-note">세션이 없습니다.</p>}</details>)}{archived.length > 0 && <details className="v0-archive"><summary><Icon name="archive"/> 이전 대화·진행 기록 <small>{archived.length}</small></summary>{archived.map(m => <article key={m.id} data-archived-id={m.id}><strong>{m.speaker === 'antigravity' ? 'Antigravity' : m.speaker === 'codex' ? 'Codex 진행 안내' : '사용자'}</strong><p>{m.content}</p></article>)}</details>}</nav>}
        <footer className="v0-explorer-footer"><div><strong>{room?.name ?? 'ESH workspace'}</strong><small>Lead: Codex · 로컬 저장</small></div><button type="button" className="v0-icon-button" aria-label="워크스페이스 메뉴" title="워크스페이스 작업" onClick={() => setSettingsOpen(v => !v)}><Icon name="settings"/></button>{settingsOpen && <div className="v0-footer-menu"><button type="button" onClick={() => { setSettingsOpen(false); openDialog('room'); }}>새 프로젝트·룸</button><button type="button" onClick={() => { setSettingsOpen(false); openDialog('import'); }}>Codex 기록 추가</button></div>}</footer>
      </> : <button type="button" className="v0-rail-button" aria-label="탐색기 열기" onClick={() => setExplorerOpen(true)}><Icon name="menu"/></button>}
    </aside>
    <div {...panels.separator('left')} style={{ visibility: explorerOpen ? 'visible' : 'hidden' }} />
    <main data-panel="main" className="v0-main" aria-label="Lead conversation">
      <header className="v0-main-header"><button type="button" className="v0-icon-button v0-explorer-toggle" aria-label={explorerOpen ? '탐색기 닫기' : '탐색기 열기'} onClick={() => setExplorerOpen(v => !v)}><Icon name="menu"/></button><span className="v0-dot codex"/><div className="v0-lead"><strong title={session?.name ?? '세션 선택'}>{session?.name ?? '세션 선택'}</strong><small>Codex Lead · {selectedModel?.displayName ?? session?.model ?? '기본 모델'}</small></div><button type="button" className="v0-icon-button v0-collab-toggle" aria-label={collaborationOpen ? '협업 패널 접기' : '협업 패널 열기'} onClick={() => setCollaborationOpen(v => !v)}><Icon name="panel"/></button></header>
      <section data-channel-panel="main" aria-label="사용자와 Codex 대화" aria-live="polite" className="v0-conversation-scroll"><div className="v0-conversation-inner">{main.length === 0 ? <div className="v0-empty"><span className="v0-kicker">NEW TASK</span><h2>무엇을 구현해 드릴까요?</h2><p>Codex와 대화를 시작해 주세요. Arch 검토는 협업 패널에 표시됩니다.</p></div> : main.map(m => <MessageCard key={m.id} message={m}/>)}{reviews.length > 0 && <button type="button" className="v0-review-callout" onClick={() => { setSelectedReview(reviews[0].id); setCollaborationOpen(true); }}><Icon name={running && pendingReview ? 'clock' : 'check'}/><span><strong>{running && pendingReview ? '리뷰 응답을 기다리는 중입니다' : '협업 검토 기록 보기'}</strong><small>{reviews[0].first.content.slice(0, 70)} · Codex → Antigravity</small></span><Icon name="chevronRight"/></button>}<div ref={bottom}/></div></section>
      <div className="v0-composer-area">{modelsError && <p role="alert" className="error-banner">{modelsError} <button type="button" className="model-retry" onClick={() => loadModels()} disabled={modelsLoading}>다시 불러오기</button></p>}{settingsError && <p role="alert" className="error-banner">{settingsError}</p>}{error && <p role="alert" className="error-banner">{error}{/Arch.*시간 초과|Antigravity.*시간 초과|Arch.*종료 시간 초과/.test(error) && <button type="button" className="model-retry" onClick={() => retryArchReview()}>다시 검토</button>}</p>}{failures.slice(-2).map(m => <p key={m.id} role="alert" data-error-id={m.id} className="error-banner">{m.content}</p>)}{!selectedSession ? <div className="history-notice">이 프로젝트·룸에 세션을 추가해 주세요.<button onClick={() => openDialog('session')}>새 세션 만들기</button></div> : <form onSubmit={send} className="v0-composer"><textarea aria-label="Codex에게 메시지" value={text} onInput={e => setText(e.currentTarget.value)} placeholder="Codex에게 요청하세요…" rows={2} disabled={!ready} onKeyDown={e => { if (e.key === 'Enter' && !e.shiftKey && !e.nativeEvent.isComposing) { e.preventDefault(); e.currentTarget.form?.requestSubmit(); } }}/><div className="v0-composer-toolbar"><div className="v0-composer-options"><button type="button" disabled title="파일 첨부는 아직 지원하지 않습니다" aria-label="파일 첨부 미지원" className="v0-icon-button"><Icon name="paperclip"/></button><label className="v0-select-wrap"><span className="sr-only">모델</span><select aria-label="Codex 모델" value={session?.model ?? ''} disabled={modelsLoading || settingsSaving || sending || !models.length} onChange={e => { const chosen = models.find(m => m.model === e.target.value); if (chosen) void saveModelSettings(chosen.model, chosen.defaultReasoningEffort); }}><option value="" disabled>{modelsLoading ? '모델 불러오는 중…' : '기존 세션·기본 모델'}</option>{session?.model && !selectedModel && <option value={session.model} disabled>{session.model} · 목록 확인 필요</option>}{models.map(m => <option key={m.model} value={m.model}>{m.displayName}</option>)}</select><Icon name="chevronDown"/></label><label className="v0-select-wrap v0-effort"><span className="sr-only">추론 강도</span><select aria-label="Codex 추론 강도" value={session?.reasoningEffort ?? ''} disabled={!selectedModel || modelsLoading || settingsSaving || sending} onChange={e => { if (session?.model) void saveModelSettings(session.model, e.target.value); }}>{!selectedModel && <option value="">낮음</option>}{effortOptions.map(e => <option key={e.reasoningEffort} value={e.reasoningEffort} title={e.description}>{effortLabels[e.reasoningEffort] ?? e.reasoningEffort}</option>)}</select><Icon name="chevronDown"/></label></div><div className="v0-send-actions">{activeRun && <button type="button" data-action="stop-run" className="v0-stop" onClick={() => stopRun()} disabled={isCancelling} title="현재 실행 중단"><Icon name="stop"/>{isCancelling ? '중단 중' : '중단'}</button>}{selectedSession && <ContextStatus key={selectedSession} context={contextState} onCompact={() => void compactContext()} compactDisabled={!!compactBlocked || sending || isCancelling || settingsSaving} compactReason={compactBlocked}/>}<button type="submit" className="v0-send" aria-label="메시지 전송" disabled={!ready || sending || isCancelling || settingsSaving || !text.trim()}><Icon name="arrowUp"/></button></div></div></form>}<p className="v0-composer-hint">Enter 전송 · Shift+Enter 줄바꿈 · 파일 첨부 미지원 {settingsSaving ? '· 모델 설정 저장 중' : ''}</p></div>
    </main>
    <div {...panels.separator('right')} style={{ visibility: collaborationOpen ? 'visible' : 'hidden' }} />
    <aside data-panel="collaboration" className={`v0-collaboration ${collaborationOpen ? '' : 'v0-collapsed'}`} aria-label="AI 협업 타임라인">
      {collaborationOpen ? <>
        <header className="v0-collab-header">
          {currentReview && <button type="button" className="v0-icon-button v0-review-back" data-action="review-back" aria-label="협업 스레드 목록으로 돌아가기" title="스레드 목록" onClick={() => setSelectedReview(null)}><Icon name="arrowLeft"/></button>}
          <div className="v0-collab-heading"><strong title={currentReview?.first.content.split('\n')[0]}>{currentReview ? currentReview.first.content.split('\n')[0].slice(0, 80) || '검토 대화' : 'AI COLLABORATION'}</strong><small>{currentReview ? `Codex → Antigravity · ${currentReview.messages.length}개 기록` : `검토 스레드 ${reviews.length}개 · 읽기 전용`}</small></div>
          <button type="button" className="v0-icon-button" aria-label="협업 패널 접기" onClick={() => setCollaborationOpen(false)}><Icon name="chevrons"/></button>
        </header>
        {currentReview ? <section data-channel-panel="collaboration" data-thread-detail={currentReview.id} aria-label="검토 요청과 응답" aria-live="polite" className="v0-review-detail">
          {currentReview.messages.map(m => <MessageCard key={m.id} message={m} collaboration/>)}
          <div ref={collaborationBottom}/>
        </section> : <section data-thread-list aria-label="협업 스레드 목록" className="v0-review-list">
          <div className="v0-review-heading">REVIEW THREADS <span>{reviews.length}</span></div>
          <div className="v0-review-items">{reviews.length ? reviews.map(r => <button type="button" key={r.id} data-review-id={r.id} className="v0-review-thread" onClick={() => setSelectedReview(r.id)}><span className={`v0-dot ${r.last.isError ? 'error' : r.last.kind === 'review_reply' ? 'done' : running ? 'waiting' : 'unknown'}`}/><span className="v0-review-title"><strong>{r.first.content.split('\n')[0].slice(0, 80) || '검토 요청'}</strong><small>Codex → Antigravity · {time(r.last.createdAt)}</small></span><em>{r.last.isError ? '오류' : r.last.kind === 'review_reply' ? '응답 완료' : running ? '대기 중' : '상태 확인 필요'}</em></button>) : <p className="v0-collab-empty">아직 검토 스레드가 없습니다. 협업이 시작되면 여기에 표시됩니다.</p>}</div>
        </section>}
      </> : <button type="button" className="v0-rail-button" aria-label="협업 패널 펼치기" onClick={() => setCollaborationOpen(true)}><Icon name="chevrons"/><span>AI Collaboration</span></button>}
    </aside>
    {dialog && <div className="modal-backdrop" onKeyDown={e => { if (e.key === 'Escape' && !working) setDialog(null); }}><section role="dialog" aria-modal="true" aria-label={dialog === 'room' ? '새 프로젝트·대화룸' : dialog === 'session' ? '새 세션' : '기존 Codex 세션 추가'} className="workspace-dialog"><header><h2>{dialog === 'room' ? '새 프로젝트·대화룸' : dialog === 'session' ? '새 세션' : '기존 Codex 세션 추가'}</h2><button aria-label="닫기" onClick={() => setDialog(null)} disabled={working}>×</button></header><form onSubmit={saveDialog}>
      {dialog === 'import' && <><div className="search-row"><input disabled={working} aria-label="Codex 세션 검색" placeholder="세션 제목으로 검색" value={search} onChange={e => setSearch(e.target.value)} onKeyDown={e => { if (e.key === 'Enter') { e.preventDefault(); if (!working) void loadExternal(false); } }}/><button type="button" onClick={() => loadExternal(false)} disabled={working}>검색</button></div><label className="verification-toggle"><input type="checkbox" aria-label="자동 실행·검증 세션 표시" checked={showVerification} onChange={e => { setShowVerification(e.target.checked); if (!e.target.checked && source && external.some(t => t.id === source && threadCategory(t) !== 'conversation')) { setSource(null); setName(''); } }}/>자동 실행·검증 세션 표시</label><p className="import-note">기본 목록에는 대화 세션을 표시합니다. 이 페이지에서 숨긴 자동 실행·검증 기록 {external.filter(t => threadCategory(t) !== 'conversation').length}개는 체크하면 볼 수 있습니다.</p><div className="external-list">{external.filter(t => showVerification || threadCategory(t) === 'conversation').map(t => <label key={t.id} data-external-id={t.id}><input type="radio" name="codex-source" value={t.id} checked={source === t.id} onChange={() => { setSource(t.id); setName(threadTitle(t)); }}/><span><strong title={threadTitle(t)}>{threadTitle(t)}</strong>{threadCategory(t) !== 'conversation' && <em className="verification-badge">{threadCategory(t) === 'verification' ? '검증용' : '자동 실행'}</em>}<small>{t.cwd || t.id}</small></span></label>)}{working && <p>Codex 기록을 읽는 중…</p>}{!working && !external.some(t => showVerification || threadCategory(t) === 'conversation') && <p>표시할 세션이 없습니다. 검색어를 바꾸거나 더 불러와 주세요.</p>}</div>{cursor && <button type="button" onClick={() => loadExternal(true)} disabled={working}>더 불러오기</button>}<label className="field-label">추가 방식<select aria-label="가져오기 방식" value={importMode} onChange={e => setImportMode(e.target.value as 'copy' | 'resume')}><option value="copy">기록 가져오기 · 원본 보존, 새 Arch 협업 세션</option><option value="resume">원본 이어쓰기 · 원본 대화에 저장</option></select></label><p className="import-note">{importMode === 'copy' ? '공개 텍스트 대화를 새 세션에 가져옵니다. 원본은 보존하며 Arch 검토를 사용할 수 있습니다. 첨부파일·내부 추론은 가져오지 않습니다.' : '원본에 새 대화를 저장합니다. 원본 앱과 동시에 실행하지 마세요. Arch 도구를 새로 붙이는 기능은 지원하지 않습니다.'}</p></>}
      {dialog === 'import' && !source && <p className="import-note">목록에서 세션을 선택하면 이름이 자동으로 채워집니다.</p>}<label className="field-label">{dialog === 'room' ? '프로젝트·룸 이름' : '세션 이름'}<input autoFocus={dialog !== 'import'} aria-label="이름" maxLength={100} value={name} onChange={e => setName(e.target.value)} required/></label>
      {dialogError && <p role="alert" className="error-banner">{dialogError}</p>}<footer><button type="button" onClick={() => setDialog(null)} disabled={working}>취소</button><button type="submit" disabled={working || !name.trim() || (dialog === 'import' && !source)}>{working ? '처리 중…' : dialog === 'import' ? '세션 추가' : '만들기'}</button></footer>
    </form></section></div>}
  </div>;
}
createRoot(document.getElementById('root')!).render(<App/>);
