import type { SVGProps } from 'react';

type IconName = 'search' | 'plus' | 'chat' | 'bot' | 'folder' | 'archive' | 'settings' | 'menu' | 'panel' | 'chevrons' | 'chevronDown' | 'chevronRight' | 'arrowLeft' | 'clock' | 'check' | 'alert' | 'arrowUp' | 'paperclip' | 'stop' | 'link' | 'x';

const paths: Record<IconName, React.ReactNode> = {
  search: <><circle cx="11" cy="11" r="8"/><path d="m21 21-4.34-4.34"/></>,
  plus: <path d="M5 12h14M12 5v14"/>,
  chat: <path d="M22 17a2 2 0 0 1-2 2H6.8a2 2 0 0 0-1.4.6L3.2 21.8A.7.7 0 0 1 2 21.3V5a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2z"/>,
  bot: <><path d="M12 8V4H8M2 14h2m16 0h2M9 13v2m6-2v2"/><rect x="4" y="8" width="16" height="12" rx="2"/></>,
  folder: <path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.7-.9l-.8-1.2A2 2 0 0 0 7.9 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2z"/>,
  archive: <><rect x="2" y="3" width="20" height="5" rx="1"/><path d="M4 8v11a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8M10 12h4"/></>,
  settings: <><path d="M9.7 4.1a2.34 2.34 0 0 1 4.6 0 2.34 2.34 0 0 0 3.3 1.9 2.34 2.34 0 0 1 2.4 4 2.34 2.34 0 0 0 0 4 2.34 2.34 0 0 1-2.4 4 2.34 2.34 0 0 0-3.3 1.9 2.34 2.34 0 0 1-4.6 0 2.34 2.34 0 0 0-3.3-1.9 2.34 2.34 0 0 1-2.4-4 2.34 2.34 0 0 0 0-4 2.34 2.34 0 0 1 2.4-4 2.34 2.34 0 0 0 3.3-1.9"/><circle cx="12" cy="12" r="3"/></>,
  menu: <path d="M4 5h16M4 12h16M4 19h16"/>,
  panel: <><rect x="3" y="3" width="18" height="18" rx="2"/><path d="M15 3v18"/></>,
  chevrons: <path d="m6 17 5-5-5-5m7 10 5-5-5-5"/>,
  chevronDown: <path d="m6 9 6 6 6-6"/>,
  chevronRight: <path d="m9 18 6-6-6-6"/>,
  arrowLeft: <path d="m12 19-7-7 7-7M5 12h14"/>,
  clock: <><circle cx="12" cy="12" r="10"/><path d="M12 6v6h4"/></>,
  check: <><circle cx="12" cy="12" r="10"/><path d="m9 12 2 2 4-4"/></>,
  alert: <><circle cx="12" cy="12" r="10"/><path d="M12 8v4m0 4h.01"/></>,
  arrowUp: <path d="m5 12 7-7 7 7M12 19V5"/>,
  paperclip: <path d="m16 6-8.4 8.6a2 2 0 0 0 2.8 2.8l8.4-8.6a4 4 0 1 0-5.6-5.6l-8.4 8.5a6 6 0 1 0 8.5 8.5l8.4-8.5"/>,
  stop: <rect x="6" y="6" width="12" height="12" rx="2"/>,
  link: <><path d="M9 17H7A5 5 0 0 1 7 7h2m6 0h2a5 5 0 0 1 0 10h-2"/><path d="M8 12h8"/></>,
  x: <path d="M18 6 6 18M6 6l12 12"/>,
};

export function Icon({ name, ...props }: SVGProps<SVGSVGElement> & { name: IconName }) {
  return <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" {...props}>{paths[name]}</svg>;
}
