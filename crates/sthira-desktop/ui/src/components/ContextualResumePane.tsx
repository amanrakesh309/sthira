import React from 'react';
import { ArrowRight, ChevronUp, Clock, Sparkles, Maximize2, Minimize2, FileText, Loader2 } from 'lucide-react';
import type { BriefClaim, ResumeBrief } from '../types/sthira';

interface ContextualResumePaneProps {
  brief: ResumeBrief;
  onExpandReview?: () => void;
  isExpandedReview?: boolean;
  isReviewLoading?: boolean;
  reviewError?: string | null;
  onSelectArtifact?: (artifactId: string) => void;
  isOpen: boolean;
  onToggleOpen: () => void;
}

const words = (text: string) => text.trim().split(/\s+/).filter(Boolean);
const shorten = (text: string, limit: number) => {
  const parts = words(text);
  return parts.length > limit ? `${parts.slice(0, Math.max(1, limit)).join(' ')}…` : text;
};
const originLabel = (claim: BriefClaim) => ({
  user_recorded: 'Recorded', source_backed: 'Source-backed', ai_inferred: 'AI inference',
}[claim.origin] || 'Unclassified');

export const ContextualResumePane: React.FC<ContextualResumePaneProps> = ({
  brief, onExpandReview, isExpandedReview = false, isReviewLoading = false,
  reviewError, onSelectArtifact, isOpen, onToggleOpen,
}) => {
  const breadcrumb = shorten(brief.active_working_breadcrumb || 'Active thread', 8);
  if (!isOpen) {
    return <button type="button" className="focus-brief-collapsed" onClick={onToggleOpen} aria-expanded={false}>
      <Sparkles size={16} /><span>Resume Brief</span><span className="focus-brief-collapsed-context">{breadcrumb}</span>
    </button>;
  }

  const progress = brief.recent_progress || [];
  const conflicts = brief.conflicting_decisions || [];
  // Reserve space for continuation and provenance. Count visible labels too, so
  // long backend text cannot turn Level 1 into an unbounded summary panel.
  const claims = isExpandedReview ? progress : progress.slice(0, 2);
  const visibleConflicts = isExpandedReview ? conflicts : conflicts.slice(0, 1);
  const allClaims = [...claims, ...visibleConflicts];
  const fixedCopy = `Contextual Resume Brief ${breadcrumb} ${onExpandReview ? 'Expand Review' : ''} Purpose Recent grounded findings ${visibleConflicts.length ? 'Conflicting decisions' : ''} ${brief.continuation_point ? `Continuation ${brief.primary_continuation_artifact_id && onSelectArtifact ? 'Resume' : ''}` : ''} ${claims.length === 0 ? 'No progress recorded yet.' : ''} ${allClaims.map((claim) => `${originLabel(claim)} ${(claim.artifact_ids?.length && onSelectArtifact) ? 'Source' : ''}`).join(' ')}`;
  const contentBudget = Math.max(12, 75 - words(fixedCopy).length);
  const purpose = isExpandedReview ? (brief.purpose || 'No purpose recorded.') : shorten(brief.purpose || 'No purpose recorded.', Math.min(14, Math.floor(contentBudget / 3)));
  const continuation = brief.continuation_point
    ? isExpandedReview ? brief.continuation_point : shorten(brief.continuation_point, Math.min(16, Math.floor(contentBudget / 3)))
    : null;
  let remaining = contentBudget - words(purpose).length - words(continuation || '').length;
  const claimTexts = allClaims.map((claim, index) => {
    if (isExpandedReview) return claim.text;
    const text = shorten(claim.text, Math.max(1, Math.floor(remaining / (allClaims.length - index))));
    remaining -= words(text).length;
    return text;
  });

  const renderClaim = (claim: BriefClaim, index: number) => (
    <li key={`${claim.text}-${index}`} className="focus-brief-claim">
      <span className="focus-brief-claim-text">{claimTexts[index]}</span>
      <span className="focus-brief-provenance" title={claim.rendering?.replace(/_/g, ' ')}>{originLabel(claim)}</span>
      {onSelectArtifact && (isExpandedReview ? claim.artifact_ids || [] : (claim.artifact_ids || []).slice(0, 1)).map((id, sourceIndex) => (
        <button type="button" key={id} className="focus-brief-source" onClick={() => onSelectArtifact(id)} aria-label={`Open source ${sourceIndex + 1} for ${claim.text}`}>
          <FileText size={13} />{isExpandedReview ? `Source ${sourceIndex + 1}` : 'Source'}
        </button>
      ))}
    </li>
  );

  return (
    <section className={`focus-brief ${isExpandedReview ? 'focus-brief-expanded' : ''}`} data-testid="contextual-resume-pane" aria-label={isExpandedReview ? 'Expanded project review' : 'Contextual resume brief'}>
      <div className="focus-brief-heading">
        <span className="focus-brief-icon"><Sparkles size={18} /></span>
        <div className="focus-brief-heading-copy">
          <h2>{isExpandedReview ? 'Expanded Review (Level 2)' : 'Contextual Resume Brief'}</h2>
          <p title={brief.active_working_breadcrumb}>{breadcrumb}</p>
        </div>
        {onExpandReview && <button type="button" className="focus-brief-review" onClick={onExpandReview} disabled={isReviewLoading}>
          {isReviewLoading ? <Loader2 size={14} className="animate-spin" /> : isExpandedReview ? <Minimize2 size={14} /> : <Maximize2 size={14} />}
          {isReviewLoading ? 'Loading review…' : isExpandedReview ? 'Compact' : 'Expand Review'}
        </button>}
        <button type="button" className="focus-icon-button" onClick={onToggleOpen} aria-label="Collapse Brief" aria-expanded={true}><ChevronUp size={16} /></button>
      </div>
      {reviewError && <p className="focus-error" role="alert">{reviewError}</p>}
      <div className="focus-brief-columns">
        <div className="focus-brief-purpose"><h3>Purpose</h3><p>{purpose}</p></div>
        <div className="focus-brief-progress"><h3>Recent grounded findings</h3>
          {claims.length > 0 ? <ul>{claims.map(renderClaim)}</ul> : <p className="focus-empty-copy">No progress recorded yet.</p>}
          {visibleConflicts.length > 0 && <div className="focus-brief-conflicts"><h3>Conflicting decisions</h3><ul>{visibleConflicts.map((claim, index) => renderClaim(claim, claims.length + index))}</ul></div>}
        </div>
      </div>
      {continuation && <div className="focus-brief-continuation">
        <Clock size={16} /><p><span>Continuation: </span>{continuation}</p>
        {brief.primary_continuation_artifact_id && onSelectArtifact && <button type="button" className="sthira-button-primary" onClick={() => onSelectArtifact(brief.primary_continuation_artifact_id!)}>Resume <ArrowRight size={15} /></button>}
      </div>}
    </section>
  );
};
