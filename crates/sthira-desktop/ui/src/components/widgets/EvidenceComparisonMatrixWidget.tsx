import React from 'react';
import { Columns3, ExternalLink, Check, Sparkles, BookOpen } from 'lucide-react';
import type { ComparisonRow } from '../../types/sthira';

interface EvidenceComparisonMatrixWidgetProps {
  rows: ComparisonRow[];
  isStreaming?: boolean;
}

export const EvidenceComparisonMatrixWidget: React.FC<EvidenceComparisonMatrixWidgetProps> = ({
  rows,
  isStreaming = false,
}) => {
  if (!rows || rows.length === 0) {
    return null;
  }

  return (
    <div className="w-full space-y-3.5 my-4 font-sans">
      {/* Header */}
      <div className="flex items-center justify-between pb-1 border-b border-[#E8E5DC]">
        <div className="flex items-center gap-2">
          <Columns3 className="w-4 h-4 text-[#7A7870]" />
          <span className="text-xs font-semibold uppercase tracking-wider text-[#7A7870]">
            Evidence Comparison Matrix ({rows.length} Criteria)
          </span>
        </div>
        {isStreaming && (
          <span className="text-[11px] font-medium px-2 py-0.5 rounded-md bg-[#FFF7E8] text-[#8C5D00] border border-[#F2E0BA] flex items-center gap-1.5 animate-pulse">
            <span className="w-1.5 h-1.5 rounded-full bg-[#E58C00]" />
            Evaluating Criteria...
          </span>
        )}
      </div>

      {/* Comparison Table */}
      <div className="overflow-x-auto rounded-xl border border-[#E8E5DC] bg-white shadow-xs">
        <table className="w-full text-left border-collapse">
          <thead>
            <tr className="bg-[#FAF9F6] border-b border-[#E8E5DC] text-[11px] font-semibold text-[#66645D] uppercase tracking-wider">
              <th className="py-2.5 px-3.5 w-1/4">Evaluation Criteria</th>
              <th className="py-2.5 px-3.5 w-1/4">Aspect / Option A</th>
              <th className="py-2.5 px-3.5 w-1/4">Aspect / Option B</th>
              <th className="py-2.5 px-3.5 w-1/4">Verdict & Epistemic Basis</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-[#E8E5DC] text-xs">
            {rows.map((row, idx) => (
              <tr
                key={idx}
                className="hover:bg-[#FCFBF9] transition-colors duration-150 group"
              >
                {/* Criteria Column */}
                <td className="py-3 px-3.5 align-top font-medium text-[#1E201C]">
                  <div className="flex items-center gap-1.5">
                    <span className="w-1.5 h-1.5 rounded-full bg-[#8C8A81]" />
                    <span>{row.criteria}</span>
                  </div>
                </td>

                {/* Aspect A Column */}
                <td className="py-3 px-3.5 align-top text-[#363A31] leading-relaxed">
                  <div className="p-2 rounded-lg bg-[#FAF9F6] border border-[#EFECE4]">
                    <div className="flex items-center justify-between mb-1">
                      <span className="text-[10px] font-semibold uppercase text-[#7A7870]">
                        Approach A
                      </span>
                      <span className="text-[9px] font-mono px-1 py-0.2 rounded bg-[#EBF2EA] text-[#2F5933] border border-[#D1E2D0] flex items-center gap-1">
                        <BookOpen className="w-2.5 h-2.5" />
                        Recorded
                      </span>
                    </div>
                    <span>{row.aspect_a}</span>
                  </div>
                </td>

                {/* Aspect B Column */}
                <td className="py-3 px-3.5 align-top text-[#363A31] leading-relaxed">
                  <div className="p-2 rounded-lg bg-[#FAF9F6] border border-[#EFECE4]">
                    <div className="flex items-center justify-between mb-1">
                      <span className="text-[10px] font-semibold uppercase text-[#7A7870]">
                        Approach B
                      </span>
                      <span className="text-[9px] font-mono px-1 py-0.2 rounded bg-[#F1F4F9] text-[#2F456B] border border-[#D5E1F0] flex items-center gap-1">
                        <ExternalLink className="w-2.5 h-2.5" />
                        Source
                      </span>
                    </div>
                    <span>{row.aspect_b}</span>
                  </div>
                </td>

                {/* Verdict Column */}
                <td className="py-3 px-3.5 align-top text-[#282C25]">
                  {row.verdict ? (
                    <div className="p-2 rounded-lg bg-[#F5F8F5] border border-[#DCE8DC]">
                      <div className="flex items-center gap-1 text-[10px] font-semibold text-[#2F5933] mb-1">
                        <Check className="w-3 h-3 text-[#3E7444]" />
                        <span>SYNTHESIS VERDICT</span>
                      </div>
                      <p className="text-xs text-[#2A472E] leading-relaxed">
                        {row.verdict}
                      </p>
                    </div>
                  ) : (
                    <span className="inline-flex items-center gap-1 text-[11px] text-[#8C8A81] italic">
                      <Sparkles className="w-3 h-3 text-[#B0AD9E]" />
                      Synthesizing trade-off...
                    </span>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
};
