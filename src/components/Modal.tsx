import { useEffect, useId, useRef, type ReactNode } from 'react';
import { X } from 'lucide-react';

export default function Modal({ title, subtitle, onClose, children, wide = false, closable = true }: {
  title: string; subtitle?: string; onClose: () => void; children: ReactNode; wide?: boolean; closable?: boolean;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const titleId = useId();
  useEffect(() => {
    const element = dialog.current!;
    const previous = document.activeElement as HTMLElement | null;
    element.showModal();
    (element.querySelector('input, select, textarea') as HTMLElement | null)?.focus();
    return () => { element.close(); previous?.focus(); };
  }, []);
  return <dialog ref={dialog} aria-labelledby={titleId} className={`modal ${wide ? 'modal-wide' : ''}`} onCancel={e => { e.preventDefault(); if (closable) onClose(); }}
    onClick={e => { if (closable && e.target === e.currentTarget) { const rect = e.currentTarget.getBoundingClientRect(); if (e.clientX < rect.left || e.clientX > rect.right || e.clientY < rect.top || e.clientY > rect.bottom) onClose(); } }}>
    <header className="modal-header"><div><h2 id={titleId}>{title}</h2>{subtitle && <p>{subtitle}</p>}</div><button type="button" className="icon-button" aria-label="关闭弹窗" disabled={!closable} onClick={onClose}><X size={20} /></button></header>
    {children}
  </dialog>;
}
