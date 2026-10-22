import { useEffect, useRef, useState, type RefObject } from 'react';
import type { KeyboardEvent, PointerEvent } from 'react';
type Widths = { left: number; right: number };
type Side = keyof Widths;
const key = 'esh.panel-widths.v1';
const readableLayoutKey = 'esh.collaboration-readable-layout.v1';
const defaults: Widths = { left: 224, right: 520 };
export function usePanelWidths(root: RefObject<HTMLDivElement | null>) {
  const [widths, setWidths] = useState<Widths>(() => {
    try {
      const v = JSON.parse(localStorage.getItem(key) ?? 'null');
      const saved = v && Number.isFinite(v.left) && Number.isFinite(v.right) ? { left: Math.max(180, Math.min(420, v.left)), right: Math.max(340, Math.min(800, v.right)) } : defaults;
      if (localStorage.getItem(readableLayoutKey) === '1') return saved;
      localStorage.setItem(readableLayoutKey, '1');
      return { ...saved, right: Math.max(520, saved.right) };
    }
    catch { return defaults; }
  });
  const drag = useRef<{ side: Side; x: number; widths: Widths } | null>(null);
  const clamp = (value: Widths, priority: Side = 'left'): Widths => {
    const available = Math.max(520, (root.current?.clientWidth ?? window.innerWidth) - 360 - 12);
    let left = Math.max(180, Math.min(420, value.left)); let right = Math.max(340, Math.min(800, value.right));
    if (priority === 'left') left = Math.min(left, available - right); else right = Math.min(right, available - left);
    if (left + right > available) { left = Math.max(180, Math.min(left, available - 340)); right = available - left; }
    return { left, right };
  };
  useEffect(() => { const observer = new ResizeObserver(() => setWidths(prev => clamp(prev))); if (root.current) observer.observe(root.current); return () => observer.disconnect(); }, []);
  useEffect(() => { try { localStorage.setItem(key, JSON.stringify(widths)); } catch { /* Layout works without storage. */ } }, [widths]);
  function separator(side: Side) {
    return {
      role: 'separator' as const, tabIndex: 0, 'aria-orientation': 'vertical' as const,
      'aria-label': side === 'left' ? '탐색기 너비 조절' : '협업 타임라인 너비 조절',
      'aria-valuemin': side === 'left' ? 180 : 340, 'aria-valuemax': side === 'left' ? 420 : 800, 'aria-valuenow': Math.round(widths[side]),
      'data-resizer': side, className: 'panel-resizer',
      onPointerDown: (e: PointerEvent<HTMLDivElement>) => { if (e.button !== 0) return; e.preventDefault(); try { e.currentTarget.setPointerCapture(e.pointerId); } catch { /* Synthetic test events have no native pointer to capture. */ } drag.current = { side, x: e.clientX, widths }; document.body.classList.add('resizing'); },
      onPointerMove: (e: PointerEvent<HTMLDivElement>) => { const start = drag.current; if (!start || start.side !== side) return; const delta = e.clientX - start.x; setWidths(clamp({ ...start.widths, [side]: start.widths[side] + (side === 'left' ? delta : -delta) }, side)); },
      onPointerUp: () => { drag.current = null; document.body.classList.remove('resizing'); },
      onPointerCancel: () => { drag.current = null; document.body.classList.remove('resizing'); },
      onLostPointerCapture: () => { drag.current = null; document.body.classList.remove('resizing'); },
      onKeyDown: (e: KeyboardEvent<HTMLDivElement>) => { if (!['ArrowLeft', 'ArrowRight', 'Home'].includes(e.key)) return; e.preventDefault(); setWidths(prev => e.key === 'Home' ? clamp(defaults) : clamp({ ...prev, [side]: prev[side] + (e.key === 'ArrowRight' ? 16 : -16) * (side === 'left' ? 1 : -1) }, side)); },
    };
  }
  return { widths, separator };
}
