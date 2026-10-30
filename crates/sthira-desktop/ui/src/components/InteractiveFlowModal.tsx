import React, { useState, useEffect } from 'react';
import { X, Brain, CheckSquare, Bookmark, Check, Sparkles } from 'lucide-react';
import { createGuidedProject, saveArtifact, synthesizeAndCreateResearch } from '../api/ipc';

export type FlowMode = 'research' | 'plan' | 'capture';

interface InteractiveFlowModalProps {
  mode: FlowMode | null;
  initialPrompt?: string;
  onClose: () => void;
  onProjectCreated: (projectId: string) => void;
  onArtifactSaved: (artifactId?: string) => void;
}

export const InteractiveFlowModal: React.FC<InteractiveFlowModalProps> = ({
  mode,
  initialPrompt,
  onClose,
  onProjectCreated,
  onArtifactSaved,
}) => {
  const [title, setTitle] = useState('');
  const [purpose, setPurpose] = useState('');
  const [notes, setNotes] = useState('');
  const [useModelSynthesis, setUseModelSynthesis] = useState(true);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  useEffect(() => {
    if (initialPrompt && mode) {
      if (mode === 'research' || mode === 'plan') {
        const cleaned = initialPrompt.trim();
        const shortTitle = cleaned.length > 50 ? cleaned.slice(0, 50) + '...' : cleaned;
        setTitle(shortTitle);
        setPurpose(cleaned);
      } else {
        setNotes(initialPrompt.trim());
      }
    } else {
      setTitle('');
      setPurpose('');
      setNotes('');
    }
  }, [initialPrompt, mode]);

  if (!mode) return null;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!title.trim() && mode !== 'capture') return;
    if (mode === 'capture' && !notes.trim()) return;

    setIsSubmitting(true);
    setErrorMsg(null);

    try {
      if (mode === 'research') {
        const projName = title.trim();
        const projPurpose = purpose.trim() || `Investigate ${projName} with verified source citations.`;

        if (useModelSynthesis) {
          const synthesisPrompt = `${projName}. Research Objectives: ${projPurpose}. Notes: ${notes.trim()}`;
          const res = await synthesizeAndCreateResearch(synthesisPrompt);
          onProjectCreated(res.project_id);
          onClose();
          return;
        }

        const initialContent = `# ${projName}\n\n## Objective\n${projPurpose}\n\n## Core Questions & Hypotheses\n${notes.trim() ? notes : '- What are the key bottlenecks?\n- How does this compare to existing baselines?'}\n\n## Initial Findings\n`;

        const res = await createGuidedProject(
          projName,
          projPurpose,
          `${projName} — Initial Notes`,
          initialContent
        );
        onProjectCreated(res.project_id);
        onClose();
      } else if (mode === 'plan') {
        const projName = title.trim();
        const projPurpose = purpose.trim() || `Plan and execute ${projName}.`;
        const initialContent = `# ${projName} — Project Plan\n\n## Vision & Objective\n${projPurpose}\n\n## Key Milestones\n${notes.trim() ? notes : '- [ ] Phase 1: Architecture & Data Schema\n- [ ] Phase 2: Core Implementation & Safety Boundaries\n- [ ] Phase 3: Integration Testing & Verification'}\n\n## Working Notes\n`;

        const res = await createGuidedProject(
          projName,
          projPurpose,
          `${projName} Roadmap`,
          initialContent
        );
        onProjectCreated(res.project_id);
        onClose();
      } else if (mode === 'capture') {
        const noteContent = title.trim()
          ? `# ${title.trim()}\n\n${notes.trim()}`
          : notes.trim();

        const saved = await saveArtifact('', noteContent, title.trim() || undefined, null);
        onArtifactSaved(saved.id);
        onClose();
      }
    } catch (err: any) {
      setErrorMsg(`Failed: ${err?.message || err}`);
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/30 backdrop-blur-sm animate-in fade-in duration-150">
      <div className="w-full max-w-lg bg-[#FFFFFF] border border-[#E8E5DC] rounded-2xl shadow-2xl overflow-hidden flex flex-col">
        {/* Header */}
        <div className="px-6 py-4 border-b border-[#ECE9E0] flex items-center justify-between">
          <div className="flex items-center gap-2.5">
            <div className="w-8 h-8 rounded-xl bg-[#282C25] text-white flex items-center justify-center">
              {mode === 'research' && <Brain className="w-4 h-4" />}
              {mode === 'plan' && <CheckSquare className="w-4 h-4" />}
              {mode === 'capture' && <Bookmark className="w-4 h-4" />}
            </div>
            <div>
              <h3 className="text-base font-bold text-[#1E201C]">
                {mode === 'research' && 'Start New Research'}
                {mode === 'plan' && 'Plan a Project'}
                {mode === 'capture' && 'Capture a Thought'}
              </h3>
              <p className="text-xs text-[#6E6D66]">
                {mode === 'research' && 'Explore questions with grounded citation verification'}
                {mode === 'plan' && 'Define objectives and scaffold milestones'}
                {mode === 'capture' && 'Save quick observations to your local vault'}
              </p>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label="Close"
            className="p-1.5 rounded-lg text-[#6E6D66] hover:text-[#1E201C] hover:bg-[#EFECE4] transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Form Body */}
        <form onSubmit={handleSubmit} className="p-6 space-y-4">
          {errorMsg && (
            <div className="p-3 bg-rose-50 border border-rose-200 text-rose-700 text-xs rounded-xl">
              {errorMsg}
            </div>
          )}

          {mode !== 'capture' && (
            <div>
              <label className="block text-xs font-semibold text-[#1E201C] mb-1.5">
                {mode === 'research' ? 'Research Topic or Question' : 'Project Name'}
              </label>
              <input
                type="text"
                autoFocus
                required
                value={title}
                onChange={(e) => setTitle(e.target.value)}
                placeholder={
                  mode === 'research'
                    ? 'e.g. Local LLM Speculative Decoding'
                    : 'e.g. Webhook Delivery Platform'
                }
                className="w-full px-3.5 py-2.5 rounded-xl border border-[#E8E5DC] bg-white text-sm text-[#1E201C] placeholder-[#6E6D66] outline-none focus:border-[#282C25] transition-colors"
              />
            </div>
          )}

          {mode !== 'capture' && (
            <div>
              <label className="block text-xs font-semibold text-[#1E201C] mb-1.5">
                {mode === 'research' ? 'Research Goal & Scope' : 'Core Purpose'}
              </label>
              <input
                type="text"
                value={purpose}
                onChange={(e) => setPurpose(e.target.value)}
                placeholder={
                  mode === 'research'
                    ? 'e.g. Evaluate draft model latency reduction and memory bounds'
                    : 'e.g. High throughput webhook ingestion with replay guarantees'
                }
                className="w-full px-3.5 py-2.5 rounded-xl border border-[#E8E5DC] bg-white text-sm text-[#1E201C] placeholder-[#6E6D66] outline-none focus:border-[#282C25] transition-colors"
              />
            </div>
          )}

          {mode === 'capture' && (
            <div>
              <label className="block text-xs font-semibold text-[#1E201C] mb-1.5">
                Note Title (Optional)
              </label>
              <input
                type="text"
                value={title}
                onChange={(e) => setTitle(e.target.value)}
                placeholder="e.g. Quick observation on prompt caching"
                className="w-full px-3.5 py-2.5 rounded-xl border border-[#E8E5DC] bg-white text-sm text-[#1E201C] placeholder-[#6E6D66] outline-none focus:border-[#282C25] transition-colors"
              />
            </div>
          )}

          <div>
            <label className="block text-xs font-semibold text-[#1E201C] mb-1.5">
              {mode === 'research' && 'Initial Hypotheses or Notes (Optional)'}
              {mode === 'plan' && 'Milestones or Objectives (Optional)'}
              {mode === 'capture' && 'Thought Body'}
            </label>
            <textarea
              rows={4}
              required={mode === 'capture'}
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
              placeholder={
                mode === 'research'
                  ? 'Enter initial ideas, literature links, or questions...'
                  : mode === 'plan'
                  ? '- [ ] Step 1: Core setup\n- [ ] Step 2: Key integrations\n- [ ] Step 3: Verification'
                  : 'Write your thought, snippet, or idea...'
              }
              className="w-full px-3.5 py-2.5 rounded-xl border border-[#E8E5DC] bg-white text-sm text-[#1E201C] placeholder-[#6E6D66] outline-none focus:border-[#282C25] transition-colors resize-none leading-relaxed font-mono"
            />
          </div>

          {mode === 'research' && (
            <div className="flex items-center gap-2 pt-1 pb-1">
              <input
                type="checkbox"
                id="useModelSynthesis"
                checked={useModelSynthesis}
                onChange={(e) => setUseModelSynthesis(e.target.checked)}
                className="w-4 h-4 rounded border-[#E8E5DC] text-[#282C25] focus:ring-0 cursor-pointer"
              />
              <label htmlFor="useModelSynthesis" className="text-xs text-[#3A3B36] font-medium flex items-center gap-1.5 cursor-pointer">
                <Sparkles className="w-3.5 h-3.5 text-[#282C25]" />
                Synthesize research document using configured models (gemma4 / llama3.2)
              </label>
            </div>
          )}

          <div className="pt-2 flex items-center justify-end gap-2.5">
            <button
              type="button"
              onClick={onClose}
              disabled={isSubmitting}
              className="px-4 py-2 rounded-xl text-xs font-semibold text-[#6E6D66] hover:text-[#1E201C] hover:bg-[#EFECE4] transition-colors cursor-pointer"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={isSubmitting || (mode !== 'capture' && !title.trim()) || (mode === 'capture' && !notes.trim())}
              className="inline-flex items-center gap-1.5 px-4 py-2 rounded-xl bg-[#282C25] text-white text-xs font-semibold hover:opacity-90 disabled:opacity-40 transition-opacity cursor-pointer"
            >
              <Check className="w-3.5 h-3.5" />
              {isSubmitting
                ? (useModelSynthesis && mode === 'research' ? 'Synthesizing with models...' : 'Creating...')
                : mode === 'research'
                ? 'Begin Research'
                : mode === 'plan'
                ? 'Create Plan Workspace'
                : 'Save Thought'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
