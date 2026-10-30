import React from 'react';
import { Sparkles, Check, X } from 'lucide-react';
import type { ProposalView } from '../types/sthira';

interface AmbientProposalPillProps {
  proposal: ProposalView;
  onAccept: (proposalId: string) => void;
  onDismiss: (proposalId: string) => void;
}

export const AmbientProposalPill: React.FC<AmbientProposalPillProps> = ({
  proposal,
  onAccept,
  onDismiss,
}) => {
  return (
    <div className="bg-[#FAF8F3] border border-[#E8E5DC] rounded-xl p-3 shadow-sm hover:border-[#D8D4C8] transition-all">
      <div className="flex items-center gap-1.5 text-xs font-semibold text-[#8C6D1F] mb-1">
        <Sparkles className="w-3.5 h-3.5" />
        Suggested Link
      </div>
      <p className="text-xs text-[#1E201C] mb-2 leading-relaxed font-medium">
        Link with &ldquo;{proposal.target_title}&rdquo;
      </p>
      {proposal.reason && (
        <p className="text-[11px] text-[#6E6D66] mb-2.5 line-clamp-2">
          {proposal.reason}
        </p>
      )}
      <div className="flex items-center gap-1.5">
        <button
          type="button"
          onClick={() => onAccept(proposal.id)}
          className="inline-flex items-center gap-1 px-2.5 py-1 rounded-lg bg-[#282C25] text-white text-xs font-medium hover:opacity-90 transition-opacity"
        >
          <Check className="w-3 h-3" />
          Accept
        </button>
        <button
          type="button"
          onClick={() => onDismiss(proposal.id)}
          className="inline-flex items-center gap-1 px-2.5 py-1 rounded-lg bg-[#EFECE4] hover:bg-[#E5E1D6] text-[#3A3B36] text-xs font-medium transition-colors"
        >
          <X className="w-3 h-3" />
          Dismiss
        </button>
      </div>
    </div>
  );
};
