import { useEffect, useRef, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { X } from 'lucide-react';
import ReactMarkdown, { defaultUrlTransform } from 'react-markdown';
import remarkGfm from 'remark-gfm';
interface Props {
  operation?: 'edit' | 'create';
  heading?: string; currentLabel?:string; proposedLabel?:string; applyLabel?:string; footnote?:string; children?:ReactNode;
  current: ReactNode; proposed: ReactNode; summary: string; stale: boolean;
  pending: boolean; busy: boolean; error: string | null; researchCitations?: boolean;
  onClose: () => void; onApply: () => void; onDismiss: () => void; onOpenArtifact?: (id: string) => void;
}
/** Read-only review is outside the chat rail and outside the editable document. */
export function DocumentEditReview({ operation = 'edit', heading, currentLabel, proposedLabel, applyLabel, footnote, children, current, proposed, summary, stale, pending, busy, error, researchCitations = false, onClose, onApply, onDismiss, onOpenArtifact }: Props) {
  const dialog = useRef<HTMLDivElement>(null);
  const close = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    const opener = document.activeElement as HTMLElement | null;
    const app = document.getElementById('root');
    const wasInert = app?.inert;
    const overflow = document.body.style.overflow;
    if (app) app.inert = true;
    document.body.style.overflow = 'hidden';
    close.current?.focus();
    return () => {
      if (app) app.inert = wasInert || false;
      document.body.style.overflow = overflow;
      opener?.focus();
    };
  }, []);
  return createPortal(<div className="document-edit-review-layer" onMouseDown={e => { if (e.target === e.currentTarget && !pending) onClose(); }}>
    <div ref={dialog} role="dialog" aria-modal="true" aria-labelledby="document-edit-review-title" className="document-edit-review-dialog"
      onKeyDown={e => {
        if (e.key === 'Escape') { e.stopPropagation(); if (!pending) onClose(); }
        if (e.key === 'Tab') {
          const targets = dialog.current?.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), a[href], [tabindex="0"]');
          if (!targets?.length) return;
          const first = targets[0], last = targets[targets.length - 1];
          if (e.shiftKey && document.activeElement === first) { e.preventDefault(); last.focus(); }
          else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first.focus(); }
        }
      }}>
      <header><div><p>{heading || (operation === 'create' ? 'Suggested new page' : 'Suggested document edit')}</p><h2 id="document-edit-review-title">Review changes</h2></div><button ref={close} type="button" className="focus-icon-button" aria-label="Back to conversation" onClick={onClose} disabled={pending}><X size={20} /></button></header>
      <div className="document-edit-review-summary"><ReactMarkdown remarkPlugins={[remarkGfm]}>{summary}</ReactMarkdown></div>
      {children && <div className="document-edit-review-controls">{children}</div>}
      {(error || stale) && <p role="alert" className="document-chat-error">{error || 'Your document has changed. Ask for an updated suggestion before applying.'}</p>}
      <div className="document-edit-review-columns">
        {([[currentLabel || (operation === 'create' ? 'Before creation' : 'Current document'), current], [proposedLabel || (operation === 'create' ? 'Proposed page' : 'Suggested document'), proposed]] as const).map(([label, markdown]) => <section key={label} aria-label={label}>
          <h3>{label}</h3><div className="document-edit-review-prose" tabIndex={0}>
            {typeof markdown==='string'?<ReactMarkdown remarkPlugins={[remarkGfm]} urlTransform={url=>url.startsWith('sthira://artifact/')||url.startsWith('sthira://project/')?url:defaultUrlTransform(url)} components={{ table: ({children}) => <div className="document-edit-review-table"><table>{children}</table></div>, a: ({children, href}) => { if(href?.startsWith('sthira://artifact/')&&onOpenArtifact)return <button type="button" className="agent-link" disabled={pending} onClick={()=>{onClose();onOpenArtifact(href.slice('sthira://artifact/'.length));}}>{children}</button>; const citation=researchCitations&&Boolean(href?.startsWith('#research-source-')); return <a href={href} target={citation?undefined:'_blank'} rel="noreferrer" className={citation?'research-citation-link':undefined} onClick={citation?event=>{event.preventDefault();const target=dialog.current?.querySelector<HTMLElement>(String(href));target?.scrollIntoView({block:'center'});target?.focus();}:undefined}>{children}</a>; } }}>{markdown}</ReactMarkdown>:markdown}
          </div>
        </section>)}
      </div>
      <footer><p>{footnote || (operation === 'create' ? 'A new page is created only when you accept this proposal.' : 'Your document changes only when you apply this suggestion.')}</p><div><button type="button" disabled={pending} className="sthira-button-secondary" onClick={onDismiss}>Dismiss</button><button type="button" className="sthira-button-primary" disabled={pending || busy || stale} onClick={onApply}>{pending ? 'Applying…' : applyLabel || (operation === 'create' ? 'Create page' : 'Apply edit')}</button></div></footer>
    </div>
  </div>, document.body);
}
