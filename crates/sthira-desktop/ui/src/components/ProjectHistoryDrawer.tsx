import React, { useEffect, useRef } from 'react';
import { History, X, Clock, FileText } from 'lucide-react';
import { EpistemicBadge } from './EpistemicBadge';
import type { ProjectHistory, EpistemicOrigin } from '../types/sthira';

interface ProjectHistoryDrawerProps {
  history: ProjectHistory;
  isOpen: boolean;
  onClose: () => void;
  onSelectArtifact?: (artifactId: string) => void;
}

export const ProjectHistoryDrawer: React.FC<ProjectHistoryDrawerProps> = ({
  history, isOpen, onClose, onSelectArtifact,
}) => {
  const drawerRef = useRef<HTMLElement>(null);
  const closeRef = useRef(onClose);
  closeRef.current = onClose;
  useEffect(() => {
    if (!isOpen) return;
    const previous = document.activeElement as HTMLElement | null;
    drawerRef.current?.querySelector<HTMLButtonElement>('button')?.focus();
    const handleKeys = (event: KeyboardEvent) => {
      if (event.key === 'Escape') { event.preventDefault(); closeRef.current(); }
      if (event.key !== 'Tab') return;
      const controls = drawerRef.current?.querySelectorAll<HTMLElement>('button:not(:disabled), a[href], summary, [tabindex="0"]');
      if (!controls?.length) return;
      const first = controls[0];
      const last = controls[controls.length - 1];
      if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus(); }
      else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
    };
    document.addEventListener('keydown', handleKeys);
    return () => {
      document.removeEventListener('keydown', handleKeys);
      if (previous?.isConnected) previous.focus();
    };
  }, [isOpen]);
  if (!isOpen) return null;
  const segments = history.narrative_segments || [];
  const milestones = history.milestones || [];

  return (
    <div className="focus-history-layer">
      <div className="focus-history-backdrop" onClick={onClose} aria-hidden="true" />
      <aside ref={drawerRef} className="focus-history" role="dialog" aria-modal="true" aria-labelledby="focus-history-title">
        <header className="focus-history-heading">
          <span className="focus-brief-icon"><History size={20} /></span>
          <div><h2 id="focus-history-title">Project History</h2><p>Evolution narrative and audited milestones</p></div>
          <button type="button" onClick={onClose} aria-label="Close Drawer" className="focus-icon-button"><X size={18} /></button>
        </header>
        <div className="focus-history-content">
          <section aria-labelledby="focus-narrative-title">
            <h3 id="focus-narrative-title">Evolution narrative</h3>
            {segments.length === 0 && <p className="focus-empty-copy">No evolution narrative recorded yet.</p>}
            {segments.map((segment, index) => (
              <article key={index} className="focus-history-segment" data-inferred={segment.is_ai_interpretation}>
                <span>{segment.is_ai_interpretation ? 'AI Interpretation' : segment.origin === 'source_backed' ? 'Source-backed segment' : 'Recorded Segment'}</span>
                <p>{segment.content}</p>
              </article>
            ))}
          </section>
          <section aria-labelledby="focus-milestones-title">
            <h3 id="focus-milestones-title">Milestone timeline</h3>
            {milestones.length === 0 && <p className="focus-empty-copy">No milestones recorded yet.</p>}
            <ol className="focus-history-timeline">
              {milestones.map((milestone) => {
                const origin: EpistemicOrigin = milestone.attribution === 'you_recorded' ? 'user_recorded' : milestone.attribution === 'supported_by_source' ? 'source_backed' : 'ai_inferred';
                return <li key={milestone.id || milestone.seq}>
                  <div className="focus-history-milestone-meta"><EpistemicBadge origin={origin} /><span title={new Date(milestone.occurred_at).toLocaleString()}><Clock size={13} />{new Date(milestone.occurred_at).toLocaleDateString(undefined, { month: 'short', day: 'numeric' })}</span></div>
                  <p>{milestone.title}</p>
                  {milestone.description && <p className="focus-history-description">{milestone.description}</p>}
                  {!!milestone.evidence_ids?.length && <details className="focus-history-evidence"><summary>Evidence references ({milestone.evidence_ids.length})</summary><ul>{milestone.evidence_ids.map((id) => <li key={id}><code>{id}</code></li>)}</ul></details>}
                  {milestone.artifact_id && onSelectArtifact && <button type="button" className="focus-history-open" onClick={() => onSelectArtifact(milestone.artifact_id!)}><FileText size={15} />View artifact</button>}
                </li>;
              })}
            </ol>
          </section>
        </div>
      </aside>
    </div>
  );
};
