import React, { useState, useRef, useEffect, useLayoutEffect, useCallback } from 'react';
import { createPortal } from 'react-dom';
import { ChevronDown, Check, Cpu, Sparkles, Cloud } from 'lucide-react';
import type { ProviderEntry, ProviderModelEntry, WebSearchCapability } from '../types/sthira';

export function getWebSearchBadgeConfig(capability?: WebSearchCapability) {
  switch (capability) {
    case 'native':
      return {
        label: '🌐 Native Search',
        bg: 'bg-cyan-50 dark:bg-cyan-950/40',
        text: 'text-cyan-700 dark:text-cyan-300',
        border: 'border-cyan-200 dark:border-cyan-800',
        tooltip: 'Provider features built-in native web search grounding.',
      };
    case 'agentic_tool_call':
      return {
        label: '🌐 Web Search Ready',
        bg: 'bg-emerald-50 dark:bg-emerald-950/40',
        text: 'text-emerald-700 dark:text-emerald-300',
        border: 'border-emerald-200 dark:border-emerald-800',
        tooltip: 'Model supports function calling for SearXNG / Brave search tools.',
      };
    case 'unsupported':
      return {
        label: '🚫 Offline Only',
        bg: 'bg-stone-100 dark:bg-stone-800',
        text: 'text-stone-500 dark:text-stone-400',
        border: 'border-stone-200 dark:border-stone-700',
        tooltip: 'Search not allowed or supported for this model.',
      };
    case 'engine_augmented_only':
    default:
      return {
        label: '📄 Web Context Injected',
        bg: 'bg-[#F4F1EA] dark:bg-[#2C2E27]',
        text: 'text-[#7D7A70] dark:text-[#A8A59B]',
        border: 'border-[#E2DED4] dark:border-[#3D4036]',
        tooltip: 'Sthira will automatically query SearXNG/Brave and inject scraped web pages into this model\'s prompt.',
      };
  }
}

export const WebSearchCapabilityBadge: React.FC<{
  capability?: WebSearchCapability;
  className?: string;
}> = ({ capability, className = '' }) => {
  const config = getWebSearchBadgeConfig(capability);
  const capKey = capability || 'engine_augmented_only';
  return (
    <span
      data-testid={`badge-capability-${capKey}`}
      className={`inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-medium border ${config.bg} ${config.text} ${config.border} ${className}`}
      title={config.tooltip}
    >
      {config.label}
    </span>
  );
};

export interface ModelSelectorProps {
  selectedModel?: ProviderModelEntry | null;
  selectedModelId?: string | null;
  onSelectModel: (model: ProviderModelEntry) => void;
  models?: ProviderModelEntry[];
  catalog?: ProviderEntry[];
  onOpenSettings?: () => void;
  className?: string;
  size?: 'sm' | 'md';
  disabled?: boolean;
  dataTestId?: string;
}

export const ModelSelector: React.FC<ModelSelectorProps> = ({
  selectedModel,
  selectedModelId,
  onSelectModel,
  models,
  catalog,
  onOpenSettings,
  className = '',
  size = 'md',
  disabled = false,
  dataTestId,
}) => {
  const [isOpen, setIsOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);
  const [placement, setPlacement] = useState<{
    top?: number;
    bottom?: number;
    left: number;
    width: number;
    maxHeight: number;
  }>({ top: 48, left: 12, width: 320, maxHeight: 320 });

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      const target = event.target as Node | null;
      if (
        target &&
        !containerRef.current?.contains(target) &&
        !dropdownRef.current?.contains(target)
      ) {
        setIsOpen(false);
      }
    };
    const handleEscapeCapture = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault();
        event.stopPropagation();
        setIsOpen(false);
        triggerRef.current?.focus();
      }
    };
    if (isOpen) {
      document.addEventListener('mousedown', handleClickOutside);
      document.addEventListener('keydown', handleEscapeCapture, true);
    }
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
      document.removeEventListener('keydown', handleEscapeCapture, true);
    };
  }, [isOpen]);

  // Determine all available models
  const allModels: ProviderModelEntry[] = React.useMemo(() => {
    if (models && models.length > 0) return models;
    if (catalog && catalog.length > 0) {
      const flat: ProviderModelEntry[] = [];
      for (const p of catalog) {
        if (p.is_enabled && p.models) {
          for (const m of p.models) {
            if (m.is_enabled) flat.push(m);
          }
        }
      }
      return flat;
    }
    return [];
  }, [models, catalog]);

  // Determine active model
  const activeModel = React.useMemo(() => {
    if (selectedModel) return selectedModel;
    if (selectedModelId) {
      return (
        allModels.find(
          (m) => m.id === selectedModelId || m.model_identifier === selectedModelId
        ) || null
      );
    }
    return allModels.find((m) => m.is_default) || allModels[0] || null;
  }, [selectedModel, selectedModelId, allModels]);

  const updatePosition = useCallback(() => {
    const trigger = triggerRef.current;
    if (!trigger) return;
    const rect = trigger.getBoundingClientRect();
    const vw = typeof window !== 'undefined' ? window.innerWidth : 1024;
    const vh = typeof window !== 'undefined' ? window.innerHeight : 768;
    const width = Math.min(320, Math.max(200, vw - 24));
    const spaceBelow = vh - rect.bottom - 12;
    const spaceAbove = rect.top - 12;
    const preferredHeight = 320;
    const placeAbove = spaceBelow < Math.min(220, preferredHeight) && spaceAbove > spaceBelow;
    const maxHeight = Math.max(120, Math.min(preferredHeight, placeAbove ? spaceAbove : spaceBelow));
    const left = Math.max(12, Math.min(rect.left, vw - width - 12));
    setPlacement(
      placeAbove
        ? { bottom: vh - rect.top + 6, top: undefined, left, width, maxHeight }
        : { top: rect.bottom + 6, bottom: undefined, left, width, maxHeight }
    );
  }, []);

  useLayoutEffect(() => {
    if (!isOpen) return;
    updatePosition();
    window.addEventListener('resize', updatePosition);
    window.addEventListener('scroll', updatePosition, true);
    return () => {
      window.removeEventListener('resize', updatePosition);
      window.removeEventListener('scroll', updatePosition, true);
    };
  }, [isOpen, allModels.length, updatePosition]);

  const getProviderIcon = (providerId?: string) => {
    if (!providerId) return <Cpu className="w-3.5 h-3.5" />;
    const p = providerId.toLowerCase();
    if (p.includes('ollama')) return <Cpu className="w-3.5 h-3.5 text-stone-600 dark:text-stone-300" />;
    if (p.includes('anthropic') || p.includes('claude'))
      return <Sparkles className="w-3.5 h-3.5 text-amber-600 dark:text-amber-400" />;
    return <Cloud className="w-3.5 h-3.5 text-blue-600 dark:text-blue-400" />;
  };

  const displayName = activeModel
    ? activeModel.display_name || activeModel.model_identifier
    : 'Select Model';

  const dropdownContent = isOpen ? (
    <div
      ref={dropdownRef}
      data-testid="model-selector-dropdown"
      style={{
        position: 'fixed',
        top: placement.top,
        bottom: placement.bottom,
        left: placement.left,
        width: placement.width,
        maxHeight: placement.maxHeight,
        zIndex: 80,
      }}
      className="overflow-y-auto rounded-xl bg-white dark:bg-[#1E201C] border border-[#E2DED4] dark:border-[#3D4036] shadow-xl p-1.5 space-y-1 animate-in fade-in zoom-in-95 duration-100"
    >
      <div className="px-2 py-1 text-[10px] font-semibold text-[#8C8A82] dark:text-[#A8A59B] uppercase tracking-wider flex items-center justify-between border-b border-[#EFECE4] dark:border-[#2C2E27] pb-1.5 mb-1">
        <span>Available Models</span>
        <span>{allModels.length} ready</span>
      </div>

      {allModels.length === 0 ? (
        <div className="p-3 text-center text-xs text-[#8C8A82] dark:text-[#A8A59B]">
          No models available
        </div>
      ) : (
        allModels.map((m) => {
          const isSelected = activeModel?.id === m.id;
          return (
            <button
              key={m.id}
              type="button"
              data-testid={`model-option-${m.id}`}
              onClick={() => {
                onSelectModel(m);
                setIsOpen(false);
                triggerRef.current?.focus();
              }}
              className={`w-full text-left px-2.5 py-2 rounded-lg text-xs flex items-center justify-between transition-colors cursor-pointer ${
                isSelected
                  ? 'bg-[#282C25] dark:bg-[#383B32] text-white'
                  : 'hover:bg-[#EFECE4] dark:hover:bg-[#252820] text-[#1E201C] dark:text-[#EDEAE2]'
              }`}
            >
              <div className="min-w-0 pr-2">
                <div className="flex items-center gap-1.5">
                  <span className="shrink-0">{getProviderIcon(m.provider_id)}</span>
                  <span className="font-medium truncate">{m.display_name}</span>
                </div>
                <div
                  className={`text-[10px] font-mono truncate pl-5 ${
                    isSelected ? 'text-stone-300' : 'text-[#8C8A82] dark:text-[#A8A59B]'
                  }`}
                >
                  {m.model_identifier}
                </div>
              </div>
              <div className="flex items-center gap-1.5 shrink-0">
                <WebSearchCapabilityBadge
                  capability={m.web_search_capability}
                  className={isSelected ? '!bg-white/10 !border-white/20 !text-white' : ''}
                />
                {isSelected && <Check className="w-3.5 h-3.5 text-white ml-0.5 shrink-0" />}
              </div>
            </button>
          );
        })
      )}

      {onOpenSettings && (
        <div className="pt-1.5 border-t border-[#EFECE4] dark:border-[#2C2E27] mt-1">
          <button
            type="button"
            onClick={() => {
              setIsOpen(false);
              onOpenSettings();
            }}
            className="w-full text-left px-2.5 py-1.5 text-[11px] font-medium text-[#5C5F58] dark:text-[#B0ADA4] hover:text-black dark:hover:text-white hover:bg-[#F2EFE8] dark:hover:bg-[#252820] rounded-lg transition-colors cursor-pointer flex items-center justify-between"
          >
            <span>Manage Providers & Models</span>
            <span>⚙️</span>
          </button>
        </div>
      )}
    </div>
  ) : null;

  return (
    <div className={`relative inline-block text-left ${className}`} ref={containerRef}>
      {/* Active Selector Chip */}
      <button
        ref={triggerRef}
        type="button"
        disabled={disabled}
        aria-haspopup="listbox"
        aria-expanded={isOpen}
        onClick={() => setIsOpen((prev) => !prev)}
        data-testid={dataTestId || 'model-selector-chip'}
        className={`flex items-center gap-1.5 rounded-lg border border-[#E2DED4] dark:border-[#3D4036] bg-[#FAF8F5] dark:bg-[#1C1E19] text-[#282C25] dark:text-[#EDEAE2] hover:bg-[#F2EFE8] dark:hover:bg-[#252820] transition-colors cursor-pointer ${
          size === 'sm' ? 'px-2 py-1 text-xs' : 'px-2.5 py-1.5 text-xs'
        } ${disabled ? 'opacity-50 cursor-not-allowed' : ''}`}
      >
        <span className="shrink-0">{getProviderIcon(activeModel?.provider_id)}</span>
        <span className="font-medium truncate max-w-[140px] sm:max-w-[180px]">
          {displayName}
        </span>
        {activeModel && (
          <WebSearchCapabilityBadge
            capability={activeModel.web_search_capability}
            className="hidden sm:inline-flex shrink-0"
          />
        )}
        <ChevronDown
          className={`w-3.5 h-3.5 text-[#8C8A82] transition-transform duration-150 shrink-0 ${
            isOpen ? 'rotate-180' : ''
          }`}
        />
      </button>

      {/* Dropdown Menu portaled to body so overflow-clipped ancestors never cut it off */}
      {dropdownContent && typeof document !== 'undefined' ? createPortal(dropdownContent, document.body) : null}
    </div>
  );
};
