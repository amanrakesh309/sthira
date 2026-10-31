import React from 'react';
import {
  Brain,
  Code,
  FileText,
  LayoutGrid,
  ArrowRight,
  FolderPlus,
  Search,
  Sparkles,
  SlidersHorizontal,
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
  // Deduplicate recent_items vs loose_captures so the exact same note does not appear twice
  const looseCaptureIds = new Set((homeState.loose_captures || []).map((c) => c.id));
  const dedupedRecentItems = (homeState.recent_items || []).filter(
    (item) => !item.artifact_id || !looseCaptureIds.has(item.artifact_id)
  );

  // Cap sections at max 3 items
  const activeProjects = homeState.projects.slice(0, 3);
  const activeRecentItems = dedupedRecentItems.slice(0, 3);

  return (
    <div className="min-h-screen bg-[#F5F3EC] dark:bg-[#121310] text-[#1E201C] dark:text-[#EDEAE2] flex flex-col items-center transition-colors">
      {/* 1. Unified Top Navigation Bar */}
      <header className="w-full max-w-5xl px-6 pt-5 pb-3 flex items-center justify-between border-b border-[#E8E5DC]/70 dark:border-[#2C2E27]/70">
        <div className="flex items-center gap-2.5">
          <div className="w-7 h-7 rounded-lg bg-[#282C25] dark:bg-[#EDEAE2] flex items-center justify-center shadow-xs">
            <svg
              className="w-4 h-4 text-white dark:text-[#121310]"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2.5"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5" />
            </svg>
          </div>
        </div>

        <div className="flex items-center gap-2">
          {/* Command Palette Button */}
          <button
            type="button"
            data-testid="home-search-btn"
            onClick={onOpenSearch}
            className="flex items-center gap-2 px-3 py-1.5 rounded-xl bg-[#FFFFFF] dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] text-xs text-[#6E6D66] dark:text-[#9E9C93] hover:text-[#1E201C] dark:hover:text-[#EDEAE2] hover:border-[#D8D4C8] shadow-2xs transition-all cursor-pointer"
          >
            <Search className="w-3.5 h-3.5" />
            <span className="hidden sm:inline">Search workspace...</span>
            <kbd className="text-[10px] bg-[#EFECE4] dark:bg-[#262822] px-1.5 py-0.5 rounded text-[#3A3B36] dark:text-[#D4D1C7] font-mono border border-[#E8E5DC] dark:border-[#383B32]">
              ⌘K
            </kbd>
          </button>

          {/* Model Selector Chip */}
          <button
            type="button"
            data-testid="home-model-chip"
            onClick={onOpenSettings}
            className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl bg-[#FFFFFF] dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] text-xs text-[#3A3B36] dark:text-[#D4D1C7] hover:border-[#D8D4C8] hover:text-[#1E201C] dark:hover:text-[#EDEAE2] shadow-2xs transition-all cursor-pointer"
            title="Configure AI Models"
          >
            <Sparkles className="w-3.5 h-3.5 text-amber-600 dark:text-amber-400" />
            <span className="font-medium">Model Settings</span>
          </button>

          {/* Settings Button */}
          <button
            type="button"
            data-testid="home-settings-btn"
            onClick={onOpenSettings}
            title="Settings"
            className="p-1.5 rounded-xl bg-[#FFFFFF] dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] text-[#6E6D66] dark:text-[#9E9C93] hover:text-[#1E201C] dark:hover:text-[#EDEAE2] hover:border-[#D8D4C8] shadow-2xs transition-all cursor-pointer"
          >
            <SlidersHorizontal className="w-4 h-4" />
          </button>
        </div>
      </header>

      {/* Main Container */}
      <main className="w-full max-w-5xl px-6 pt-8 pb-16 flex flex-col space-y-8">
        {/* 2. Hero Section */}
        <section className="pt-2">
          <h1 className="text-3xl font-bold text-[#1E201C] dark:text-[#EDEAE2] tracking-tight">
            Make room to think.
          </h1>
          <p className="text-base text-[#6E6D66] dark:text-[#9E9C93] mt-1.5 font-normal">
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
          <div className="flex items-center justify-between mb-3.5">
            <h2 className="text-lg font-bold text-[#1E201C] dark:text-[#EDEAE2]">Your projects</h2>
            {homeState.projects.length > 0 && (
              <button
                type="button"
                data-testid="home-view-all-projects"
                onClick={() => onNavigateWorkspaceIndex?.('projects')}
                className="text-sm text-[#6E6D66] dark:text-[#9E9C93] hover:text-[#1E201C] dark:hover:text-[#EDEAE2] font-medium transition-colors cursor-pointer"
              >
                View all →
              </button>
            )}
          </div>

          {activeProjects.length > 0 ? (
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
              {activeProjects.map((proj) => {
                const isBrain = proj.icon_type === 'brain' || proj.name.toLowerCase().includes('research');
                return (
                  <div
                    key={proj.id}
                    onClick={() => onOpenProject(proj.id)}
                    className="bg-[#FFFFFF] dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-2xl p-5 hover:border-[#D8D4C8] dark:hover:border-[#383B32] hover:shadow-md transition-all cursor-pointer flex flex-col justify-between group"
                  >
                    <div className="flex items-start gap-3.5">
                      <div className="w-9 h-9 rounded-xl bg-[#EFECE4] dark:bg-[#22241F] text-[#3A3B36] dark:text-[#D4D1C7] flex items-center justify-center shrink-0 group-hover:bg-[#E5E1D6] dark:group-hover:bg-[#2C2F28] transition-colors">
                        {isBrain ? (
                          <Brain className="w-4 h-4" />
                        ) : (
                          <Code className="w-4 h-4" />
                        )}
                      </div>
                      <div>
                        <h3 className="font-bold text-[#1E201C] dark:text-[#EDEAE2] text-base group-hover:text-black dark:group-hover:text-white transition-colors">
                          {proj.name}
                        </h3>
                        <p className="text-xs text-[#6E6D66] dark:text-[#9E9C93] mt-1 line-clamp-1">
                          {proj.category} · {proj.item_count} items
                        </p>
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
          ) : (
            <div className="bg-[#FFFFFF] dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-2xl p-6 text-center shadow-sm">
              <div className="w-10 h-10 rounded-xl bg-[#EFECE4] dark:bg-[#22241F] text-[#3A3B36] dark:text-[#D4D1C7] flex items-center justify-center mx-auto mb-2.5">
                <FolderPlus className="w-5 h-5 text-[#282C25] dark:text-[#D4D1C7]" />
              </div>
              <h3 className="font-bold text-[#1E201C] dark:text-[#EDEAE2] text-sm">No projects yet</h3>
              <p className="text-xs text-[#6E6D66] dark:text-[#9E9C93] mt-1 max-w-sm mx-auto leading-relaxed">
                Start by typing above or click{' '}
                <button
                  type="button"
                  onClick={() => onOpenFlow('research')}
                  className="font-bold text-[#1E201C] dark:text-[#EDEAE2] underline hover:opacity-80 cursor-pointer"
                >
                  Research
                </button>{' '}
                or{' '}
                <button
                  type="button"
                  onClick={() => onOpenFlow('plan')}
                  className="font-bold text-[#1E201C] dark:text-[#EDEAE2] underline hover:opacity-80 cursor-pointer"
                >
                  Plan something
                </button>{' '}
                to begin your first investigation.
              </p>
            </div>
          )}
        </section>

        {/* 5. Pick up where you left off Section (Max 2-3 recent documents/canvases) */}
        <section>
          <h2 className="text-lg font-bold text-[#1E201C] dark:text-[#EDEAE2] mb-3.5">
            Pick up where you left off
          </h2>

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
                    className="w-full bg-[#FFFFFF] dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-2xl px-5 py-4 hover:border-[#D8D4C8] dark:hover:border-[#383B32] hover:shadow-md transition-all cursor-pointer flex items-center justify-between group"
                  >
                    <div className="flex items-center gap-3.5">
                      <div className="w-9 h-9 rounded-xl bg-[#EFECE4] dark:bg-[#22241F] text-[#3A3B36] dark:text-[#D4D1C7] flex items-center justify-center shrink-0 group-hover:bg-[#E5E1D6] dark:group-hover:bg-[#2C2F28] transition-colors">
                        {isCanvas ? (
                          <LayoutGrid className="w-4 h-4" />
                        ) : (
                          <FileText className="w-4 h-4" />
                        )}
                      </div>
                      <div>
                        <h3 className="font-bold text-[#1E201C] dark:text-[#EDEAE2] text-sm group-hover:text-black dark:group-hover:text-white transition-colors">
                          {item.title}
                        </h3>
                        <p className="text-xs text-[#6E6D66] dark:text-[#9E9C93] mt-0.5">
                          {item.subtitle}
                        </p>
                      </div>
                    </div>
                    <ArrowRight className="w-4 h-4 text-[#6E6D66] dark:text-[#9E9C93] group-hover:text-[#1E201C] dark:group-hover:text-[#EDEAE2] group-hover:translate-x-0.5 transition-all" />
                  </div>
                );
              })}
            </div>
          ) : (
            <div className="bg-[#FFFFFF] dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-2xl p-5 text-center text-xs text-[#6E6D66] dark:text-[#9E9C93]">
              No active documents or canvases yet. Your recent work will appear here as you explore.
            </div>
          )}
        </section>
      </main>
    </div>
  );
};

export default HomeView;
