import React from 'react';
import type { EpistemicOrigin } from '../types/sthira';

interface EpistemicBadgeProps {
  origin: EpistemicOrigin;
  label?: string;
  className?: string;
}

export const EpistemicBadge: React.FC<EpistemicBadgeProps> = ({
  origin,
  label,
  className = '',
}) => {
  switch (origin) {
    case 'user_recorded':
      return (
        <span
          className={`inline-flex items-center px-2 py-0.5 rounded-md text-[11px] font-semibold bg-[#E3EBE3] text-[#2D5A27] border border-[#CCDCCC] tracking-wide ${className}`}
        >
          {label || 'You recorded'}
        </span>
      );
    case 'source_backed':
      return (
        <span
          className={`inline-flex items-center px-2 py-0.5 rounded-md text-[11px] font-semibold bg-[#E4EBF2] text-[#2B4C6F] border border-[#CDE0EE] tracking-wide ${className}`}
        >
          {label || 'Supported by source'}
        </span>
      );
    case 'ai_inferred':
    default:
      return (
        <span
          className={`inline-flex items-center px-2 py-0.5 rounded-md text-[11px] font-semibold bg-[#F9F5EC] text-[#8C6D1F] border border-dashed border-[#E5D7B7] tracking-wide ${className}`}
        >
          {label || 'Sthira inferred'}
        </span>
      );
  }
};
