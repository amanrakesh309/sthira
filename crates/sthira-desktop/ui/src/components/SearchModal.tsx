import React, { useState, useEffect } from 'react';
import { Search, X, Folder, FileText, ArrowRight } from 'lucide-react';
import { searchAll } from '../api/ipc';
import type { SearchResultItem } from '../types/sthira';

interface SearchModalProps {
  isOpen: boolean;
  onClose: () => void;
  onOpenProject: (projectId: string) => void;
  onOpenArtifact: (artifactId: string) => void;
}

export const SearchModal: React.FC<SearchModalProps> = ({
  isOpen,
  onClose,
  onOpenProject,
  onOpenArtifact,
}) => {
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<SearchResultItem[]>([]);
  const [isSearching, setIsSearching] = useState(false);

  useEffect(() => {
    if (!isOpen) {
      setQuery('');
      setResults([]);
      return;
    }

    const timer = setTimeout(async () => {
      if (query.trim()) {
        setIsSearching(true);
        try {
          const res = await searchAll(query);
          setResults(res);
        } catch (err) {
          console.error('Search failed', err);
        } finally {
          setIsSearching(false);
        }
      } else {
        setResults([]);
      }
    }, 150);

    return () => clearTimeout(timer);
  }, [query, isOpen]);

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center pt-20 px-4 bg-black/40 backdrop-blur-sm animate-in fade-in duration-150">
      <div className="w-full max-w-xl bg-[#FFFFFF] dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-2xl shadow-2xl overflow-hidden flex flex-col">
        {/* Search Input Row */}
        <div className="flex items-center px-4 py-3.5 border-b border-[#ECE9E0] dark:border-[#2C2E27]">
          <Search className="w-4 h-4 text-[#6E6D66] dark:text-[#9E9C93] mr-3 shrink-0" />
          <input
            type="text"
            autoFocus
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search projects, documents, and notes..."
            className="w-full text-sm text-[#1E201C] dark:text-[#EDEAE2] placeholder-[#6E6D66] dark:placeholder-[#9E9C93] bg-transparent outline-none"
          />
          {query && (
            <button
              type="button"
              onClick={() => setQuery('')}
              className="text-[#6E6D66] dark:text-[#9E9C93] hover:text-[#1E201C] dark:hover:text-[#EDEAE2] p-1 mr-1"
            >
              <X className="w-3.5 h-3.5" />
            </button>
          )}
          <button
            type="button"
            onClick={onClose}
            className="text-xs px-2 py-1 rounded bg-[#EFECE4] dark:bg-[#252820] text-[#6E6D66] dark:text-[#9E9C93] hover:text-[#1E201C] dark:hover:text-[#EDEAE2] font-medium"
          >
            Esc
          </button>
        </div>

        {/* Results List */}
        <div className="max-h-96 overflow-y-auto p-3 space-y-1">
          {isSearching && (
            <div className="text-xs text-[#6E6D66] dark:text-[#9E9C93] py-6 text-center">Searching vault...</div>
          )}

          {!isSearching && query.trim() && results.length === 0 && (
            <div className="text-xs text-[#6E6D66] dark:text-[#9E9C93] py-6 text-center">
              No matching projects or notes found for &ldquo;{query}&rdquo;.
            </div>
          )}

          {!isSearching && !query.trim() && (
            <div className="text-xs text-[#6E6D66] dark:text-[#9E9C93] py-6 text-center">
              Type to search across projects, documents, and epistemic claims.
            </div>
          )}

          {results.map((item) => (
            <div
              key={item.id}
              onClick={() => {
                if (item.kind === 'project') {
                  onOpenProject(item.id);
                } else {
                  onOpenArtifact(item.id);
                }
                onClose();
              }}
              className="p-3 rounded-xl hover:bg-[#FAF8F3] dark:hover:bg-[#252820] border border-transparent hover:border-[#E8E5DC] dark:hover:border-[#2C2E27] transition-all cursor-pointer flex items-center justify-between group"
            >
              <div className="flex items-start gap-3">
                <div className="w-7 h-7 rounded-lg bg-[#EFECE4] dark:bg-[#252820] text-[#282C25] dark:text-[#EDEAE2] flex items-center justify-center shrink-0 mt-0.5">
                  {item.kind === 'project' ? (
                    <Folder className="w-3.5 h-3.5" />
                  ) : (
                    <FileText className="w-3.5 h-3.5" />
                  )}
                </div>
                <div>
                  <h4 className="text-sm font-semibold text-[#1E201C] dark:text-[#EDEAE2] group-hover:text-black dark:group-hover:text-white">
                    {item.title}
                  </h4>
                  {item.excerpt && (
                    <p className="text-xs text-[#6E6D66] dark:text-[#9E9C93] mt-0.5 line-clamp-1">
                      {item.excerpt}
                    </p>
                  )}
                </div>
              </div>
              <ArrowRight className="w-3.5 h-3.5 text-[#6E6D66] dark:text-[#9E9C93] group-hover:text-[#1E201C] dark:group-hover:text-[#EDEAE2] group-hover:translate-x-0.5 transition-all shrink-0 ml-3" />
            </div>
          ))}
        </div>
      </div>
    </div>
  );
};
