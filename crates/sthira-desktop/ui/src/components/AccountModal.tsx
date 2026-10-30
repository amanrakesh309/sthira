import React from 'react';
import { User, X, Shield, Lock, HardDrive } from 'lucide-react';

interface AccountModalProps {
  isOpen: boolean;
  onClose: () => void;
}

export const AccountModal: React.FC<AccountModalProps> = ({ isOpen, onClose }) => {
  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/40 backdrop-blur-sm animate-in fade-in duration-150">
      <div className="w-full max-w-md bg-[#FFFFFF] dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-2xl shadow-2xl overflow-hidden flex flex-col">
        {/* Header */}
        <div className="px-6 py-4 border-b border-[#ECE9E0] dark:border-[#2C2E27] flex items-center justify-between bg-[#FAF8F3] dark:bg-[#161714]">
          <div className="flex items-center gap-2.5">
            <div className="w-8 h-8 rounded-xl bg-[#EFECE4] dark:bg-[#252820] text-[#282C25] dark:text-[#EDEAE2] flex items-center justify-center font-bold text-sm">
              <User className="w-4 h-4" />
            </div>
            <div>
              <h3 className="text-base font-bold text-[#1E201C] dark:text-[#EDEAE2]">Local Account & Integrity</h3>
              <p className="text-xs text-[#6E6D66] dark:text-[#9E9C93]">Authorship provenance & security guarantees</p>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label="Close"
            className="p-1.5 rounded-lg text-[#6E6D66] dark:text-[#9E9C93] hover:text-[#1E201C] dark:hover:text-[#EDEAE2] hover:bg-[#EFECE4] dark:hover:bg-[#252820] transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Content */}
        <div className="p-6 space-y-4 text-xs">
          {/* Identity Card */}
          <div className="bg-[#FAF8F3] dark:bg-[#161714] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-xl p-4 flex items-center gap-3.5">
            <div className="w-10 h-10 rounded-full bg-[#282C25] dark:bg-[#EDEAE2] text-white dark:text-[#121310] flex items-center justify-center font-bold text-base">
              A
            </div>
            <div>
              <h4 className="font-bold text-sm text-[#1E201C] dark:text-[#EDEAE2]">Local Architect</h4>
              <p className="text-xs text-[#6E6D66] dark:text-[#9E9C93]">Full sovereignty · Local Vault User</p>
            </div>
          </div>

          {/* Guarantees */}
          <div className="space-y-3 pt-2">
            <h5 className="font-bold text-xs uppercase tracking-wider text-[#6E6D66] dark:text-[#9E9C93]">
              Architectural Guarantees
            </h5>

            <div className="flex items-start gap-3 p-3 rounded-xl bg-[#FAF8F3] dark:bg-[#161714] border border-[#E8E5DC] dark:border-[#2C2E27]">
              <Shield className="w-4 h-4 text-[#2D5A27] dark:text-[#88D080] shrink-0 mt-0.5" />
              <div>
                <strong className="text-[#1E201C] dark:text-[#EDEAE2] font-semibold block mb-0.5">
                  Anti-Laundering Trigger
                </strong>
                <span className="text-[#6E6D66] dark:text-[#9E9C93] leading-relaxed">
                  Database trigger <code className="bg-[#EFECE4] dark:bg-[#252820] text-[#1E201C] dark:text-[#EDEAE2] px-1 py-0.5 rounded text-[11px]">nodes_origin_matches_authorship</code> strictly prohibits AI from forging human decisions.
                </span>
              </div>
            </div>

            <div className="flex items-start gap-3 p-3 rounded-xl bg-[#FAF8F3] dark:bg-[#161714] border border-[#E8E5DC] dark:border-[#2C2E27]">
              <Lock className="w-4 h-4 text-[#2B4C6F] dark:text-[#82B1FF] shrink-0 mt-0.5" />
              <div>
                <strong className="text-[#1E201C] dark:text-[#EDEAE2] font-semibold block mb-0.5">
                  Verbatim Quote Anchoring
                </strong>
                <span className="text-[#6E6D66] dark:text-[#9E9C93] leading-relaxed">
                  Claims survive only if verified directly against original text with exact boundary preservation.
                </span>
              </div>
            </div>

            <div className="flex items-start gap-3 p-3 rounded-xl bg-[#FAF8F3] dark:bg-[#161714] border border-[#E8E5DC] dark:border-[#2C2E27]">
              <HardDrive className="w-4 h-4 text-[#8C6D1F] dark:text-[#F3BA59] shrink-0 mt-0.5" />
              <div>
                <strong className="text-[#1E201C] dark:text-[#EDEAE2] font-semibold block mb-0.5">
                  100% Local-First Storage
                </strong>
                <span className="text-[#6E6D66] dark:text-[#9E9C93] leading-relaxed">
                  All knowledge resides in your local SQLite WAL database and human-readable Markdown files. Zero telemetry.
                </span>
              </div>
            </div>
          </div>
        </div>

        {/* Footer */}
        <div className="px-6 py-3.5 bg-[#FAF8F3] dark:bg-[#161714] border-t border-[#ECE9E0] dark:border-[#2C2E27] flex justify-end">
          <button
            type="button"
            onClick={onClose}
            className="px-4 py-1.5 rounded-xl bg-[#282C25] dark:bg-[#EDEAE2] text-white dark:text-[#121310] font-medium hover:opacity-90 transition-opacity"
          >
            Done
          </button>
        </div>
      </div>
    </div>
  );
};
