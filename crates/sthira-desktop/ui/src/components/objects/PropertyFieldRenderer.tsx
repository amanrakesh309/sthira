import React, { useState, useEffect } from 'react';
import { Calendar } from 'lucide-react';
import type { PropertyDefinition } from '../../types/sthira';

interface PropertyFieldRendererProps {
  definition: PropertyDefinition;
  value: any;
  onChange: (val: any) => void;
  isReadOnly?: boolean;
  compact?: boolean;
}

export const PropertyFieldRenderer: React.FC<PropertyFieldRendererProps> = ({
  definition,
  value,
  onChange,
  isReadOnly = false,
  compact = false,
}) => {
  const [isEditing, setIsEditing] = useState(false);
  const [internalVal, setInternalVal] = useState(value ?? '');

  useEffect(() => {
    setInternalVal(value ?? '');
  }, [value]);

  const handleBlurOrCommit = () => {
    setIsEditing(false);
    if (internalVal !== value) {
      onChange(internalVal);
    }
  };

  switch (definition.property_type) {
    case 'boolean': {
      const checked = Boolean(value);
      return (
        <label className="inline-flex items-center gap-1.5 cursor-pointer select-none">
          <input
            type="checkbox"
            checked={checked}
            disabled={isReadOnly}
            onChange={(e) => onChange(e.target.checked)}
            className="w-4 h-4 rounded border-[#D5D2C9] text-[#282C25] focus:ring-1 focus:ring-[#282C25] cursor-pointer accent-[#282C25]"
          />
          {!compact && (
            <span className="text-xs text-[#52544D]">
              {checked ? 'Yes' : 'No'}
            </span>
          )}
        </label>
      );
    }

    case 'select': {
      const options = definition.options || [];
      const currentVal = value || '';

      const getOptionColor = (opt: string) => {
        const lower = opt.toLowerCase();
        if (['spark', 'exploring', 'low', 'unread'].includes(lower)) return 'bg-amber-100 text-amber-800 border-amber-200';
        if (['validated', 'high', 'processed', 'done'].includes(lower)) return 'bg-emerald-100 text-emerald-800 border-emerald-200';
        if (['dormant', 'medium', 'reading'].includes(lower)) return 'bg-blue-100 text-blue-800 border-blue-200';
        return 'bg-[#EFECE4] text-[#3A3B36] border-[#E8E5DC]';
      };

      if (isReadOnly) {
        return (
          <span
            className={`inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium border ${getOptionColor(
              currentVal
            )}`}
          >
            {currentVal || '—'}
          </span>
        );
      }

      return (
        <select
          value={currentVal}
          onChange={(e) => onChange(e.target.value)}
          className={`text-xs px-2 py-1 rounded-md border font-medium cursor-pointer outline-none transition-colors ${getOptionColor(
            currentVal
          )}`}
        >
          <option value="">Select...</option>
          {options.map((opt) => (
            <option key={opt} value={opt}>
              {opt.charAt(0).toUpperCase() + opt.slice(1)}
            </option>
          ))}
        </select>
      );
    }

    case 'date': {
      if (isReadOnly) {
        return (
          <span className="inline-flex items-center gap-1 text-xs text-[#52544D]">
            <Calendar className="w-3 h-3 text-[#8C8A82]" />
            {value ? String(value) : '—'}
          </span>
        );
      }

      return (
        <div className="inline-flex items-center gap-1.5">
          <input
            type="date"
            value={value ? String(value) : ''}
            onChange={(e) => onChange(e.target.value)}
            className="text-xs px-2 py-0.5 rounded border border-[#E8E5DC] bg-white text-[#1E201C] outline-none focus:border-[#282C25]"
          />
        </div>
      );
    }

    case 'url': {
      const urlStr = String(value || '');
      if (!isEditing && urlStr) {
        return (
          <div className="inline-flex items-center gap-1.5 max-w-[200px] group">
            <a
              href={urlStr.startsWith('http') ? urlStr : `https://${urlStr}`}
              target="_blank"
              rel="noreferrer"
              className="text-xs text-blue-600 hover:text-blue-800 underline truncate max-w-[170px]"
              title={urlStr}
            >
              {urlStr.replace(/^https?:\/\//, '')}
            </a>
            {!isReadOnly && (
              <button
                type="button"
                onClick={() => setIsEditing(true)}
                className="opacity-0 group-hover:opacity-100 text-[#8C8A82] hover:text-[#1E201C] text-[10px] cursor-pointer"
                title="Edit URL"
              >
                ✎
              </button>
            )}
          </div>
        );
      }

      if (isReadOnly) {
        return <span className="text-xs text-[#8C8A82]">None</span>;
      }

      return (
        <div className="inline-flex items-center gap-1">
          <input
            type="text"
            value={internalVal}
            placeholder="https://..."
            onChange={(e) => setInternalVal(e.target.value)}
            onBlur={handleBlurOrCommit}
            onKeyDown={(e) => e.key === 'Enter' && handleBlurOrCommit()}
            autoFocus={isEditing}
            className="text-xs px-2 py-0.5 rounded border border-[#E8E5DC] bg-white text-[#1E201C] outline-none focus:border-[#282C25] w-full"
          />
        </div>
      );
    }

    case 'number': {
      if (isReadOnly) {
        return (
          <span className="text-xs font-mono text-[#1E201C]">
            {value !== undefined && value !== null && value !== '' ? String(value) : '—'}
          </span>
        );
      }

      return (
        <input
          type="number"
          value={internalVal}
          onChange={(e) => setInternalVal(e.target.value)}
          onBlur={handleBlurOrCommit}
          onKeyDown={(e) => e.key === 'Enter' && handleBlurOrCommit()}
          className="text-xs px-2 py-0.5 rounded border border-[#E8E5DC] bg-white font-mono text-[#1E201C] outline-none focus:border-[#282C25] w-20"
        />
      );
    }

    case 'text':
    default: {
      if (isReadOnly) {
        return (
          <span className="text-xs text-[#3A3B36] truncate max-w-[240px]" title={String(value || '')}>
            {value ? String(value) : '—'}
          </span>
        );
      }

      if (!isEditing && compact) {
        return (
          <div
            onClick={() => setIsEditing(true)}
            className="text-xs text-[#3A3B36] truncate max-w-[240px] px-1 py-0.5 rounded hover:bg-[#EFECE4] cursor-text min-h-[20px]"
            title="Click to edit"
          >
            {value ? String(value) : <span className="text-[#B0ACA2] italic">Empty</span>}
          </div>
        );
      }

      return (
        <input
          type="text"
          value={internalVal}
          onChange={(e) => setInternalVal(e.target.value)}
          onBlur={handleBlurOrCommit}
          onKeyDown={(e) => e.key === 'Enter' && handleBlurOrCommit()}
          autoFocus={isEditing}
          placeholder={`Enter ${definition.label.toLowerCase()}...`}
          className="text-xs px-2 py-0.5 rounded border border-[#E8E5DC] bg-white text-[#1E201C] outline-none focus:border-[#282C25] w-full"
        />
      );
    }
  }
};

export default PropertyFieldRenderer;
