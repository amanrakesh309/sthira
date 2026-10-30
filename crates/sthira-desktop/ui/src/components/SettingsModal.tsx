import React, { useState } from 'react';
import { SlidersHorizontal, X, Cpu, Database, Trash2, Sparkles, Check } from 'lucide-react';
import { clearVaultData, seedDemoWorkspace } from '../api/ipc';

interface SettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
  onVaultMutated: () => void;
}

export const SettingsModal: React.FC<SettingsModalProps> = ({
  isOpen,
  onClose,
  onVaultMutated,
}) => {
  const [ollamaHost, setOllamaHost] = useState('http://localhost:11434');
  const [fastModel, setFastModel] = useState('llama3.2:latest');
  const [synthesisModel, setSynthesisModel] = useState('gemma4:31b-cloud');
  const [deepReviewModel, setDeepReviewModel] = useState('gpt-oss:120b-cloud');
  const [isProcessing, setIsProcessing] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);

  if (!isOpen) return null;

  const handleResetVault = async () => {
    if (!confirm('Are you sure you want to clear all projects, notes, and epistemic claims?')) {
      return;
    }
    setIsProcessing(true);
    try {
      await clearVaultData();
      setNotice('Vault cleared to clean start.');
      onVaultMutated();
    } catch (err: any) {
      setNotice(`Error: ${err?.message || err}`);
    } finally {
      setIsProcessing(false);
    }
  };

  const handleLoadDemo = async () => {
    setIsProcessing(true);
    try {
      await seedDemoWorkspace();
      setNotice('Demo workspace seeded successfully.');
      onVaultMutated();
    } catch (err: any) {
      setNotice(`Error: ${err?.message || err}`);
    } finally {
      setIsProcessing(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/30 backdrop-blur-sm animate-in fade-in duration-150">
      <div className="w-full max-w-lg bg-[#FFFFFF] border border-[#E8E5DC] rounded-2xl shadow-2xl overflow-hidden flex flex-col">
        {/* Header */}
        <div className="px-6 py-4 border-b border-[#ECE9E0] flex items-center justify-between">
          <div className="flex items-center gap-2.5">
            <div className="w-8 h-8 rounded-xl bg-[#282C25] text-white flex items-center justify-center">
              <SlidersHorizontal className="w-4 h-4" />
            </div>
            <div>
              <h3 className="text-base font-bold text-[#1E201C]">Model & System Settings</h3>
              <p className="text-xs text-[#6E6D66]">Tiered LLM orchestration & local storage</p>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label="Close Settings"
            className="p-1.5 rounded-lg text-[#6E6D66] hover:text-[#1E201C] hover:bg-[#EFECE4] transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Content */}
        <div className="p-6 space-y-6 max-h-[80vh] overflow-y-auto text-xs">
          {/* Notice banner if any */}
          {notice && (
            <div className="p-3 bg-[#E3EBE3] border border-[#CCDCCC] text-[#2D5A27] rounded-xl flex items-center gap-2 font-medium">
              <Check className="w-4 h-4 shrink-0" />
              <span>{notice}</span>
            </div>
          )}

          {/* Model Tiers Configuration */}
          <div className="space-y-4">
            <h4 className="font-bold text-sm text-[#1E201C] flex items-center gap-2">
              <Cpu className="w-4 h-4 text-[#282C25]" />
              Tiered Model Provider
            </h4>

            {/* Fast Local */}
            <div className="bg-[#FAF8F3] border border-[#E8E5DC] rounded-xl p-3.5 space-y-2">
              <div className="flex items-center justify-between">
                <span className="font-semibold text-[#1E201C]">Tier 1: Fast Local Router</span>
                <span className="px-2 py-0.5 rounded bg-[#E3EBE3] text-[#2D5A27] font-semibold text-[10px]">
                  Local · Zero Latency
                </span>
              </div>
              <p className="text-[#6E6D66] leading-relaxed">
                Powers universal input classification and quick fact extractions in &lt;15ms.
              </p>
              <div className="grid grid-cols-2 gap-2 pt-1">
                <div>
                  <label className="text-[10px] text-[#6E6D66] font-medium">Ollama Host</label>
                  <input
                    type="text"
                    value={ollamaHost}
                    onChange={(e) => setOllamaHost(e.target.value)}
                    className="w-full mt-0.5 px-2.5 py-1.5 rounded-lg border border-[#E8E5DC] bg-white text-[#1E201C] outline-none"
                  />
                </div>
                <div>
                  <label className="text-[10px] text-[#6E6D66] font-medium">Model Tag</label>
                  <input
                    type="text"
                    value={fastModel}
                    onChange={(e) => setFastModel(e.target.value)}
                    className="w-full mt-0.5 px-2.5 py-1.5 rounded-lg border border-[#E8E5DC] bg-white text-[#1E201C] outline-none"
                  />
                </div>
              </div>
            </div>

            {/* Primary Synthesis */}
            <div className="bg-[#FAF8F3] border border-[#E8E5DC] rounded-xl p-3.5 space-y-2">
              <div className="flex items-center justify-between">
                <span className="font-semibold text-[#1E201C]">Tier 2: Primary Synthesis</span>
                <span className="px-2 py-0.5 rounded bg-[#E4EBF2] text-[#2B4C6F] font-semibold text-[10px]">
                  Cloud / Local 30B+
                </span>
              </div>
              <p className="text-[#6E6D66] leading-relaxed">
                Synthesizes substantive findings, Level 1 Resume Briefs, and extracts grounded citations.
              </p>
              <div>
                <label className="text-[10px] text-[#6E6D66] font-medium">Model Tag</label>
                <input
                  type="text"
                  value={synthesisModel}
                  onChange={(e) => setSynthesisModel(e.target.value)}
                  className="w-full mt-0.5 px-2.5 py-1.5 rounded-lg border border-[#E8E5DC] bg-white text-[#1E201C] outline-none"
                />
              </div>
            </div>

            {/* Deep Review */}
            <div className="bg-[#FAF8F3] border border-[#E8E5DC] rounded-xl p-3.5 space-y-2">
              <div className="flex items-center justify-between">
                <span className="font-semibold text-[#1E201C]">Tier 3: Deep Review & Reversibility</span>
                <span className="px-2 py-0.5 rounded bg-[#F9F5EC] text-[#8C6D1F] font-semibold text-[10px]">
                  Deep Review
                </span>
              </div>
              <p className="text-[#6E6D66] leading-relaxed">
                Audits cross-project contradictions, Level 2 reviews, and verifies quotes verbatim.
              </p>
              <div>
                <label className="text-[10px] text-[#6E6D66] font-medium">Model Tag</label>
                <input
                  type="text"
                  value={deepReviewModel}
                  onChange={(e) => setDeepReviewModel(e.target.value)}
                  className="w-full mt-0.5 px-2.5 py-1.5 rounded-lg border border-[#E8E5DC] bg-white text-[#1E201C] outline-none"
                />
              </div>
            </div>
          </div>

          {/* Database / Vault Management */}
          <div className="space-y-3 pt-2 border-t border-[#ECE9E0]">
            <h4 className="font-bold text-sm text-[#1E201C] flex items-center gap-2">
              <Database className="w-4 h-4 text-[#282C25]" />
              Vault Controls
            </h4>
            <div className="flex flex-col sm:flex-row items-center gap-2.5">
              <button
                type="button"
                disabled={isProcessing}
                onClick={handleResetVault}
                className="w-full sm:w-auto inline-flex items-center justify-center gap-1.5 px-3.5 py-2 rounded-xl bg-[#FAF8F3] hover:bg-rose-50 border border-[#E8E5DC] hover:border-rose-300 text-rose-700 font-medium transition-colors"
              >
                <Trash2 className="w-3.5 h-3.5" />
                Reset Vault (Clean Slate)
              </button>

              <button
                type="button"
                disabled={isProcessing}
                onClick={handleLoadDemo}
                className="w-full sm:w-auto inline-flex items-center justify-center gap-1.5 px-3.5 py-2 rounded-xl bg-[#EFECE4] hover:bg-[#E5E1D6] text-[#1E201C] font-medium transition-colors"
              >
                <Sparkles className="w-3.5 h-3.5 text-[#282C25]" />
                Load Demo Workspace
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
