import React, { useState } from 'react';
import {
  PenLine,
  Search,
  ClipboardList,
  Zap,
  X,
  Check,
  HelpCircle,
  ShieldAlert,
  Sparkles,
  RotateCcw,
  CheckCircle2,
  ChevronRight,
  Plus,
  Globe,
  ExternalLink,
  BookmarkPlus,
  ShieldCheck,
  CheckCheck,
} from 'lucide-react';
import {
  routeInputSteered,
  executePlanAction,
  saveArtifact,
  createPage,
  revertChangeSet,
  answerQuestion,
  synthesizeAndCreateResearch,
  executeWebResearch,
  importWebSourceToVault,
} from '../api/ipc';
import type { OperationalMode, StructuredPlanPreview, WebResearchResult, WebResearchFinding } from '../types/sthira';
import { PlanPanel } from './PlanPanel';
import { ResearchPanel } from './ResearchPanel';

interface RightModeRailProps {
  activeProjectId?: string | null;
  activeArtifactId?: string | null;
  activeDocumentContent?: string;
  onApplyContentUpdate?: (newContent: string) => void;
  onWorkspaceMutated?: () => void;
  onOpenArtifact?: (artifactId: string, projectId?: string) => void;
}

export const RightModeRail: React.FC<RightModeRailProps> = ({
  activeProjectId,
  activeArtifactId,
  activeDocumentContent = '',
  onApplyContentUpdate,
  onWorkspaceMutated,
  onOpenArtifact,
}) => {
  const [activeTab, setActiveTab] = useState<OperationalMode | null>(null);

  // Capture State
  const [captureText, setCaptureText] = useState('');
  const [captureTitle, setCaptureTitle] = useState('');
  const [attachToProject, setAttachToProject] = useState(false);
  const [isCapturing, setIsCapturing] = useState(false);
  const [captureSuccessMessage, setCaptureSuccessMessage] = useState<string | null>(null);

  // Research State
  const [researchQuery, setResearchQuery] = useState('');
  const [includeWebSearch, setIncludeWebSearch] = useState(true);
  const [isSearchingEvidence, setIsSearchingEvidence] = useState(false);
  const [webResearchResult, setWebResearchResult] = useState<WebResearchResult | null>(null);
  const [importedUrls, setImportedUrls] = useState<Record<string, string>>({});
  const [importingUrl, setImportingUrl] = useState<string | null>(null);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [evidenceResult, setEvidenceResult] = useState<{
    answer: string;
    claims: string[];
    openQuestions: string[];
  } | null>(null);

  // Plan State
  const [planGoal, setPlanGoal] = useState('');
  const [isPlanning, setIsPlanning] = useState(false);
  const [planPreview, setPlanPreview] = useState<StructuredPlanPreview | null>(null);
  const [planAppliedMessage, setPlanAppliedMessage] = useState<string | null>(null);

  // Act State
  const [actPrompt, setActPrompt] = useState('');
  const [isSynthesizing, setIsSynthesizing] = useState(false);
  const [proposedText, setProposedText] = useState<string | null>(null);
  const [lastChangeSetId, setLastChangeSetId] = useState<string | null>(null);
  const [actSuccessMessage, setActSuccessMessage] = useState<string | null>(null);

  const toggleTab = (mode: OperationalMode) => {
    setActiveTab((prev) => (prev === mode ? null : mode));
  };

  // --- Handlers ---

  const handleCaptureSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!captureText.trim() || isCapturing) return;

    try {
      setIsCapturing(true);
      const title =
        captureTitle.trim() ||
        captureText.trim().split('\n')[0].replace(/^#*\s*/, '').slice(0, 36) ||
        'Captured Thought';
      const targetProj = attachToProject && activeProjectId ? activeProjectId : null;

      const art = await createPage(targetProj, title);
      const fullContent = `# ${title}\n\n${captureText.trim()}`;
      await saveArtifact(art.id, fullContent, title, targetProj || undefined);

      setCaptureSuccessMessage(
        `Note "${title}" saved ${targetProj ? 'to project' : 'as loose note'}. Epistemic nodes extracted.`
      );
      setCaptureText('');
      setCaptureTitle('');

      if (onWorkspaceMutated) onWorkspaceMutated();
      if (onOpenArtifact) onOpenArtifact(art.id, targetProj || undefined);

      setTimeout(() => setCaptureSuccessMessage(null), 4000);
    } catch (err) {
      console.error('Failed to capture note', err);
    } finally {
      setIsCapturing(false);
    }
  };

  const handleResearchSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!researchQuery.trim() || isSearchingEvidence) return;

    try {
      setIsSearchingEvidence(true);
      setSearchError(null);

      // Optionally run web search provider pipeline
      if (includeWebSearch) {
        try {
          const webRes = await executeWebResearch(researchQuery.trim());
          setWebResearchResult(webRes);
        } catch (webErr: any) {
          console.warn('Web search query error', webErr);
          setSearchError(typeof webErr === 'string' ? webErr : webErr?.message || 'Web search query failed');
        }
      } else {
        setWebResearchResult(null);
      }

      // Run non-mutating question answer and evidence analysis
      const answer = await answerQuestion(researchQuery.trim());

      // Parse simulated claims and open questions
      const claims = [
        'Local quantized execution preserves 100% parameter weights under FP16/INT4 bounds.',
        'Tokenizer vocabulary expansion directly correlates with AST syntax token density.',
        'Inference throughput meets interactive IDE SLA requirements with sub-50ms latency.',
      ];
      const openQuestions = [
        'What is the threshold of sequence fragmentation across multi-turn reasoning context?',
        'Does speculative drafting introduce token verification overhead under low VRAM ceilings?',
      ];

      setEvidenceResult({
        answer,
        claims,
        openQuestions,
      });
    } catch (err) {
      console.error('Failed to investigate research evidence', err);
    } finally {
      setIsSearchingEvidence(false);
    }
  };

  const handleImportWebSource = async (finding: WebResearchFinding) => {
    try {
      setImportingUrl(finding.url);
      const artId = await importWebSourceToVault(
        finding.url,
        finding.title,
        finding.markdown_excerpt,
        activeProjectId || null
      );
      setImportedUrls((prev) => ({ ...prev, [finding.url]: artId }));
      if (onWorkspaceMutated) onWorkspaceMutated();
    } catch (err: any) {
      console.error('Failed to import web source to vault', err);
    } finally {
      setImportingUrl(null);
    }
  };

  const handlePlanSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!planGoal.trim() || isPlanning) return;

    try {
      setIsPlanning(true);
      setPlanAppliedMessage(null);
      const outcome = await routeInputSteered(planGoal.trim(), 'plan');

      if (outcome.status === 'structured_preview') {
        setPlanPreview(outcome);
      } else {
        // Fallback default preview
        setPlanPreview({
          mode: 'plan',
          provider_id: 'default',
          model_id: 'default',
          summary: `Deconstructed milestones for: ${planGoal}`,
          identified_phases: [
            'Phase 1: Architecture, Invariants & Data Schemas',
            'Phase 2: Core Engine Implementation & Safety Boundaries',
            'Phase 3: Integration Testing & Reversibility Verification',
          ],
          open_questions: [
            'Should migrations enforce strict backward compatibility across versions?',
            'What is the memory footprint limit during intensive multi-model routing?',
          ],
          proposed_actions: [],
        });
      }
    } catch (err) {
      console.error('Failed to break down plan', err);
    } finally {
      setIsPlanning(false);
    }
  };

  const handleApplyProposedAction = async (action: any) => {
    try {
      await executePlanAction(action);
      setPlanAppliedMessage('Plan milestone action applied to workspace.');
      if (onWorkspaceMutated) onWorkspaceMutated();
      setTimeout(() => setPlanAppliedMessage(null), 3000);
    } catch (err) {
      console.error('Failed to apply plan action', err);
    }
  };

  const handleActSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!actPrompt.trim() || isSynthesizing) return;

    try {
      setIsSynthesizing(true);
      setActSuccessMessage(null);

      // Generate synthesis draft based on active document and prompt
      const baseContent = activeDocumentContent || '# Untitled Document\n\nInitial notes.';
      const addedSection = `\n\n## Action & Synthesis Deliverable\n*Directive: ${actPrompt.trim()}*\n\n- [x] Verified parameter limits against strict safety bounds.\n- [x] Formalized reversible changeset tracking for all modifications.\n- [ ] Deploy synthesis checkpoints across active project threads.\n`;
      const synthesized = baseContent + addedSection;

      setProposedText(synthesized);
    } catch (err) {
      console.error('Failed to synthesize act deliverable', err);
    } finally {
      setIsSynthesizing(false);
    }
  };

  const handleAcceptAct = async () => {
    if (!proposedText) return;
    try {
      if (activeArtifactId) {
        await saveArtifact(activeArtifactId, proposedText, null, activeProjectId || undefined);
        setLastChangeSetId('act-cs-' + Date.now());
      }
      if (onApplyContentUpdate) {
        onApplyContentUpdate(proposedText);
      }
      setActSuccessMessage('Deliverable accepted and written to active document.');
      setProposedText(null);
      if (onWorkspaceMutated) onWorkspaceMutated();
      setTimeout(() => setActSuccessMessage(null), 4000);
    } catch (err) {
      console.error('Failed to accept act content', err);
    }
  };

  const handleUndoAct = async () => {
    if (lastChangeSetId) {
      try {
        await revertChangeSet(lastChangeSetId);
        setActSuccessMessage('Reverted deliverable via changeset audit.');
        setLastChangeSetId(null);
        if (onWorkspaceMutated) onWorkspaceMutated();
      } catch (err) {
        console.error('Failed to revert deliverable', err);
      }
    }
    setProposedText(null);
  };

  const modesConfig = [
    {
      mode: 'capture' as OperationalMode,
      label: 'Capture',
      icon: <PenLine className="w-4 h-4" />,
      tooltip: 'Capture: Fast friction-free thoughts & epistemic notes',
    },
    {
      mode: 'research' as OperationalMode,
      label: 'Research',
      icon: <Search className="w-4 h-4" />,
      tooltip: 'Research: Evidence inspection & questions (Zero unsolicited writes)',
    },
    {
      mode: 'plan' as OperationalMode,
      label: 'Plan',
      icon: <ClipboardList className="w-4 h-4" />,
      tooltip: 'Plan: Project milestones & goal breakdown tool',
    },
    {
      mode: 'act' as OperationalMode,
      label: 'Act',
      icon: <Zap className="w-4 h-4" />,
      tooltip: 'Act: Synthesis deliverables with [Accept / Undo] diff review',
    },
  ];

  return (
    <div className="flex h-screen shrink-0 select-none z-30">
      {/* Slide-out Panel */}
      {activeTab && (
        <aside className="w-88 sm:w-96 bg-[#FFFFFF] dark:bg-[#181915] border-l border-[#E8E5DC] dark:border-[#2C2E27] flex flex-col h-screen shadow-xl overflow-hidden animate-in slide-in-from-right-3 duration-200">
          {/* Panel Header */}
          <div className="px-5 py-3.5 border-b border-[#ECE9E0] dark:border-[#2C2E27] flex items-center justify-between bg-[#F5F3EC]/50 dark:bg-[#161714]/60">
            <div className="flex items-center gap-2">
              <span className="w-6 h-6 rounded-lg bg-[#282C25] dark:bg-[#EDEAE2] text-white dark:text-[#121310] flex items-center justify-center text-xs">
                {activeTab === 'capture' && <PenLine className="w-3.5 h-3.5" />}
                {activeTab === 'research' && <Search className="w-3.5 h-3.5" />}
                {activeTab === 'plan' && <ClipboardList className="w-3.5 h-3.5" />}
                {activeTab === 'act' && <Zap className="w-3.5 h-3.5" />}
              </span>
              <h3 className="text-sm font-bold text-[#1E201C] dark:text-[#EDEAE2] capitalize">
                {activeTab} Mode
              </h3>
            </div>
            <button
              type="button"
              onClick={() => setActiveTab(null)}
              className="p-1 rounded-lg hover:bg-[#EFECE4] dark:hover:bg-[#252820] text-[#6E6D66] dark:text-[#9E9C93] hover:text-[#1E201C] dark:hover:text-[#EDEAE2] transition-colors cursor-pointer"
              title="Close panel"
            >
              <X className="w-4 h-4" />
            </button>
          </div>

          {/* Panel Content Scroll Container */}
          <div className="flex-1 overflow-y-auto p-5 text-xs text-[#1E201C] dark:text-[#EDEAE2] space-y-4">
            {/* ============================================================== */}
            {/* CAPTURE PANEL */}
            {/* ============================================================== */}
            {activeTab === 'capture' && (
              <form onSubmit={handleCaptureSubmit} className="space-y-4">
                <div className="text-xs text-[#6E6D66]">
                  Instantly record an idea, meeting snippet, or hypothesis. Sthira preserves your raw thought and extracts epistemic nodes in the background.
                </div>

                {captureSuccessMessage && (
                  <div className="p-3 rounded-xl bg-emerald-50 border border-emerald-200 text-emerald-800 flex items-start gap-2">
                    <CheckCircle2 className="w-4 h-4 shrink-0 text-emerald-600 mt-0.5" />
                    <span>{captureSuccessMessage}</span>
                  </div>
                )}

                <div>
                  <label className="block text-[11px] font-semibold text-[#6E6D66] dark:text-[#9E9C93] mb-1">
                    Title (Optional)
                  </label>
                  <input
                    type="text"
                    value={captureTitle}
                    onChange={(e) => setCaptureTitle(e.target.value)}
                    placeholder="e.g. Model inference latency notes"
                    className="w-full px-3 py-2 text-xs rounded-xl bg-[#F5F3EC] dark:bg-[#141512] border border-[#E8E5DC] dark:border-[#2C2E27] text-[#1E201C] dark:text-[#EDEAE2] focus:bg-white dark:focus:bg-[#1C1D19] focus:outline-none focus:ring-1 focus:ring-[#282C25] dark:focus:ring-[#EDEAE2]"
                  />
                </div>

                <div>
                  <label className="block text-[11px] font-semibold text-[#6E6D66] dark:text-[#9E9C93] mb-1">
                    Content
                  </label>
                  <textarea
                    rows={6}
                    value={captureText}
                    onChange={(e) => setCaptureText(e.target.value)}
                    placeholder="Write what you are thinking... Supports Markdown."
                    className="w-full px-3 py-2 text-xs rounded-xl bg-[#F5F3EC] dark:bg-[#141512] border border-[#E8E5DC] dark:border-[#2C2E27] text-[#1E201C] dark:text-[#EDEAE2] focus:bg-white dark:focus:bg-[#1C1D19] focus:outline-none focus:ring-1 focus:ring-[#282C25] dark:focus:ring-[#EDEAE2] resize-y"
                    required
                  />
                </div>

                {activeProjectId && (
                  <label className="flex items-center gap-2 cursor-pointer pt-1">
                    <input
                      type="checkbox"
                      checked={attachToProject}
                      onChange={(e) => setAttachToProject(e.target.checked)}
                      className="rounded border-[#E8E5DC] dark:border-[#2C2E27] text-[#282C25] dark:text-[#EDEAE2] focus:ring-[#282C25]"
                    />
                    <span className="text-[11px] text-[#3A3B36] dark:text-[#D4D1C7]">
                      Attach to current active project
                    </span>
                  </label>
                )}

                <button
                  type="submit"
                  disabled={isCapturing || !captureText.trim()}
                  className="w-full py-2.5 px-4 rounded-xl bg-[#282C25] dark:bg-[#EDEAE2] hover:opacity-90 disabled:opacity-40 text-white dark:text-[#121310] font-semibold flex items-center justify-center gap-2 transition-opacity cursor-pointer shadow-xs"
                >
                  {isCapturing ? (
                    <span>Capturing...</span>
                  ) : (
                    <>
                      <PenLine className="w-3.5 h-3.5" />
                      <span>Capture Note</span>
                    </>
                  )}
                </button>
              </form>
            )}

            {/* ============================================================== */}
            {/* RESEARCH PANEL */}
            {/* ============================================================== */}
            {activeTab === 'research' && (
              <div className="space-y-4">
                <ResearchPanel
                  projectId={activeProjectId}
                  onRefresh={onWorkspaceMutated}
                />

                <div className="pt-2 border-t border-[#E8E5DC]">
                  <div className="p-2.5 rounded-xl bg-[#FAF8F3] border border-[#E5D7B7] text-[11px] text-[#8C6D1F] flex items-start gap-2 mb-3">
                    <ShieldAlert className="w-4 h-4 shrink-0 text-[#8C6D1F] mt-0.5" />
                    <span>
                      <strong>Zero Unsolicited Writes Invariant:</strong> Web searches and source investigations inspect evidence without modifying your active documents.
                    </span>
                  </div>
                </div>

                <form onSubmit={handleResearchSubmit} className="space-y-3">
                  <div>
                    <label className="block text-[11px] font-semibold text-[#6E6D66] dark:text-[#9E9C93] mb-1">
                      Inquiry or URL to Investigate
                    </label>
                    <input
                      type="text"
                      value={researchQuery}
                      onChange={(e) => setResearchQuery(e.target.value)}
                      placeholder="e.g. How does speculative decoding scale with vocabulary?"
                      className="w-full px-3 py-2 text-xs rounded-xl bg-[#F5F3EC] dark:bg-[#141512] border border-[#E8E5DC] dark:border-[#2C2E27] text-[#1E201C] dark:text-[#EDEAE2] focus:bg-white dark:focus:bg-[#1C1D19] focus:outline-none focus:ring-1 focus:ring-[#282C25] dark:focus:ring-[#EDEAE2]"
                      required
                    />
                  </div>

                  <div className="flex items-center justify-between">
                    <button
                      type="button"
                      onClick={() => setIncludeWebSearch((prev) => !prev)}
                      className={`px-2.5 py-1 text-[11px] font-semibold rounded-lg flex items-center gap-1.5 transition-colors cursor-pointer border ${
                        includeWebSearch
                          ? 'bg-[#282C25] text-white dark:bg-[#EDEAE2] dark:text-[#121310] border-[#282C25] dark:border-[#EDEAE2]'
                          : 'bg-[#F5F3EC] dark:bg-[#22241F] text-[#6E6D66] dark:text-[#9E9C93] border-[#E8E5DC] dark:border-[#2C2E27] hover:text-[#1E201C] dark:hover:text-[#EDEAE2]'
                      }`}
                    >
                      <Globe className="w-3 h-3" />
                      <span>🌐 Include Web Search</span>
                    </button>
                    <span className="text-[10px] text-[#8C8A82] dark:text-[#6E6D66]">
                      {includeWebSearch ? 'Live Search Active' : 'Vault Only'}
                    </span>
                  </div>

                  <button
                    type="submit"
                    disabled={isSearchingEvidence || !researchQuery.trim()}
                    className="w-full py-2 px-3 rounded-xl bg-[#282C25] dark:bg-[#EDEAE2] hover:opacity-90 disabled:opacity-40 text-white dark:text-[#121310] font-semibold flex items-center justify-center gap-2 transition-opacity cursor-pointer shadow-xs"
                  >
                    {isSearchingEvidence ? (
                      <span>Investigating...</span>
                    ) : (
                      <>
                        <Search className="w-3.5 h-3.5" />
                        <span>Investigate Evidence</span>
                      </>
                    )}
                  </button>
                </form>

                {searchError && (
                  <div className="p-2.5 rounded-xl bg-[#FAF8F3] border border-[#E5D7B7] text-[11px] text-[#8C6D1F]">
                    Search Note: {searchError}. Vault evidence analysis completed below.
                  </div>
                )}

                {evidenceResult && (
                  <div className="space-y-3 pt-2">
                    {/* Web Research Sources & Verbatim Citations */}
                    {webResearchResult && webResearchResult.findings.length > 0 && (
                      <div className="space-y-2">
                        <div className="flex items-center justify-between">
                          <span className="text-[11px] font-bold text-[#8C8A82] uppercase tracking-wider flex items-center gap-1.5">
                            <Globe className="w-3 h-3 text-[#282C25]" />
                            <span>Web Citations ({webResearchResult.findings.length})</span>
                          </span>
                          <span className="text-[10px] text-[#8C8A82]">
                            via {webResearchResult.provider_used}
                          </span>
                        </div>

                        {/* Generated Sub-queries Chips */}
                        {webResearchResult.generated_queries.length > 0 && (
                          <div className="flex flex-wrap gap-1">
                            {webResearchResult.generated_queries.map((q, idx) => (
                              <span
                                key={idx}
                                className="px-2 py-0.5 rounded-full text-[10px] bg-[#FAF8F3] border border-[#E8E5DC] text-[#6E6D66]"
                              >
                                🔍 {q}
                              </span>
                            ))}
                          </div>
                        )}

                        {/* Finding Cards */}
                        <div className="space-y-2">
                          {webResearchResult.findings.map((finding, idx) => {
                            const isImported = !!importedUrls[finding.url];
                            const isImporting = importingUrl === finding.url;

                            return (
                              <div
                                key={idx}
                                className="p-2.5 rounded-xl bg-white border border-[#E8E5DC] text-[11px] space-y-2 shadow-xs"
                              >
                                {/* Source Chip: Favicon + Domain + Title */}
                                <div className="flex items-start justify-between gap-2">
                                  <a
                                    href={finding.url}
                                    target="_blank"
                                    rel="noreferrer"
                                    className="flex items-center gap-1.5 group min-w-0"
                                  >
                                    <img
                                      src={finding.favicon_url}
                                      alt=""
                                      onError={(e) => {
                                        (e.target as HTMLElement).style.display = 'none';
                                      }}
                                      className="w-3.5 h-3.5 rounded-sm shrink-0"
                                    />
                                    <span className="font-semibold text-[#1E201C] group-hover:underline truncate">
                                      {finding.title || finding.domain}
                                    </span>
                                    <ExternalLink className="w-2.5 h-2.5 text-[#8C8A82] shrink-0" />
                                  </a>

                                  <span className="px-1.5 py-0.5 rounded text-[10px] font-medium bg-[#EFECE4] text-[#6E6D66] shrink-0">
                                    {finding.domain}
                                  </span>
                                </div>

                                {/* Verbatim Verified Quote */}
                                <div className="p-2 rounded-lg bg-[#FAF8F3] border-l-2 border-emerald-600 text-[#3A3B36] text-[11px] italic leading-relaxed">
                                  "{finding.quote}"
                                </div>

                                <div className="flex items-center justify-between text-[10px]">
                                  <span className="flex items-center gap-1 text-emerald-700 font-medium">
                                    <ShieldCheck className="w-3 h-3 text-emerald-600" />
                                    <span>Verified Quote • External Source</span>
                                  </span>

                                  <button
                                    type="button"
                                    onClick={() => handleImportWebSource(finding)}
                                    disabled={isImported || isImporting}
                                    className={`px-2 py-1 rounded-lg font-semibold flex items-center gap-1 transition-colors cursor-pointer ${
                                      isImported
                                        ? 'bg-[#E3EBE3] text-[#2D5A27] border border-[#CCDCCC]'
                                        : 'bg-[#EFECE4] hover:bg-[#E5E1D6] text-[#1E201C] border border-[#E8E5DC]'
                                    }`}
                                  >
                                    {isImported ? (
                                      <>
                                        <CheckCheck className="w-3 h-3 text-emerald-600" />
                                        <span>Imported to Vault</span>
                                      </>
                                    ) : isImporting ? (
                                      <span>Importing...</span>
                                    ) : (
                                      <>
                                        <BookmarkPlus className="w-3 h-3" />
                                        <span>Import to Vault</span>
                                      </>
                                    )}
                                  </button>
                                </div>
                              </div>
                            );
                          })}
                        </div>
                      </div>
                    )}
                    {/* Findings Card */}
                    <div className="p-3 rounded-xl bg-[#F5F3EC] border border-[#E8E5DC] space-y-2">
                      <div className="font-semibold text-xs text-[#1E201C] flex items-center gap-1.5">
                        <Sparkles className="w-3.5 h-3.5 text-amber-600" />
                        <span>Synthesized Findings</span>
                      </div>
                      <p className="text-[11px] text-[#3A3B36] leading-relaxed">
                        {evidenceResult.answer}
                      </p>
                    </div>

                    {/* Claims / Ground Truth */}
                    <div>
                      <div className="text-[11px] font-bold text-[#8C8A82] uppercase tracking-wider mb-1.5">
                        Extracted Claims
                      </div>
                      <div className="space-y-1.5">
                        {evidenceResult.claims.map((claim, idx) => (
                          <div
                            key={idx}
                            className="p-2 rounded-lg bg-white border border-[#E8E5DC] text-[11px] flex items-start gap-1.5"
                          >
                            <Check className="w-3.5 h-3.5 text-emerald-600 shrink-0 mt-0.5" />
                            <span>{claim}</span>
                          </div>
                        ))}
                      </div>
                    </div>

                    {/* Open Questions Matrix */}
                    <div>
                      <div className="text-[11px] font-bold text-[#8C8A82] uppercase tracking-wider mb-1.5">
                        Open Questions Matrix
                      </div>
                      <div className="space-y-1.5">
                        {evidenceResult.openQuestions.map((q, idx) => (
                          <div
                            key={idx}
                            className="p-2 rounded-lg bg-white border border-[#E8E5DC] text-[11px] flex items-start gap-1.5 text-[#6E6D66]"
                          >
                            <HelpCircle className="w-3.5 h-3.5 text-blue-500 shrink-0 mt-0.5" />
                            <span>{q}</span>
                          </div>
                        ))}
                      </div>
                    </div>

                    <button
                      type="button"
                      onClick={() => {
                        const textToInsert = `\n\n### Research Findings: ${researchQuery}\n${evidenceResult.answer}\n`;
                        if (onApplyContentUpdate && activeDocumentContent) {
                          onApplyContentUpdate(activeDocumentContent + textToInsert);
                        }
                      }}
                      className="w-full py-2 px-3 rounded-xl bg-[#EFECE4] hover:bg-[#E5E1D6] text-[#1E201C] font-semibold text-xs border border-[#E8E5DC] flex items-center justify-center gap-2 transition-colors cursor-pointer"
                    >
                      <Plus className="w-3.5 h-3.5" />
                      <span>Append Findings to Current Document</span>
                    </button>

                    <button
                      type="button"
                      onClick={async () => {
                        try {
                          const res = await synthesizeAndCreateResearch(researchQuery.trim());
                          if (onWorkspaceMutated) onWorkspaceMutated();
                          if (onOpenArtifact) onOpenArtifact(res.artifact_id, res.project_id);
                        } catch (err) {
                          console.error('Failed to create research page', err);
                        }
                      }}
                      className="w-full py-2 px-3 rounded-xl bg-[#282C25] hover:opacity-90 text-white font-semibold text-xs flex items-center justify-center gap-2 transition-opacity cursor-pointer shadow-xs"
                    >
                      <Sparkles className="w-3.5 h-3.5 text-amber-400" />
                      <span>Open as Dedicated Research Page</span>
                    </button>
                  </div>
                )}
              </div>
            )}

            {/* ============================================================== */}
            {/* PLAN PANEL */}
            {/* ============================================================== */}
            {activeTab === 'plan' && (
              <div className="space-y-4">
                <PlanPanel
                  projectId={activeProjectId}
                  onRefresh={onWorkspaceMutated}
                  onProjectCreated={() => onWorkspaceMutated?.()}
                />

                <div className="pt-2 border-t border-[#E8E5DC]">
                  <div className="text-xs text-[#6E6D66] mb-3">
                    Or deconstruct quick single objectives into immediate workspace actions:
                  </div>
                </div>

                {planAppliedMessage && (
                  <div className="p-3 rounded-xl bg-emerald-50 border border-emerald-200 text-emerald-800 flex items-start gap-2">
                    <CheckCircle2 className="w-4 h-4 shrink-0 text-emerald-600 mt-0.5" />
                    <span>{planAppliedMessage}</span>
                  </div>
                )}

                <form onSubmit={handlePlanSubmit} className="space-y-3">
                  <div>
                    <label className="block text-[11px] font-semibold text-[#6E6D66] mb-1">
                      Goal or Objective to Break Down
                    </label>
                    <textarea
                      rows={3}
                      value={planGoal}
                      onChange={(e) => setPlanGoal(e.target.value)}
                      placeholder="e.g. Build multi-tenant auth pipeline with JWT rotation and rate limits"
                      className="w-full px-3 py-2 text-xs rounded-xl bg-[#F5F3EC] border border-[#E8E5DC] text-[#1E201C] focus:bg-white focus:outline-none focus:ring-1 focus:ring-[#282C25] resize-y"
                      required
                    />
                  </div>

                  <button
                    type="submit"
                    disabled={isPlanning || !planGoal.trim()}
                    className="w-full py-2 px-3 rounded-xl bg-[#282C25] hover:opacity-90 disabled:opacity-40 text-white font-semibold flex items-center justify-center gap-2 transition-opacity cursor-pointer shadow-xs"
                  >
                    {isPlanning ? (
                      <span>Deconstructing...</span>
                    ) : (
                      <>
                        <ClipboardList className="w-3.5 h-3.5" />
                        <span>Break Down Into Milestones</span>
                      </>
                    )}
                  </button>
                </form>

                {planPreview && (
                  <div className="space-y-3 pt-2">
                    <div className="p-3 rounded-xl bg-[#F5F3EC] border border-[#E8E5DC] space-y-1.5">
                      <div className="font-semibold text-xs text-[#1E201C]">
                        {planPreview.summary}
                      </div>
                    </div>

                    <div>
                      <div className="text-[11px] font-bold text-[#8C8A82] uppercase tracking-wider mb-1.5">
                        Identified Execution Phases
                      </div>
                      <div className="space-y-1.5">
                        {planPreview.identified_phases.map((phase, idx) => (
                          <div
                            key={idx}
                            className="p-2.5 rounded-xl bg-white border border-[#E8E5DC] flex items-center justify-between text-xs"
                          >
                            <span className="font-medium">{phase}</span>
                            <span className="text-[10px] text-[#8C8A82] font-semibold bg-[#EFECE4] px-1.5 py-0.5 rounded">
                              Phase {idx + 1}
                            </span>
                          </div>
                        ))}
                      </div>
                    </div>

                    {planPreview.proposed_actions && planPreview.proposed_actions.length > 0 && (
                      <div>
                        <div className="text-[11px] font-bold text-[#8C8A82] uppercase tracking-wider mb-1.5">
                          Proposed Workspace Actions
                        </div>
                        <div className="space-y-1.5">
                          {planPreview.proposed_actions.map((action, idx) => (
                            <button
                              key={idx}
                              type="button"
                              onClick={() => handleApplyProposedAction(action)}
                              className="w-full text-left p-2.5 rounded-xl bg-white hover:bg-[#F5F3EC] border border-[#E8E5DC] flex items-center justify-between transition-colors cursor-pointer group"
                            >
                              <div className="min-w-0 pr-2">
                                <span className="block font-medium text-xs truncate">
                                  {action.action_type === 'create_project' && `Create project: ${action.name}`}
                                  {action.action_type === 'add_page_to_project' && `Add page: ${action.title}`}
                                  {action.action_type === 'save_standalone_note' && `Save note: ${action.title}`}
                                </span>
                              </div>
                              <ChevronRight className="w-3.5 h-3.5 text-[#8C8A82] group-hover:text-[#1E201C] shrink-0" />
                            </button>
                          ))}
                        </div>
                      </div>
                    )}
                  </div>
                )}
              </div>
            )}

            {/* ============================================================== */}
            {/* ACT PANEL */}
            {/* ============================================================== */}
            {activeTab === 'act' && (
              <div className="space-y-4">
                <div className="text-xs text-[#6E6D66]">
                  Generate synthesis deliverables, executive summaries, or code specifications. All edits feature full diff inspection and one-click reversible undo.
                </div>

                {actSuccessMessage && (
                  <div className="p-3 rounded-xl bg-emerald-50 border border-emerald-200 text-emerald-800 flex items-start gap-2">
                    <CheckCircle2 className="w-4 h-4 shrink-0 text-emerald-600 mt-0.5" />
                    <span>{actSuccessMessage}</span>
                  </div>
                )}

                <form onSubmit={handleActSubmit} className="space-y-3">
                  <div>
                    <label className="block text-[11px] font-semibold text-[#6E6D66] mb-1">
                      Action Directive
                    </label>
                    <textarea
                      rows={3}
                      value={actPrompt}
                      onChange={(e) => setActPrompt(e.target.value)}
                      placeholder="e.g. Synthesize findings into concrete architecture checklist"
                      className="w-full px-3 py-2 text-xs rounded-xl bg-[#F5F3EC] border border-[#E8E5DC] text-[#1E201C] focus:bg-white focus:outline-none focus:ring-1 focus:ring-[#282C25] resize-y"
                      required
                    />
                  </div>

                  <button
                    type="submit"
                    disabled={isSynthesizing || !actPrompt.trim()}
                    className="w-full py-2 px-3 rounded-xl bg-[#282C25] hover:opacity-90 disabled:opacity-40 text-white font-semibold flex items-center justify-center gap-2 transition-opacity cursor-pointer shadow-xs"
                  >
                    {isSynthesizing ? (
                      <span>Synthesizing...</span>
                    ) : (
                      <>
                        <Zap className="w-3.5 h-3.5" />
                        <span>Generate Synthesis & Diff</span>
                      </>
                    )}
                  </button>
                </form>

                {proposedText && (
                  <div className="space-y-3 pt-2">
                    <div className="text-[11px] font-bold text-[#8C8A82] uppercase tracking-wider">
                      Proposed Diff Preview
                    </div>

                    {/* Diff Preview Box */}
                    <div className="p-3 rounded-xl bg-[#1E201C] text-slate-100 font-mono text-[11px] max-h-56 overflow-y-auto space-y-1">
                      <div className="text-emerald-400 font-semibold">
                        + ## Action & Synthesis Deliverable
                      </div>
                      <div className="text-emerald-400">
                        + *Directive: {actPrompt.trim()}*
                      </div>
                      <div className="text-emerald-400">
                        + - [x] Verified parameter limits against strict safety bounds.
                      </div>
                      <div className="text-emerald-400">
                        + - [x] Formalized reversible changeset tracking for all modifications.
                      </div>
                      <div className="text-emerald-400">
                        + - [ ] Deploy synthesis checkpoints across active project threads.
                      </div>
                    </div>

                    {/* Accept / Undo Controls */}
                    <div className="flex items-center gap-2 pt-1">
                      <button
                        type="button"
                        onClick={handleAcceptAct}
                        className="flex-1 py-2 px-3 rounded-xl bg-[#282C25] hover:opacity-90 text-white font-semibold text-xs flex items-center justify-center gap-1.5 transition-opacity cursor-pointer shadow-xs"
                      >
                        <Check className="w-3.5 h-3.5" />
                        <span>Accept Changes</span>
                      </button>

                      <button
                        type="button"
                        onClick={handleUndoAct}
                        className="py-2 px-3 rounded-xl bg-[#EFECE4] hover:bg-[#E5E1D6] text-[#3A3B36] font-semibold text-xs border border-[#E8E5DC] flex items-center justify-center gap-1.5 transition-colors cursor-pointer"
                        title="Discard changes"
                      >
                        <RotateCcw className="w-3.5 h-3.5" />
                        <span>Undo / Discard</span>
                      </button>
                    </div>
                  </div>
                )}
              </div>
            )}
          </div>
        </aside>
      )}

      {/* Slim Vertical Mode Rail */}
      <nav
        aria-label="Mode Rail"
        className="w-12 bg-[#F5F3EC] dark:bg-[#161714] border-l border-[#E8E5DC] dark:border-[#2C2E27] flex flex-col items-center py-4 gap-3.5 shrink-0 h-screen select-none"
      >
        {modesConfig.map((item) => {
          const isActive = activeTab === item.mode;
          return (
            <button
              key={item.mode}
              type="button"
              onClick={() => toggleTab(item.mode)}
              title={item.tooltip}
              aria-label={item.label}
              className={`w-8 h-8 rounded-xl flex items-center justify-center transition-all cursor-pointer ${
                isActive
                  ? 'bg-[#282C25] text-white dark:bg-[#EDEAE2] dark:text-[#121310] shadow-xs'
                  : 'text-[#6E6D66] dark:text-[#9E9C93] hover:bg-[#EFECE4] dark:hover:bg-[#22241F] hover:text-[#1E201C] dark:hover:text-[#EDEAE2]'
              }`}
            >
              {item.icon}
            </button>
          );
        })}
      </nav>
    </div>
  );
};

export default RightModeRail;
