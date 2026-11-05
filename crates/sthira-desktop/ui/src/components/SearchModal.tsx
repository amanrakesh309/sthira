import React, { useState, useEffect, useRef, useCallback } from 'react';
import { Search, X, Folder, FileText, ArrowRight } from 'lucide-react';
import { searchAll } from '../api/ipc';
import type { SearchResultItem } from '../types/sthira';

interface SearchModalProps {
  isOpen: boolean;
  onClose: () => void;
  onOpenProject: (projectId: string) => void;
  onOpenArtifact: (artifactId: string) => void;
  openerElement?: HTMLElement | null;
}

export const SearchModal: React.FC<SearchModalProps> = ({
  isOpen,
  onClose,
  onOpenProject,
  onOpenArtifact,
  openerElement,
}) => {
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<SearchResultItem[]>([]);
  const [isSearching, setIsSearching] = useState(false);
  const [selectedIndex, setSelectedIndex] = useState<number>(-1);
  const openerRef = useRef<HTMLElement | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // Capture opener element on open, focus input, and reset search state
  useEffect(() => {
    if (isOpen) {
      if (openerElement) {
        openerRef.current = openerElement;
      } else {
        const active = document.activeElement as HTMLElement | null;
        if (active && active !== document.body && !active.closest('[role="dialog"]')) {
          openerRef.current = active;
        }
      }
      setQuery('');
      setResults([]);
      setSelectedIndex(-1);
      inputRef.current?.focus();
    }
  }, [isOpen, openerElement]);

  const handleClose = useCallback(() => {
    const targetToFocus = openerRef.current;
    onClose();
    if (targetToFocus && typeof targetToFocus.focus === 'function') {
      // Use queueMicrotask / setTimeout to restore focus once modal unmounts
      setTimeout(() => {
        targetToFocus.focus();
      }, 0);
    }
  }, [onClose]);

  useEffect(() => {
    if (!isOpen) return;

    const timer = setTimeout(async () => {
      if (query.trim()) {
        setIsSearching(true);
        try {
          const res = await searchAll(query);
          setResults(res);
          setSelectedIndex(res.length > 0 ? 0 : -1);
        } catch (err) {
          console.error('Search failed', err);
          setResults([]);
          setSelectedIndex(-1);
        } finally {
          setIsSearching(false);
        }
      } else {
        setResults([]);
        setSelectedIndex(-1);
      }
    }, 150);

    return () => clearTimeout(timer);
  }, [query, isOpen]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        handleClose();
      } else if (e.key === 'ArrowDown') {
        e.preventDefault();
        setSelectedIndex((prev) => (results.length > 0 ? (prev + 1) % results.length : -1));
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        setSelectedIndex((prev) =>
          results.length > 0 ? (prev - 1 + results.length) % results.length : -1
        );
      } else if (e.key === 'Enter') {
        if (selectedIndex >= 0 && selectedIndex < results.length) {
          e.preventDefault();
          const item = results[selectedIndex];
          if (item.kind === 'project') {
            onOpenProject(item.id);
          } else {
            onOpenArtifact(item.id);
          }
          handleClose();
        }
      }
    },
    [handleClose, results, selectedIndex, onOpenProject, onOpenArtifact]
  );

  // Global escape listener in case focus leaves the input
  useEffect(() => {
    if (!isOpen) return;
    const globalKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        handleClose();
      }
    };
    window.addEventListener('keydown', globalKeyDown);
    return () => window.removeEventListener('keydown', globalKeyDown);
  }, [isOpen, handleClose]);

  if (!isOpen) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center pt-20 px-4 bg-black/40 backdrop-blur-sm animate-in fade-in duration-150"
      onClick={(e) => {
        if (e.target === e.currentTarget) handleClose();
      }}
      role="presentation"
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Search workspace"
        className="w-full max-w-xl bg-[var(--sthira-surface-raised)] border border-[var(--sthira-border)] rounded-2xl shadow-2xl overflow-hidden flex flex-col text-[var(--sthira-text)]"
      >
        {/* Search Input Row */}
        <div className="flex items-center px-4 py-3.5 border-b border-[var(--sthira-border)]">
          <Search className="w-4 h-4 text-[var(--sthira-text-muted)] mr-3 shrink-0" />
          <input
            ref={inputRef}
            type="text"
            aria-label="Search workspace entities"
            aria-activedescendant={
              selectedIndex >= 0 && results[selectedIndex]
                ? `search-item-${results[selectedIndex].id}`
                : undefined
            }
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Search projects, documents, and notes..."
            className="w-full text-sm text-[var(--sthira-text)] placeholder-[var(--sthira-text-muted)] bg-transparent outline-none"
          />
          {query && (
            <button
              type="button"
              aria-label="Clear search input"
              onClick={() => {
                setQuery('');
                inputRef.current?.focus();
              }}
              className="text-[var(--sthira-text-muted)] hover:text-[var(--sthira-text)] p-1 mr-1 cursor-pointer"
            >
              <X className="w-3.5 h-3.5" />
            </button>
          )}
          <button
            type="button"
            aria-label="Close search"
            onClick={handleClose}
            className="text-xs px-2 py-1 rounded bg-[var(--sthira-surface-muted)] text-[var(--sthira-text-muted)] hover:text-[var(--sthira-text)] font-medium cursor-pointer"
          >
            Esc
          </button>
        </div>

        {/* Results List */}
        <div
          role="listbox"
          aria-label="Search results"
          className="max-h-96 overflow-y-auto p-3 space-y-1"
        >
          {isSearching && (
            <div className="text-xs text-[var(--sthira-text-muted)] py-6 text-center">
              Searching vault...
            </div>
          )}

          {!isSearching && query.trim() && results.length === 0 && (
            <div className="text-xs text-[var(--sthira-text-muted)] py-6 text-center">
              No matching projects or notes found for &ldquo;{query}&rdquo;.
            </div>
          )}

          {!isSearching && !query.trim() && (
            <div className="text-xs text-[var(--sthira-text-muted)] py-6 text-center">
              Type to search across projects, documents, and epistemic claims.
            </div>
          )}

          {results.map((item, index) => {
            const isSelected = index === selectedIndex;
            return (
              <div
                key={item.id}
                id={`search-item-${item.id}`}
                role="option"
                aria-selected={isSelected}
                tabIndex={0}
                aria-label={`Open ${item.title}`}
                onMouseEnter={() => setSelectedIndex(index)}
                onClick={() => {
                  if (item.kind === 'project') {
                    onOpenProject(item.id);
                  } else {
                    onOpenArtifact(item.id);
                  }
                  handleClose();
                }}
                className={`p-3 rounded-xl border transition-all cursor-pointer flex items-center justify-between group ${
                  isSelected
                    ? 'bg-[var(--sthira-surface-muted)] border-[var(--sthira-primary)] shadow-2xs'
                    : 'border-transparent hover:bg-[var(--sthira-canvas)] hover:border-[var(--sthira-border)]'
                }`}
              >
                <div className="flex items-start gap-3">
                  <div
                    className={`w-7 h-7 rounded-lg flex items-center justify-center shrink-0 mt-0.5 ${
                      isSelected
                        ? 'bg-[var(--sthira-primary)] text-white'
                        : 'bg-[var(--sthira-surface-muted)] text-[var(--sthira-text)]'
                    }`}
                  >
                    {item.kind === 'project' ? (
                      <Folder className="w-3.5 h-3.5" />
                    ) : (
                      <FileText className="w-3.5 h-3.5" />
                    )}
                  </div>
                  <div>
                    <h4 className="text-sm font-semibold text-[var(--sthira-text)]">
                      {item.title}
                    </h4>
                    {item.excerpt && (
                      <p className="text-xs text-[var(--sthira-text-muted)] mt-0.5 line-clamp-1">
                        {item.excerpt}
                      </p>
                    )}
                  </div>
                </div>
                <ArrowRight
                  className={`w-3.5 h-3.5 transition-all shrink-0 ml-3 ${
                    isSelected
                      ? 'text-[var(--sthira-primary)] translate-x-0.5'
                      : 'text-[var(--sthira-text-muted)] group-hover:text-[var(--sthira-text)] group-hover:translate-x-0.5'
                  }`}
                />
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
};
