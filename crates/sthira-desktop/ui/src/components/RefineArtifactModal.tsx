import React, { useState, useEffect } from 'react';
import { X, RefreshCw, Globe, Loader2, Sparkles } from 'lucide-react';
import { ModelSelector } from './ModelSelector';
import { getProviderCatalog, refineArtifact } from '../api/ipc';
import type { ProviderModelEntry, ProviderEntry } from '../types/sthira';

export interface RefineArtifactModalProps {
  isOpen: boolean;
  onClose: () => void;
  artifactId: string;
  artifactTitle: string;
  onRefined: (newContent: string) => void;
  models?: ProviderModelEntry[];
}

export const RefineArtifactModal: React.FC<RefineArtifactModalProps> = ({
  isOpen,
  onClose,
  artifactId,
  artifactTitle,
  onRefined,
  models: propModels,
}) => {
  const [critique, setCritique] = useState('');
  const [webSearch, setWebSearch] = useState(false);
  const [selectedModelId, setSelectedModelId] = useState<string | null>(null);
  const [availableModels, setAvailableModels] = useState<ProviderModelEntry[]>(propModels || []);
  const [catalog, setCatalog] = useState<ProviderEntry[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isOpen) {
      setCritique('');
      setError(null);
      setIsLoading(false);
      return;
    }

    if (propModels && propModels.length > 0) {
      setAvailableModels(propModels);
      const def = propModels.find((m) => m.is_default) || propModels[0];
      if (def && !selectedModelId) {
        setSelectedModelId(def.model_identifier || def.id);
      }
    } else {
      getProviderCatalog()
        .then((cat) => {
          setCatalog(cat);
          const list: ProviderModelEntry[] = [];
          for (const p of cat) {
            if (p.is_enabled && p.models) {
              for (const m of p.models) {
                if (m.is_enabled) list.push(m);
              }
            }
          }
          setAvailableModels(list);
          const def = list.find((m) => m.is_default) || list[0];
          if (def && !selectedModelId) {
            setSelectedModelId(def.model_identifier || def.id);
          }
        })
        .catch(() => {});
    }
  }, [isOpen, propModels, selectedModelId]);

  if (!isOpen) return null;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!critique.trim() || isLoading) return;

    setIsLoading(true);
    setError(null);

    try {
      const refinedText = await refineArtifact(
        artifactId,
        critique.trim(),
        selectedModelId,
        webSearch
      );
      onRefined(refinedText);
      onClose();
    } catch (err: any) {
      setError(typeof err === 'string' ? err : err?.message || 'Refinement failed.');
    } finally {
      setIsLoading(false);
    }
  };

  const selectedModel =
    availableModels.find(
      (m) => m.model_identifier === selectedModelId || m.id === selectedModelId
    ) || null;

  return (
    <div
      role="dialog"
      aria-modal="true"
      data-testid="refine-artifact-modal"
      className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/40 backdrop-blur-xs animate-in fade-in duration-150"
    >
      <div
        className="w-full max-w-lg bg-[#FAF8F5] dark:bg-[#1C1E18] border border-[#E8E5DC] dark:border-[#2D3027] rounded-xl shadow-xl flex flex-col overflow-hidden"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-[#E8E5DC] dark:border-[#2D3027] bg-[#F4F1EA] dark:bg-[#23261F]">
          <div className="flex items-center gap-2">
            <div className="w-8 h-8 rounded-lg bg-emerald-100 dark:bg-emerald-950/60 text-emerald-800 dark:text-emerald-300 flex items-center justify-center">
              <RefreshCw className="w-4 h-4" />
            </div>
            <div>
              <h2 className="text-sm font-semibold text-[#282C25] dark:text-[#EDEAE2]">
                Refine / Re-Research Note
              </h2>
              <p className="text-xs text-[#7D7A70] dark:text-[#9E9C93] truncate max-w-xs">
                {artifactTitle || 'Untitled Note'}
              </p>
            </div>
          </div>
          <button
            type="button"
            data-testid="btn-close-refine-modal"
            onClick={onClose}
            className="p-1 rounded-md text-[#7D7A70] dark:text-[#9E9C93] hover:text-[#282C25] dark:hover:text-[#EDEAE2] hover:bg-[#E8E5DC] dark:hover:bg-[#2D3027] transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Form Body */}
        <form onSubmit={handleSubmit} className="p-5 flex flex-col gap-4">
          {error && (
            <div className="p-3 text-xs bg-red-50 dark:bg-red-950/40 text-red-700 dark:text-red-300 border border-red-200 dark:border-red-900 rounded-lg">
              {error}
            </div>
          )}

          {/* Critique Input */}
          <div className="flex flex-col gap-1.5">
            <label
              htmlFor="critique-prompt"
              className="text-xs font-medium text-[#484B42] dark:text-[#C5C2B8]"
            >
              Critique & Angle Adjustments
            </label>
            <textarea
              id="critique-prompt"
              data-testid="textarea-critique-prompt"
              rows={4}
              value={critique}
              onChange={(e) => setCritique(e.target.value)}
              placeholder="What changes or angles should be refined? (e.g., Focus more on benchmark data, add latency comparisons, address security trade-offs)"
              className="w-full text-xs p-3 rounded-lg border border-[#D5D1C7] dark:border-[#383B32] bg-white dark:bg-[#141511] text-[#282C25] dark:text-[#EDEAE2] placeholder-[#9E9C93] focus:outline-none focus:ring-1 focus:ring-emerald-600 dark:focus:ring-emerald-500 resize-none transition-all leading-relaxed"
              autoFocus
            />
          </div>

          {/* Model Selector & Web Search Toggle Row */}
          <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3 pt-1">
            <div className="flex flex-col gap-1">
              <span className="text-[11px] font-medium text-[#7D7A70] dark:text-[#9E9C93]">
                Synthesizing Model
              </span>
              <ModelSelector
                selectedModel={selectedModel}
                selectedModelId={selectedModelId}
                onSelectModel={(m) => setSelectedModelId(m.model_identifier || m.id)}
                models={availableModels}
                catalog={catalog}
                size="sm"
                dataTestId="refine-model-selector"
              />
            </div>

            {/* Web Search Toggle */}
            <label className="flex items-center gap-2 cursor-pointer mt-1 sm:mt-4 text-xs font-medium text-[#484B42] dark:text-[#C5C2B8]">
              <input
                type="checkbox"
                data-testid="checkbox-web-search-refine"
                checked={webSearch}
                onChange={(e) => setWebSearch(e.target.checked)}
                className="w-4 h-4 rounded text-emerald-600 border-[#D5D1C7] dark:border-[#383B32] focus:ring-emerald-500 cursor-pointer"
              />
              <span className="inline-flex items-center gap-1.5">
                <Globe className="w-3.5 h-3.5 text-blue-600 dark:text-blue-400" />
                <span>Search Web for New Evidence</span>
              </span>
            </label>
          </div>

          <div className="text-[11px] text-[#7D7A70] dark:text-[#9E9C93] bg-[#F4F1EA] dark:bg-[#23261F] p-2.5 rounded-lg border border-[#E8E5DC] dark:border-[#2D3027] flex items-center gap-2">
            <Sparkles className="w-3.5 h-3.5 text-amber-600 dark:text-amber-400 shrink-0" />
            <span>
              A snapshot ChangeSet is captured before saving. You can undo anytime with 1-click.
            </span>
          </div>

          {/* Action Buttons */}
          <div className="flex items-center justify-end gap-2 pt-2 border-t border-[#E8E5DC] dark:border-[#2D3027]">
            <button
              type="button"
              onClick={onClose}
              disabled={isLoading}
              className="px-3 py-1.5 text-xs font-medium rounded-lg text-[#6E6D66] dark:text-[#9E9C93] hover:text-[#282C25] dark:hover:text-[#EDEAE2] hover:bg-[#E8E5DC] dark:hover:bg-[#2D3027] transition-colors cursor-pointer"
            >
              Cancel
            </button>
            <button
              type="submit"
              data-testid="btn-run-refinement"
              disabled={!critique.trim() || isLoading}
              className={`inline-flex items-center gap-1.5 px-4 py-1.5 text-xs font-semibold rounded-lg text-white shadow-xs transition-all cursor-pointer ${
                !critique.trim() || isLoading
                  ? 'bg-emerald-600/50 cursor-not-allowed opacity-60'
                  : 'bg-emerald-700 hover:bg-emerald-800 active:scale-98'
              }`}
            >
              {isLoading ? (
                <>
                  <Loader2 className="w-3.5 h-3.5 animate-spin" />
                  <span>Refining Note...</span>
                </>
              ) : (
                <>
                  <RefreshCw className="w-3.5 h-3.5" />
                  <span>Run Refinement</span>
                </>
              )}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
