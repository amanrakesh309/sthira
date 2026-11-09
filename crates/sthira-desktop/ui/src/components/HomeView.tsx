import React, { useEffect, useRef, useState } from 'react';
import {
  ArrowRight, ArrowUpRight, BookOpen, ChevronDown, FileText, Folder,
  FolderPlus, LayoutGrid, Plus, Search, SlidersHorizontal, Sparkles,
} from 'lucide-react';
import { UniversalInputBar } from './UniversalInputBar';
import { Button } from './ui';
import type {
  HomeStateView, RecentItemView, WorkspaceIndexTab, WebSearchMode, WebResearchResult,
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
  loadStatus?: 'loading' | 'ready' | 'error';
  onStartResearch?: (query: string, mode: WebSearchMode, projectId?: string | null, templateId?: string | null, modelId?: string | null) => Promise<WebResearchResult | null>;
  onStartPlan?: (goal: string, projectId?: string | null, templateId?: string | null, modelId?: string | null) => Promise<string | null>;
}

export const HomeView: React.FC<HomeViewProps> = ({
  homeState, onOpenProject, onOpenArtifact, onOpenCanvas, onRefresh,
  onOpenSettings, onOpenFlow, onNavigateWorkspaceIndex, onStartResearch,
  onStartPlan, loadStatus = 'ready',
}) => {
  const [isNewMenuOpen, setIsNewMenuOpen] = useState(false);
  const newMenuRef = useRef<HTMLDivElement>(null);
  const newButtonRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!isNewMenuOpen) return;
    const closeOutside = (event: MouseEvent) => {
      if (!newMenuRef.current?.contains(event.target as Node)) setIsNewMenuOpen(false);
    };
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setIsNewMenuOpen(false);
        newButtonRef.current?.focus();
      }
    };
    document.addEventListener('mousedown', closeOutside);
    document.addEventListener('keydown', closeOnEscape);
    return () => {
      document.removeEventListener('mousedown', closeOutside);
      document.removeEventListener('keydown', closeOnEscape);
    };
  }, [isNewMenuOpen]);

  const activeProjects = homeState.projects.slice(0, 3);
  // Loose captures already present in recent_items belong here too. Deduplicate
  // actual destinations, rather than excluding the notes most recently captured.
  const seen = new Set<string>();
  const recentItems = (homeState.recent_items || []).filter((item) => {
    const key = item.artifact_id || item.canvas_id || item.id;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  }).slice(0, 3);

  const openRecent = (item: RecentItemView) => {
    if (item.item_type === 'Canvas') onOpenCanvas(item.canvas_id || undefined);
    else if (item.artifact_id) onOpenArtifact(item.artifact_id, item.project_id || null);
    else if (item.project_id) onOpenProject(item.project_id);
  };
  const formatDate = (timestamp?: number | string | null) => {
    if (!timestamp) return 'Recent';
    const date = new Date(timestamp);
    return Number.isNaN(date.getTime()) ? 'Recent' : date.toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
  };

  return (
    <div className="sthira-home">
      <header className="home-toolbar" aria-label="Home actions">
        <span className="home-location">Workspace / Home</span>
        <div className="home-toolbar-actions">
          <Button type="button" data-testid="home-model-chip" onClick={onOpenSettings} icon={<SlidersHorizontal size={16} />}>
            Model Settings
          </Button>
          <div ref={newMenuRef} className="home-new">
            <button ref={newButtonRef} type="button" className="sthira-button-primary home-new-trigger"
              aria-expanded={isNewMenuOpen} aria-controls="home-new-options"
              onClick={() => setIsNewMenuOpen((open) => !open)}>
              <Plus size={16} /> New <ChevronDown size={14} />
            </button>
            {isNewMenuOpen && (
              <div id="home-new-options" className="home-new-options sthira-popover" aria-label="Create new">
                {([
                  ['capture', 'New Note', FileText], ['research', 'New Research', Search], ['plan', 'New Plan', BookOpen],
                ] as const).map(([mode, label, Icon]) => (
                  <button key={mode} type="button" onClick={() => { setIsNewMenuOpen(false); onOpenFlow(mode); }}>
                    <Icon size={16} /> {label}
                  </button>
                ))}
              </div>
            )}
          </div>
        </div>
      </header>

      <div className="home-content">
        <section className="home-hero" aria-labelledby="home-title">
          <p className="home-eyebrow">Welcome to Sthira</p>
          <h1 id="home-title">Make room to think.</h1>
          <p className="home-subtitle">Your knowledge, projects and ideas in one place.</p>
        </section>

        <section aria-label="Universal input">
          <UniversalInputBar onProjectSelect={onOpenProject} onOpenArtifact={onOpenArtifact}
            onRefresh={onRefresh} onOpenFlow={onOpenFlow} onOpenSettings={onOpenSettings}
            projects={homeState.projects} onStartResearch={onStartResearch} onStartPlan={onStartPlan} />
        </section>

        {loadStatus === 'error' && (
          <div className="home-load-notice" role="alert">
            <span>Couldn’t load your workspace. Your saved work is still in the vault.</span>
            <Button type="button" onClick={onRefresh}>Try again</Button>
          </div>
        )}
        {loadStatus === 'loading' && <p className="sthira-muted" role="status">Loading your workspace…</p>}

        <section aria-labelledby="home-projects-title" aria-busy={loadStatus === 'loading'}>
          <div className="home-section-heading">
            <h2 id="home-projects-title">Your projects</h2>
            {homeState.projects.length > 0 && (
              <button type="button" data-testid="home-view-all-projects" className="home-text-action"
                onClick={() => onNavigateWorkspaceIndex?.('projects')}>View all projects <ArrowRight size={16} /></button>
            )}
          </div>
          {activeProjects.length > 0 ? (
            <div className="home-project-grid">
              {activeProjects.map((project) => (
                <button type="button" key={project.id} className="home-project-card" aria-label={`Open project: ${project.name}`} onClick={() => onOpenProject(project.id)}>
                  <span className="home-project-heading">
                    <span className="home-project-icon"><Folder size={23} strokeWidth={1.5} /></span>
                    <span className="home-project-label">
                      <span className="home-card-title" title={project.name}>{project.name}</span>
                      <span className="home-card-meta">{project.category || 'Project'} · {project.item_count} {project.item_count === 1 ? 'item' : 'items'}</span>
                    </span>
                    <ArrowUpRight className="home-project-arrow" size={16} aria-hidden="true" />
                  </span>
                  <span className="home-project-description">{project.purpose || 'Open this project to review its pages and notes.'}</span>
                </button>
              ))}
            </div>
          ) : loadStatus === 'ready' && (
            <div className="home-empty-projects">
              <span className="home-project-icon"><FolderPlus size={23} strokeWidth={1.5} /></span>
              <div><h3>No projects yet</h3><p>Capture a thought above, or start a research or planning workspace.</p></div>
              <Button type="button" variant="outline" icon={<Plus size={16} />} iconPosition="right" onClick={() => onOpenFlow('research')}>Start a project</Button>
            </div>
          )}
        </section>

        <section aria-labelledby="home-resume-title">
          <div className="home-section-heading">
            <h2 id="home-resume-title">Pick up where you left off</h2>
            {recentItems.length > 0 && (
              <button type="button" className="home-text-action" onClick={() => onNavigateWorkspaceIndex?.('notes')}>View all <ArrowRight size={16} /></button>
            )}
          </div>
          {recentItems.length > 0 ? (
            <div className="home-resume-list">
              {recentItems.map((item) => {
                const isCanvas = item.item_type === 'Canvas';
                const canOpen = isCanvas || !!item.artifact_id || !!item.project_id;
                return (
                  <button type="button" key={item.id} className="home-resume-row" disabled={!canOpen} onClick={() => openRecent(item)}>
                    <span className="home-resume-icon">{isCanvas ? <LayoutGrid size={20} /> : <FileText size={20} strokeWidth={1.5} />}</span>
                    <span className="home-resume-copy">
                      <span className="home-card-title" title={item.title}>{item.title}</span>
                      <span className="home-card-meta">{item.subtitle || (isCanvas ? 'Open your saved canvas.' : 'Open your saved work to continue.')}</span>
                    </span>
                    <span className="home-continue"><ArrowRight size={15} /> {isCanvas ? 'Open' : 'Continue'}</span>
                    <span className="home-resume-date">{formatDate(item.timestamp)}</span>
                  </button>
                );
              })}
            </div>
          ) : loadStatus === 'ready' && (
            <div className="home-empty-resume"><FileText size={20} /><p>Your recent documents and canvases will appear here. There’s room for your next idea.</p></div>
          )}
        </section>

        {homeState.projects.length === 0 && recentItems.length === 0 && loadStatus === 'ready' && (
          <p className="home-start-hint"><Sparkles size={16} /> Start with a thought. Build on it at your own pace.</p>
        )}
      </div>
    </div>
  );
};

export default HomeView;
