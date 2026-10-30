import React, { useState, useEffect, useRef } from 'react';
import {
  Heading1,
  Heading2,
  Heading3,
  CheckSquare,
  List,
  Table,
  Code,
  Info,
  CheckCircle2,
  HelpCircle,
  Sparkles,
  Search,
  Maximize2,
} from 'lucide-react';

import type { ObjectType } from '../types/sthira';

export type SlashCategory = 'formatting' | 'epistemic' | 'objects' | 'ai';

export interface SlashCommandItem {
  id: string;
  title: string;
  description: string;
  category: SlashCategory;
  icon: React.ReactNode;
  keywords: string[];
  template: string;
  isAiAction?: boolean;
  aiMode?: 'plan' | 'research' | 'expand';
  isObjectAction?: boolean;
  objectType?: ObjectType;
}

export const SLASH_COMMANDS: SlashCommandItem[] = [
  // Formatting Blocks
  {
    id: 'h1',
    title: 'Heading 1',
    description: 'High-level section title',
    category: 'formatting',
    icon: <Heading1 className="w-4 h-4 text-[#282C25] dark:text-[#EDEAE2]" />,
    keywords: ['h1', 'heading', 'title', 'large'],
    template: '# ',
  },
  {
    id: 'h2',
    title: 'Heading 2',
    description: 'Medium subsection title',
    category: 'formatting',
    icon: <Heading2 className="w-4 h-4 text-[#282C25] dark:text-[#EDEAE2]" />,
    keywords: ['h2', 'heading', 'subtitle', 'medium'],
    template: '## ',
  },
  {
    id: 'h3',
    title: 'Heading 3',
    description: 'Small topic heading',
    category: 'formatting',
    icon: <Heading3 className="w-4 h-4 text-[#282C25] dark:text-[#EDEAE2]" />,
    keywords: ['h3', 'heading', 'subtopic', 'small'],
    template: '### ',
  },
  {
    id: 'todo',
    title: 'To-do Checklist',
    description: 'Interactive checkbox item',
    category: 'formatting',
    icon: <CheckSquare className="w-4 h-4 text-emerald-700 dark:text-emerald-400" />,
    keywords: ['todo', 'checklist', 'task', 'check', 'box'],
    template: '- [ ] ',
  },
  {
    id: 'bullet',
    title: 'Bullet List',
    description: 'Simple bulleted list point',
    category: 'formatting',
    icon: <List className="w-4 h-4 text-[#282C25] dark:text-[#EDEAE2]" />,
    keywords: ['bullet', 'list', 'point', 'unordered'],
    template: '- ',
  },
  {
    id: 'table',
    title: 'Comparison Table',
    description: 'Structured Markdown table template',
    category: 'formatting',
    icon: <Table className="w-4 h-4 text-[#282C25] dark:text-[#EDEAE2]" />,
    keywords: ['table', 'matrix', 'comparison', 'grid', 'columns'],
    template: '| Criteria | Option A | Option B |\n| :--- | :--- | :--- |\n| Performance | High | Medium |\n| Complexity | Low | High |\n',
  },
  {
    id: 'code',
    title: 'Code Block',
    description: 'Code snippet with syntax highlight',
    category: 'formatting',
    icon: <Code className="w-4 h-4 text-[#282C25] dark:text-[#EDEAE2]" />,
    keywords: ['code', 'snippet', 'syntax', 'programming'],
    template: '```rust\n// Implementation\n```\n',
  },
  {
    id: 'callout',
    title: 'Callout Note',
    description: 'Highlighted note / invariant block',
    category: 'formatting',
    icon: <Info className="w-4 h-4 text-blue-700 dark:text-blue-400" />,
    keywords: ['callout', 'note', 'alert', 'info', 'highlight'],
    template: '> [!NOTE]\n> Key architectural invariant or observation.\n',
  },

  // Sthira Epistemic & Invariant Blocks
  {
    id: 'decision',
    title: 'Decision Node',
    description: 'Records a formal epistemic decision into knowledge graph',
    category: 'epistemic',
    icon: <CheckCircle2 className="w-4 h-4 text-emerald-600 dark:text-emerald-400" />,
    keywords: ['decision', 'epistemic', 'record', 'graph', 'choice'],
    template: '> **Decision:** [Document decision statement here]\n> *Rationale:* [Key tradeoff and reasoning]\n',
  },
  {
    id: 'question',
    title: 'Open Question',
    description: 'Registers an unresolved epistemic question',
    category: 'epistemic',
    icon: <HelpCircle className="w-4 h-4 text-amber-600 dark:text-amber-400" />,
    keywords: ['question', 'open', 'unknown', 'inquiry', 'epistemic'],
    template: '> **Open Question:** [Document unresolved inquiry here]\n',
  },

  // Inline AI Commands
  {
    id: 'plan',
    title: 'AI Multi-Phase Plan',
    description: 'Calls active LLM to stream a structured DAG plan at cursor',
    category: 'ai',
    icon: <Sparkles className="w-4 h-4 text-purple-600 dark:text-purple-400" />,
    keywords: ['plan', 'ai', 'milestone', 'dag', 'architecture'],
    template: '',
    isAiAction: true,
    aiMode: 'plan',
  },
  {
    id: 'research',
    title: 'AI Grounded Research',
    description: 'Calls active LLM + search to stream comparative research report',
    category: 'ai',
    icon: <Search className="w-4 h-4 text-indigo-600 dark:text-indigo-400" />,
    keywords: ['research', 'ai', 'investigate', 'compare', 'evidence'],
    template: '',
    isAiAction: true,
    aiMode: 'research',
  },
  {
    id: 'expand',
    title: 'AI Expand Section',
    description: 'Expands the current paragraph or topic with technical depth',
    category: 'ai',
    icon: <Maximize2 className="w-4 h-4 text-teal-600 dark:text-teal-400" />,
    keywords: ['expand', 'ai', 'elaborate', 'detail', 'deepen'],
    template: '',
    isAiAction: true,
    aiMode: 'expand',
  },
];

interface SlashCommandMenuProps {
  filterQuery: string;
  onSelect: (item: SlashCommandItem) => void;
  onClose: () => void;
  position?: { top: number; left: number } | null;
  activeModelName?: string;
  customObjectTypes?: ObjectType[];
}

export const SlashCommandMenu: React.FC<SlashCommandMenuProps> = ({
  filterQuery,
  onSelect,
  onClose,
  position,
  activeModelName = 'Active Model',
  customObjectTypes = [],
}) => {
  const [selectedIndex, setSelectedIndex] = useState(0);
  const containerRef = useRef<HTMLDivElement>(null);

  const cleanQuery = filterQuery.toLowerCase().replace(/^\//, '').trim();

  // Dynamically generate commands from custom & system object types
  const dynamicObjectCommands: SlashCommandItem[] = customObjectTypes.map((objType) => ({
    id: objType.id,
    title: objType.display_name,
    description: `Insert and link typed ${objType.display_name} object`,
    category: 'objects',
    icon: (
      <span
        className="w-4 h-4 flex items-center justify-center text-sm"
        style={{ color: objType.color || '#F59E0B' }}
      >
        {objType.icon || '📦'}
      </span>
    ),
    keywords: [
      objType.id.toLowerCase(),
      objType.display_name.toLowerCase(),
      'object',
      'typed',
      'custom',
    ],
    template: '',
    isObjectAction: true,
    objectType: objType,
  }));

  const allCommands = [...SLASH_COMMANDS, ...dynamicObjectCommands];

  const filteredCommands = allCommands.filter((cmd) => {
    if (!cleanQuery) return true;
    return (
      cmd.id.toLowerCase().includes(cleanQuery) ||
      cmd.title.toLowerCase().includes(cleanQuery) ||
      cmd.keywords.some((kw) => kw.toLowerCase().includes(cleanQuery))
    );
  });

  useEffect(() => {
    setSelectedIndex(0);
  }, [cleanQuery]);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        setSelectedIndex((prev) => (prev + 1) % Math.max(1, filteredCommands.length));
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        setSelectedIndex((prev) => (prev - 1 + filteredCommands.length) % Math.max(1, filteredCommands.length));
      } else if (e.key === 'Enter') {
        e.preventDefault();
        if (filteredCommands[selectedIndex]) {
          onSelect(filteredCommands[selectedIndex]);
        }
      } else if (e.key === 'Escape') {
        e.preventDefault();
        onClose();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [filteredCommands, selectedIndex, onSelect, onClose]);

  // Click outside to close
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [onClose]);

  if (filteredCommands.length === 0) {
    return null;
  }

  const categoryLabels: Record<SlashCategory, string> = {
    formatting: 'Formatting Blocks',
    epistemic: 'Sthira Epistemic & Invariants',
    objects: 'Custom & Typed Objects',
    ai: `Inline AI Compute (${activeModelName})`,
  };

  const categories: SlashCategory[] = ['formatting', 'epistemic', 'objects', 'ai'];

  return (
    <div
      ref={containerRef}
      id="slash-command-popover"
      style={
        position
          ? {
              top: `${position.top}px`,
              left: `${Math.min(position.left, window.innerWidth - 320)}px`,
            }
          : undefined
      }
      className="absolute z-50 w-76 bg-white dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-xl shadow-xl overflow-hidden font-sans text-xs animate-in fade-in zoom-in-95 duration-100"
    >
      <div className="px-3 py-2 border-b border-[#ECE9E0] dark:border-[#2C2E27] bg-[#FAF8F3] dark:bg-[#161714] flex items-center justify-between">
        <span className="text-[10px] font-bold text-[#6E6D66] dark:text-[#9E9C93] uppercase tracking-wider">
          Insert Block or Run Command
        </span>
        <span className="text-[10px] text-[#A09D94] dark:text-[#6E6D66] font-mono">
          esc to close
        </span>
      </div>

      <div className="max-h-72 overflow-y-auto p-1.5 space-y-2">
        {categories.map((cat) => {
          const catCommands = filteredCommands.filter((c) => c.category === cat);
          if (catCommands.length === 0) return null;

          return (
            <div key={cat} className="space-y-0.5">
              <div className="px-2 py-1 text-[10px] font-bold text-[#8C8A82] dark:text-[#6E6D66] uppercase tracking-wider">
                {categoryLabels[cat]}
              </div>
              {catCommands.map((item) => {
                const globalIndex = filteredCommands.indexOf(item);
                const isSelected = globalIndex === selectedIndex;

                return (
                  <button
                    key={item.id}
                    type="button"
                    data-testid={`slash-cmd-${item.id}`}
                    onClick={() => onSelect(item)}
                    onMouseEnter={() => setSelectedIndex(globalIndex)}
                    className={`w-full text-left px-2.5 py-1.5 rounded-lg flex items-center gap-2.5 transition-colors cursor-pointer ${
                      isSelected
                        ? 'bg-[#282C25] dark:bg-[#2A2D24] text-white dark:text-[#EDEAE2]'
                        : 'text-[#1E201C] dark:text-[#EDEAE2] hover:bg-[#EFECE4] dark:hover:bg-[#252820]'
                    }`}
                  >
                    <div
                      className={`w-6 h-6 rounded-md flex items-center justify-center shrink-0 ${
                        isSelected ? 'bg-white/20 text-white' : 'bg-[#EFECE4] dark:bg-[#252820] text-[#1E201C] dark:text-[#EDEAE2]'
                      }`}
                    >
                      {item.icon}
                    </div>
                    <div className="min-w-0 flex-1">
                      <div className="font-semibold text-xs truncate flex items-center gap-1.5">
                        <span>{item.title}</span>
                        {item.isAiAction && (
                          <span
                            className={`text-[9px] px-1 py-0.2 rounded font-normal ${
                              isSelected ? 'bg-white/30 text-white' : 'bg-purple-100 dark:bg-purple-950/80 text-purple-700 dark:text-purple-300'
                            }`}
                          >
                            AI
                          </span>
                        )}
                      </div>
                      <div
                        className={`text-[10px] truncate ${
                          isSelected ? 'text-zinc-300' : 'text-[#6E6D66] dark:text-[#9E9C93]'
                        }`}
                      >
                        {item.description}
                      </div>
                    </div>
                  </button>
                );
              })}
            </div>
          );
        })}
      </div>
    </div>
  );
};
