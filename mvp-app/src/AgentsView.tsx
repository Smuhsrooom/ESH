import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { AgentCandidate, AgentInstance, AgentProbe, Snapshot } from './types';

const executionLabels: Record<string, string> = {
  idle: '대기', cancelling: '중단 중', running: '답변 중', compacting: '컨텍스트 압축 중',
  waiting_for_arch: 'Arch 검토 대기', arch_running: '검토 중', resuming: 'Codex 재개 중',
  completed: '답변 완료', failed: '실패', cancelled: '취소됨', unknown: '상태 확인 필요',
};
const probeLabels: Record<string, string> = {
  installed: '설치됨', missing: '미설치', authenticated: '인증됨', required: '인증 필요',
  ready: '실행 가능', unavailable: '실행 불가', unknown: '불명', error: '점검 실패',
};
const activeStatuses = new Set(['starting', 'running', 'compacting', 'waiting_for_arch', 'arch_running', 'resuming', 'cancelling']);
const agentName = (id: string) => id === 'codex' ? 'Codex Lead' : id === 'antigravity' ? 'Antigravity Arch' : id;

type Props = {
  snapshot: Snapshot;
  probes: AgentProbe[];
  checking: boolean;
  probeError: string;
  onRefresh: () => void;
};

export function AgentsView({ snapshot, probes, checking, probeError, onRefresh }: Props) {
  const [selectedId, setSelectedId] = useState('codex');
  const [instances, setInstances] = useState<AgentInstance[]>([]);
  const [candidates, setCandidates] = useState<AgentCandidate[]>([]);
  const [adding, setAdding] = useState(false);
  const [provider, setProvider] = useState('claude_code');
  const [path, setPath] = useState('');
  const [role, setRole] = useState('lead');
  const [working, setWorking] = useState(false);
  const [registryError, setRegistryError] = useState('');
  const refreshInstances = async () => setInstances(await invoke<AgentInstance[]>('list_agent_instances'));
  useEffect(() => { void refreshInstances().catch(e => setRegistryError(String(e))); }, []);
  async function openAdd() {
    setAdding(true); setRegistryError('');
    try { const found = await invoke<AgentCandidate[]>('detect_agent_candidates'); setCandidates(found); setPath(found.find(c => c.provider === provider)?.path ?? ''); }
    catch (e) { setRegistryError(String(e)); }
  }
  async function register() {
    setWorking(true); setRegistryError('');
    try {
      const added = await invoke<AgentInstance>('register_agent_instance', { provider, path, role });
      await refreshInstances(); setSelectedId(added.id); setAdding(false); setPath('');
    } catch (e) { setRegistryError(String(e)); }
    finally { setWorking(false); }
  }
  async function unregister() {
    setWorking(true); setRegistryError('');
    try { await invoke('unregister_agent_instance', { id: selectedId }); await refreshInstances(); setSelectedId('codex'); }
    catch (e) { setRegistryError(String(e)); }
    finally { setWorking(false); }
  }
  const registered = instances.filter(item => !item.builtin);
  const selectedInstance = registered.find(item => item.id === selectedId);
  const selectedBuiltin = instances.find(item => item.builtin && item.id === selectedId);
  const selected = snapshot.agents.find(agent => agent.id === selectedId) ?? snapshot.agents[0];
  const displayName = selectedInstance?.name ?? (selected ? agentName(selected.id) : '');
  const probe = probes.find(item => item.id === selectedId);
  const sessionId = !selectedInstance && selected && activeStatuses.has(selected.status)
    ? snapshot.activeRun?.sessionId ?? snapshot.busySessionId : null;
  const runId = sessionId && snapshot.activeRun?.sessionId === sessionId ? snapshot.activeRun.id : null;
  const execution = selected && activeStatuses.has(selected.status) && !sessionId
    ? '상태 확인 필요' : executionLabels[selected?.status ?? 'unknown'] ?? selected?.status;
  const checkedAt = selectedInstance ? new Date(selectedInstance.checkedAt).toLocaleString('ko-KR') : probe ? new Date(probe.checkedAt).toLocaleString('ko-KR') : '점검 전';
  const status = (value: string | undefined) => checking ? '점검 중' : value ? probeLabels[value] ?? value : '점검 전';
  function refreshAll() { onRefresh(); void refreshInstances().catch(e => setRegistryError(String(e))); }

  return <main data-panel="agents" className="v0-main v0-agents" aria-label="에이전트 관리">
    <header className="v0-main-header v0-agents-header"><strong>에이전트</strong><div><button type="button" data-action="add-agent" onClick={() => void openAdd()}>에이전트 추가</button><button type="button" data-action="refresh-agents" onClick={refreshAll} disabled={checking} aria-label="에이전트 상태 새로고침">{checking ? '점검 중…' : '새로고침'}</button></div></header>
    {(probeError || registryError) && <p role="alert" className="v0-agents-error">{probeError || registryError}</p>}
    {adding && <section className="v0-agent-add" aria-label="에이전트 추가"><div className="v0-agent-add-head"><strong>에이전트 추가</strong><button type="button" onClick={() => setAdding(false)} aria-label="닫기">닫기</button></div><div className="v0-agent-candidates" aria-label="탐지 후보">{candidates.map(candidate => <button key={candidate.provider} type="button" data-candidate={candidate.provider} className={provider === candidate.provider ? 'selected' : ''} onClick={() => { setProvider(candidate.provider); setPath(candidate.path ?? ''); }}><span>{candidate.name}</span><small>{probeLabels[candidate.installation] ?? candidate.installation}</small></button>)}</div><label>종류<select aria-label="에이전트 종류" value={provider} onChange={e => { setProvider(e.target.value); setPath(candidates.find(c => c.provider === e.target.value)?.path ?? ''); }}><option value="claude_code">Claude Code</option><option value="gemini_cli">Gemini CLI</option><option value="opencode">OpenCode</option></select></label><label>실행 파일 경로<input aria-label="실행 파일 경로" value={path} onChange={e => setPath(e.target.value)} placeholder="C:\\...\\claude.exe"/></label><label>희망 역할<select aria-label="희망 역할" value={role} onChange={e => setRole(e.target.value)}><option value="lead">Lead</option><option value="reviewer">Reviewer</option></select></label><button type="button" data-action="register-agent" disabled={working || !path.trim()} onClick={() => void register()}>등록</button></section>}
    <div className="v0-agents-layout">
      <nav className="v0-agents-list" aria-label="에이전트 목록">
        {snapshot.agents.map(agent => {
          const item = probes.find(value => value.id === agent.id);
          const active = activeStatuses.has(agent.status);
          const owner = active ? snapshot.activeRun?.sessionId ?? snapshot.busySessionId : null;
          const currentRun = owner && snapshot.activeRun?.sessionId === owner ? snapshot.activeRun.id : null;
          return <button key={agent.id} type="button" className={selected?.id === agent.id ? 'selected' : ''} data-agent={agent.id} data-status={agent.status} data-installation={item?.installation ?? 'not_checked'} data-authentication={item?.authentication ?? 'not_checked'} data-availability={item?.availability ?? 'not_checked'} data-session-id={owner ?? ''} data-run-id={currentRun ?? ''} aria-current={selected?.id === agent.id ? 'true' : undefined} onClick={() => setSelectedId(agent.id)}>
            <span className={`v0-dot ${agent.id}`} aria-hidden="true"/><span>{agentName(agent.id)}</span><small>{executionLabels[agent.status] ?? agent.status}</small>
          </button>;
        })}
        {registered.map(item => <button key={item.id} type="button" className={selectedId === item.id ? 'selected' : ''} data-agent={item.id} data-provider={item.provider} data-support={item.support} data-installation={item.installation} data-authentication={item.authentication} data-availability={item.availability} data-status={item.execution} onClick={() => setSelectedId(item.id)}><span className="v0-dot" aria-hidden="true"/><span>{item.name}</span><small>미지원</small></button>)}
      </nav>
      {(selected || selectedInstance) && <section className="v0-agents-detail" aria-label={`${displayName} 상세 정보`}>
        <h2>{displayName}</h2>
        <dl className="v0-agents-facts">
          <div><dt>설치</dt><dd>{status(selectedInstance?.installation ?? probe?.installation)}</dd></div>
          <div><dt>인증</dt><dd>{status(selectedInstance?.authentication ?? probe?.authentication)}</dd></div>
          <div><dt>실행 가능</dt><dd>{status(selectedInstance?.availability ?? probe?.availability)}</dd></div>
          <div><dt>실행 상태</dt><dd>{selectedInstance ? executionLabels[selectedInstance.execution] ?? selectedInstance.execution : execution}</dd></div>
          <div><dt>연결 방식</dt><dd>{selectedInstance ? '설정 필요' : probe?.connection ?? '점검 전'}</dd></div>
          <div><dt>활성 역할</dt><dd data-active-role={selectedInstance?.activeRole ?? selectedBuiltin?.activeRole ?? ''}>{selectedInstance?.activeRole === 'lead' || selectedBuiltin?.activeRole === 'lead' ? 'Lead' : selectedInstance?.activeRole === 'reviewer' || selectedBuiltin?.activeRole === 'reviewer' ? 'Reviewer' : selectedInstance || selectedBuiltin ? '없음' : '확인 중'}</dd></div>
          {selectedInstance && <><div><dt>지원</dt><dd>미지원</dd></div><div><dt>종류</dt><dd>{selectedInstance.provider}</dd></div><div><dt>인스턴스 ID</dt><dd className="v0-agents-id">{selectedInstance.id}</dd></div><div><dt>희망 역할</dt><dd>{selectedInstance.role === 'lead' ? 'Lead' : 'Reviewer'}</dd></div><div><dt>실행 파일</dt><dd>{selectedInstance.path}</dd></div></>}
          <div><dt>점검 시각</dt><dd>{checkedAt}</dd></div>
          {sessionId && <div><dt>세션</dt><dd>{snapshot.sessions.find(item => item.id === sessionId)?.name ?? sessionId}<small title={sessionId}>{sessionId}</small></dd></div>}
          {runId && <div><dt>실행 ID</dt><dd className="v0-agents-id">{runId}</dd></div>}
        </dl>
        {(selectedInstance?.failureReason ?? probe?.failureReason) && <p className="v0-agent-failure">{selectedInstance?.failureReason ?? probe?.failureReason}</p>}
        {selectedInstance && <button type="button" data-action="unregister-agent" disabled={working} onClick={() => void unregister()}>등록 해제</button>}
      </section>}
    </div>
  </main>;
}
