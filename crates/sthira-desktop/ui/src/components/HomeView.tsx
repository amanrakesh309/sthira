import React, { useState } from 'react';
import {
  Folder,
  FileText,
  LayoutGrid,
  Search,
  Sparkles,
  SlidersHorizontal,
  Plus,
  Play,
  MoreHorizontal,
  FolderPlus,
} from 'lucide-react';
import { UniversalInputBar } from './UniversalInputBar';
import type {
  HomeStateView,
  WorkspaceIndexTab,
  WebSearchMode,
  WebResearchResult,
} from '../types/sthira';

interface HomeViewProps {
  homeState: HomeStateView;
  onOpenProject: (projectId: string) => void;
  onOpenArtifact: (artifactId: string, projectId?: string | null) => void;
  onOpenCanvas: (canvasId?: string) => void;
  onPromoteLooseCapture?: (artifactId: string) => void;
  onRefresh: () => void;
  onOpenSearch?: () => void;
  onOpenSettings?: () => void;
  onOpenAccount?: () => void;
  onOpenFlow: (mode: 'research' | 'plan' | 'capture', initialPrompt?: string) => void;
  onNavigateWorkspaceIndex?: (tab?: WorkspaceIndexTab) => void;
  /** Phase 20: research/plan stream into the center workspace instead of a side rail. */
  onStartResearch?: (
    query: string,
    mode: WebSearchMode,
    projectId?: string | null
  ) => Promise<WebResearchResult | null>;
  onStartPlan?: (goal: string, projectId?: string | null) => Promise<string | null>;
}

export const HomeView: React.FC<HomeViewProps> = ({
  homeState,
  onOpenProject,
  onOpenArtifact,
  onOpenCanvas,
  onRefresh,
  onOpenSearch,
  onOpenSettings,
  onOpenFlow,
  onNavigateWorkspaceIndex,
  onStartResearch,
  onStartPlan,
}) => {
  const [isNewMenuOpen, setIsNewMenuOpen] = useState(false);

  // Deduplicate recent_items vs loose_captures so the exact same note does not appear twice
  const looseCaptureIds = new Set((homeState.loose_captures || []).map((c) => c.id));
  const dedupedRecentItems = (homeState.recent_items || []).filter(
    (item) => !item.artifact_id || !looseCaptureIds.has(item.artifact_id)
  );

  // Cap sections at max 3 items
  const activeProjects = homeState.projects.slice(0, 3);
  const activeRecentItems = dedupedRecentItems.slice(0, 3);

  const formatDate = (timestamp?: number | string | null) => {
    if (!timestamp) return 'Recent';
    try {
      const date = typeof timestamp === 'number' ? new Date(timestamp) : new Date(timestamp);
      if (isNaN(date.getTime())) return 'Recent';
      return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
    } catch {
      return 'Recent';
    }
  };

  return (
    <div className="min-h-screen bg-[#F5F3EC] dark:bg-[#121310] text-[#1E201C] dark:text-[#EDEAE2] flex flex-col items-center transition-colors relative overflow-x-hidden">
      {/* Background Decorative Mountain Wash (localized to low-contrast header area) */}
      <div
        className="absolute top-0 right-0 w-full max-w-4xl h-80 pointer-events-none opacity-[0.07] dark:opacity-[0.04] overflow-hidden z-0"
        aria-hidden="true"
      >
        <svg
          viewBox="0 0 1000 300"
          preserveAspectRatio="none"
          className="w-full h-full fill-[#49583D] dark:fill-[#879E78]"
        >
          <path d="M0 300 L0 180 Q250 80 500 190 T1000 120 L1000 300 Z" />
          <path d="M0 300 L0 230 Q300 140 600 240 T1000 170 L1000 300 Z" opacity="0.6" />
        </svg>
      </div>

      {/* 1. Unified Top Navigation Bar */}
      <header className="w-full max-w-5xl px-6 pt-5 pb-3 flex items-center justify-between z-10">
        <div className="flex items-center gap-2.5">
          {/* Breadcrumb / Title Context */}
          <span className="text-xs font-medium text-[#73766D] dark:text-[#8C8F85] tracking-wide">
            Workspace
          </span>
        </div>

        <div className="flex items-center gap-2.5">
          {/* Command Palette Button */}
          <button
            type="button"
            data-testid="home-search-btn"
            onClick={onOpenSearch}
            className="flex items-center gap-2 px-3.5 py-1.5 rounded-xl bg-white dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] text-xs text-[#62665D] dark:text-[#B1B5AB] hover:text-[#1E201C] dark:hover:text-[#EDEAE2] hover:border-[#D8D4C8] shadow-2xs transition-all cursor-pointer"
          >
            <Search className="w-3.5 h-3.5 text-[#62665D] dark:text-[#B1B5AB]" />
            <span className="hidden sm:inline">Search your knowledge...</span>
            <kbd className="text-[10px] bg-[#F1EEE6] dark:bg-[#262822] px-1.5 py-0.5 rounded text-[#1E201C] dark:text-[#D4D1C7] font-mono border border-[#E8E5DC] dark:border-[#383B32]">
              ⌘K
            </kbd>
          </button>

          {/* Model Settings Button */}
          <button
            type="button"
            data-testid="home-model-chip"
            onClick={onOpenSettings}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl bg-white dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] text-xs text-[#1E201C] dark:text-[#EDEAE2] hover:border-[#D8D4C8] shadow-2xs transition-all cursor-pointer"
            title="Configure AI Models & Compute Providers"
          >
            <SlidersHorizontal className="w-3.5 h-3.5 text-[#62665D] dark:text-[#B1B5AB]" />
            <span className="font-medium hidden sm:inline">Model Settings</span>
          </button>

          {/* Quick Settings Icon Button */}
          <button
            type="button"
            data-testid="home-settings-btn"
            onClick={onOpenSettings}
            className="p-2 rounded-xl bg-white dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] text-[#62665D] dark:text-[#B1B5AB] hover:text-[#1E201C] dark:hover:text-[#EDEAE2] hover:border-[#D8D4C8] shadow-2xs transition-all cursor-pointer"
            title="Settings"
          >
            <SlidersHorizontal className="w-3.5 h-3.5" />
          </button>

          {/* Primary + New Dropdown */}
          <div className="relative">
            <button
              type="button"
              onClick={() => setIsNewMenuOpen((prev) => !prev)}
              className="flex items-center gap-1.5 px-3.5 py-1.5 rounded-xl bg-[#49583D] dark:bg-[#2F3E28] text-white hover:bg-[#39472F] transition-all text-xs font-semibold shadow-xs cursor-pointer"
            >
              <Plus className="w-3.5 h-3.5 stroke-[2.5]" />
              <span>New</span>
              <span className="text-[10px]">▾</span>
            </button>

            {isNewMenuOpen && (
              <div
                className="absolute right-0 mt-1.5 w-48 bg-white dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-xl shadow-lg z-50 p-1.5 space-y-0.5 animate-in fade-in"
                onMouseLeave={() => setIsNewMenuOpen(false)}
              >
                <button
                  type="button"
                  onClick={() => {
                    setIsNewMenuOpen(false);
                    onOpenFlow('capture');
                  }}
                  className="w-full flex items-center gap-2 px-3 py-2 rounded-lg text-xs text-[#1E201C] dark:text-[#EDEAE2] hover:bg-[#F1EEE6] dark:hover:bg-[#23251F] text-left cursor-pointer"
                >
                  <FileText className="w-3.5 h-3.5 text-[#62665D] dark:text-[#B1B5AB]" />
                  <span>New Note</span>
                </button>
                <button
                  type="button"
                  onClick={() => {
                    setIsNewMenuOpen(false);
                    onOpenFlow('research');
                  }}
                  className="w-full flex items-center gap-2 px-3 py-2 rounded-lg text-xs text-[#1E201C] dark:text-[#EDEAE2] hover:bg-[#F1EEE6] dark:hover:bg-[#23251F] text-left cursor-pointer"
                >
                  <Search className="w-3.5 h-3.5 text-[#49583D] dark:text-[#879E78]" />
                  <span>New Research</span>
                </button>
                <button
                  type="button"
                  onClick={() => {
                    setIsNewMenuOpen(false);
                    onOpenFlow('plan');
                  }}
                  className="w-full flex items-center gap-2 px-3 py-2 rounded-lg text-xs text-[#1E201C] dark:text-[#EDEAE2] hover:bg-[#F1EEE6] dark:hover:bg-[#23251F] text-left cursor-pointer"
                >
                  <Sparkles className="w-3.5 h-3.5 text-[#BE8B41]" />
                  <span>New Plan</span>
                </button>
              </div>
            )}
          </div>
        </div>
      </header>

      {/* Main Container */}
      <main className="w-full max-w-5xl px-6 pt-6 pb-16 flex flex-col space-y-9 z-10">
        {/* 2. Hero Section */}
        <section className="pt-2">
          <div className="flex items-center gap-2 mb-2">
            <span className="w-5 h-0.5 bg-[#BE8B41] dark:bg-[#D3A15D] rounded-full inline-block" />
            <span className="text-[11px] font-bold text-[#BE8B41] dark:text-[#D3A15D] uppercase tracking-widest">
              Welcome to Sthira
            </span>
          </div>
          <h1 className="text-4xl sm:text-[42px] font-bold font-editorial text-[#1E201C] dark:text-[#EDEAE2] tracking-tight leading-[1.15]">
            Make room to think.
          </h1>
          <p className="text-base text-[#62665D] dark:text-[#B1B5AB] mt-2 font-normal">
            Your knowledge, projects and ideas in one place.
          </p>
        </section>

        {/* 3. Universal Input Card */}
        <section>
          <UniversalInputBar
            onProjectSelect={onOpenProject}
            onOpenArtifact={onOpenArtifact}
            onRefresh={onRefresh}
            onOpenFlow={onOpenFlow}
            onOpenSettings={onOpenSettings}
            projects={homeState.projects}
            onStartResearch={onStartResearch}
            onStartPlan={onStartPlan}
          />
        </section>

        {/* 4. Your Projects Section (Max 2-3 active threads) */}
        <section>
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-xl font-bold font-editorial text-[#1E201C] dark:text-[#EDEAE2]">
              Your projects
            </h2>
            {homeState.projects.length > 0 && (
              <button
                type="button"
                data-testid="home-view-all-projects"
                onClick={() => onNavigateWorkspaceIndex?.('projects')}
                className="text-xs text-[#62665D] dark:text-[#B1B5AB] hover:text-[#1E201C] dark:hover:text-white font-medium transition-colors cursor-pointer"
              >
                View all projects →
              </button>
            )}
          </div>

          {activeProjects.length > 0 ? (
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
              {activeProjects.map((proj) => {
                return (
                  <div
                    key={proj.id}
                    onClick={() => onOpenProject(proj.id)}
                    className="bg-white dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-2xl p-5 hover:border-[#D8D4C8] dark:hover:border-[#3B4037] hover:shadow-xs transition-all cursor-pointer flex flex-col justify-between group relative overflow-hidden"
                  >
                    {/* Subtle corner contour */}
                    <div
                      className="absolute -bottom-6 -right-6 w-20 h-20 rounded-full bg-[#FAF8F3] dark:bg-[#23251F] opacity-70 pointer-events-none"
                      aria-hidden="true"
                    />

                    <div>
                      <div className="flex items-start justify-between mb-3">
                        <div className="w-9 h-9 rounded-xl bg-[#FDFBF7] dark:bg-[#23251F] border border-[#E8E5DC] dark:border-[#2C2E27] text-[#BE8B41] dark:text-[#D3A15D] flex items-center justify-center shrink-0 shadow-2xs">
                          <Folder className="w-4 h-4" />
                        </div>
                        <button
                          type="button"
                          onClick={(e) => {
                            e.stopPropagation();
                            onOpenProject(proj.id);
                          }}
                          className="text-[#73766D] dark:text-[#8C8F85] hover:text-[#1E201C] dark:hover:text-white p-1 rounded-md"
                          title="Project actions"
                        >
                          <MoreHorizontal className="w-4 h-4" />
                        </button>
                      </div>

                      <h3 className="font-bold text-[#1E201C] dark:text-[#EDEAE2] text-base group-hover:text-[#49583D] dark:group-hover:text-[#879E78] transition-colors leading-snug">
                        {proj.name}
                      </h3>
                      <p className="text-xs text-[#73766D] dark:text-[#8C8F85] mt-1 font-medium">
                        {proj.category || 'Project'} · {proj.item_count} {proj.item_count === 1 ? 'item' : 'items'}
                      </p>
                      <p className="text-xs text-[#62665D] dark:text-[#B1B5AB] mt-2.5 line-clamp-2 leading-relaxed">
                        {proj.purpose || 'Active workspace for focused investigations and documents.'}
                      </p>
                    </div>
                  </div>
                );
              })}
            </div>
          ) : (
            <div className="bg-white dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-2xl p-7 text-center shadow-2xs">
              <div className="w-11 h-11 rounded-xl bg-[#F1EEE6] dark:bg-[#22241F] text-[#49583D] dark:text-[#879E78] flex items-center justify-center mx-auto mb-3">
                <FolderPlus className="w-5 h-5" />
              </div>
              <h3 className="font-bold text-[#1E201C] dark:text-[#EDEAE2] text-sm">No projects yet</h3>
              <p className="text-xs text-[#62665D] dark:text-[#B1B5AB] mt-1 max-w-sm mx-auto leading-relaxed">
                Start by typing above or click{' '}
                <button
                  type="button"
                  onClick={() => onOpenFlow('research')}
                  className="font-bold text-[#49583D] dark:text-[#879E78] underline hover:opacity-85 cursor-pointer"
                >
                  Research
                </button>{' '}
                or{' '}
                <button
                  type="button"
                  onClick={() => onOpenFlow('plan')}
                  className="font-bold text-[#49583D] dark:text-[#879E78] underline hover:opacity-85 cursor-pointer"
                >
                  Plan
                </button>{' '}
                to begin your first workspace thread.
              </p>
            </div>
          )}
        </section>

        {/* 5. Pick up where you left off Section (Max 2-3 recent documents/canvases) */}
        <section>
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-xl font-bold font-editorial text-[#1E201C] dark:text-[#EDEAE2]">
              Pick up where you left off
            </h2>
            {activeRecentItems.length > 0 && (
              <button
                type="button"
                onClick={() => onNavigateWorkspaceIndex?.('notes')}
                className="text-xs text-[#62665D] dark:text-[#B1B5AB] hover:text-[#1E201C] dark:hover:text-white font-medium transition-colors cursor-pointer"
              >
                View all →
              </button>
            )}
          </div>

          {activeRecentItems.length > 0 ? (
            <div className="space-y-3">
              {activeRecentItems.map((item) => {
                const isCanvas = item.item_type === 'Canvas';
                return (
                  <div
                    key={item.id}
                    onClick={() => {
                      if (isCanvas) {
                        onOpenCanvas(item.canvas_id || undefined);
                      } else if (item.artifact_id) {
                        onOpenArtifact(item.artifact_id, item.project_id || null);
                      } else if (item.project_id) {
                        onOpenProject(item.project_id);
                      }
                    }}
                    className="w-full bg-white dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-2xl px-5 py-4 hover:border-[#D8D4C8] dark:hover:border-[#3B4037] hover:shadow-xs transition-all cursor-pointer flex items-center justify-between group"
                  >
                    <div className="flex items-center gap-4 min-w-0 flex-1 mr-4">
                      <div className="w-10 h-10 rounded-xl bg-[#FDFBF7] dark:bg-[#23251F] border border-[#E8E5DC] dark:border-[#2C2E27] text-[#62665D] dark:text-[#B1B5AB] flex items-center justify-center shrink-0 group-hover:text-[#49583D] dark:group-hover:text-[#879E78] transition-colors">
                        {isCanvas ? (
                          <LayoutGrid className="w-4 h-4" />
                        ) : (
                          <FileText className="w-4 h-4" />
                        )}
                      </div>
                      <div className="min-w-0 flex-1">
                        <h3 className="font-semibold text-[#1E201C] dark:text-[#EDEAE2] text-sm group-hover:text-[#49583D] dark:group-hover:text-[#879E78] transition-colors truncate">
                          {item.title}
                        </h3>
                        <p className="text-xs text-[#73766D] dark:text-[#8C8F85] mt-0.5 truncate">
                          <span className="font-medium text-[#62665D] dark:text-[#B1B5AB]">Continue:</span>{' '}
                          {item.subtitle || 'Review evidence and continue active drafting.'}
                        </p>
                      </div>
                    </div>

                    <div className="flex items-center gap-3 shrink-0">
                      <button
                        type="button"
                        onClick={(e) => {
                          e.stopPropagation();
                          if (isCanvas) {
                            onOpenCanvas(item.canvas_id || undefined);
                          } else if (item.artifact_id) {
                            onOpenArtifact(item.artifact_id, item.project_id || null);
                          }
                        }}
                        className="px-3.5 py-1.5 rounded-xl bg-[#EEF1E8] dark:bg-[#283427] text-[#49583D] dark:text-[#879E78] hover:bg-[#49583D] hover:text-white dark:hover:bg-[#879E78] dark:hover:text-[#121710] text-xs font-semibold flex items-center gap-1.5 transition-all cursor-pointer"
                      >
                        <Play className="w-3 h-3 fill-current" />
                        <span>Continue</span>
                      </button>

                      <span className="text-xs text-[#73766D] dark:text-[#8C8F85] w-14 text-right">
                        {formatDate(item.timestamp)}
                      </span>

                      <button
                        type="button"
                        onClick={(e) => e.stopPropagation()}
                        className="text-[#73766D] dark:text-[#8C8F85] hover:text-[#1E201C] dark:hover:text-white p-1 rounded-md"
                      >
                        <MoreHorizontal className="w-4 h-4" />
                      </button>
                    </div>
                  </div>
                );
              })}
            </div>
          ) : (
            <div className="bg-white dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-2xl p-6 text-center text-xs text-[#73766D] dark:text-[#8C8F85]">
              No active documents or canvases yet. Your recent work will appear here as you explore.
            </div>
          )}
        </section>
      </main>
    </div>
  );
};

export default HomeView;
