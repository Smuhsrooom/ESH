import { useEffect, useId, useRef, useState } from 'react';
import type { Context } from './types';

const labels: Record<string, string> = { not_observed: '압축 관측 전', compacting: '컨텍스트 압축 중…', completed: '압축 완료', failed: '압축 실패', cancelled: '압축 중단됨', unknown: '압축 결과 확인 필요' };
const number = (n: number) => n.toLocaleString('ko-KR');

export function ContextStatus({ context, onCompact, compactDisabled, compactReason }: { context?: Context; onCompact?: () => void; compactDisabled?: boolean; compactReason?: string }) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const popupId = useId();
  const usage = context?.usage;
  const window = usage?.modelContextWindow;
  const ratio = usage && window && !context?.usageStale ? usage.last.totalTokens / window * 100 : null;
  const percentage = ratio === null ? null : Math.max(0, Math.min(100, ratio));
  const compacting = context?.compaction === 'compacting';
  const usageText = ratio !== null ? `컨텍스트 약 ${ratio.toFixed(1)}%` : context?.usageStale ? '컨텍스트 갱신 대기' : '컨텍스트 사용량 대기';

  useEffect(() => {
    if (!open) return;
    const closeOutside = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(false);
    };
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setOpen(false);
        trigger.current?.focus();
      }
    };
    document.addEventListener('pointerdown', closeOutside);
    document.addEventListener('keydown', closeOnEscape);
    return () => {
      document.removeEventListener('pointerdown', closeOutside);
      document.removeEventListener('keydown', closeOnEscape);
    };
  }, [open]);

  return <div ref={root} className="context-control" aria-label="Codex 컨텍스트 상태" data-context-session={context?.sessionId} data-compaction={context?.compaction ?? 'not_observed'}>
    <button ref={trigger} type="button" data-action="context-usage" className="context-trigger" aria-label={`${usageText}, 상세 보기`} aria-expanded={open} aria-controls={popupId} title={usageText} onClick={() => setOpen(value => !value)}>
      <svg className="context-ring" viewBox="0 0 40 40" aria-hidden="true"><circle className="context-ring-track" cx="20" cy="20" r="16"/><circle className="context-ring-progress" cx="20" cy="20" r="16" pathLength="100" strokeDasharray={`${percentage ?? 0} 100`}/></svg>
      <span className="context-pct">{percentage === null ? '—' : `${Math.round(percentage)}%`}</span>
    </button>
    <div id={popupId} className="context-status context-popup" role="dialog" aria-label="컨텍스트 사용량 상세" hidden={!open}>
      <div className="context-summary"><span data-context-usage>{usageText}</span><span role="status">{labels[context?.compaction ?? 'not_observed'] ?? '압축 상태 확인 필요'}{context?.compactionCount ? ` · ${context.compactionCount}회` : ''}</span></div>
      {onCompact && <div className="context-actions"><button type="button" data-action="compact-context" className="context-compact" title={compactReason ?? 'Codex가 이 스레드의 컨텍스트를 압축해 요약본으로 바꿔. ESH 대화 기록은 그대로 남아.'} onClick={onCompact} disabled={compactDisabled || compacting}>{compacting ? '압축 중…' : '컨텍스트 압축'}</button>{compactReason && <small>{compactReason}</small>}</div>}
      {ratio !== null && <div className="context-meter" role="meter" aria-label="최근 요청 기준 컨텍스트 사용률" aria-valuemin={0} aria-valuemax={100} aria-valuenow={percentage ?? 0} aria-valuetext={`약 ${ratio.toFixed(1)}%`}><i style={{ width: `${percentage}%` }}/></div>}
      <details><summary>{usage ? `최근 ${number(usage.last.totalTokens)} / ${window ? number(window) : '한도 미제공'} 토큰` : '사용량 안내'}</summary>
        <p>Codex가 마지막으로 보고한 토큰 수 기준 추정치야. 현재 입력창과 대기 메시지는 포함하지 않아. 누적 토큰이나 계정 사용 한도와는 달라. 자동 압축 임계값은 별도로 제공되지 않아.</p>
        {usage && <p>최근 입력 {number(usage.last.inputTokens)} · 출력 {number(usage.last.outputTokens)} · 캐시 입력 {number(usage.last.cachedInputTokens)} · 누적 {number(usage.total.totalTokens)} 토큰<br/>최근 보고: {context?.usageUpdatedAt ? new Date(context.usageUpdatedAt).toLocaleString('ko-KR') : '없음'}{context?.usageStale ? ' · 이전 값, 갱신 대기' : ''}</p>}
        <p>{context?.compactedAt ? `마지막 압축 완료: ${new Date(context.compactedAt).toLocaleString('ko-KR')}` : '이 앱에서 압축 완료 이벤트를 아직 관측하지 않았어.'} 압축은 Codex가 수행하며 ESH 메시지 기록은 유지돼. 압축 버튼은 Codex의 thread/compact/start를 호출하고 완료 이벤트를 확인한 뒤에만 완료로 표시해.</p>
      </details>
    </div>
  </div>;
}
