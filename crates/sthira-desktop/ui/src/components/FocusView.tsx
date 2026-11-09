import { applyDocumentTitle } from '../api/naming';
import { TitleSuggestionsPanel, type TitleBasis } from './TitleSuggestionsPanel';
import { DocumentChatPanel } from './DocumentChatPanel';
import { useWorkflowSettings } from '../hooks/useWorkflowSettings';
import { openWorkflowSettings } from '../api/workflowSettings';
import { undoDocumentEdit, getDocumentConversation, type Conversation } from '../api/conversation';
import React, { useState, useEffect, useRef, useCallback } from 'react';
import {
  ArrowLeft,
  Sparkles,
  History,
  Check,
  Quote,
  Search,
  Loader2,
  Bold,
  Italic,
  CheckSquare,
  Code as CodeIcon,
  Table as TableIcon,
  Info,
  X,
  Globe,
  ShieldCheck,
  AlertTriangle,
  RefreshCw,
  Zap,
  ChevronDown,
  ExternalLink,
  BookOpen,
  Lightbulb,
  Link2,
  MessageSquare,
} from 'lucide-react';
import { ConnectionsPanel } from './ConnectionsPanel';
import { entityHref, getWorkspaceEntity } from '../api/connections';
import type { EntityRef, WorkspaceEntity } from '../types/sthira';
import { RefineArtifactModal } from './RefineArtifactModal';
import { ProjectResume } from './ProjectResume';
import { ProjectHistoryDrawer } from './ProjectHistoryDrawer';
import { AmbientProposalPill } from './AmbientProposalPill';
import { UndoToast } from './UndoToast';
import { EpistemicBadge } from './EpistemicBadge';
import { SlashCommandMenu, type SlashCommandItem } from './SlashCommandMenu';
import type { DocumentRunState } from '../types/sthira';
import { QuickObjectCreatorCard } from './objects/QuickObjectCreatorCard';
import { TemplateSelectorPopover } from './objects/TemplateSelectorPopover';
import { DocumentPropertyHeader } from './objects/DocumentPropertyHeader';
import { PageActionMenu } from './PageActionMenu';
import { DocumentEditor, type DocumentEditorHandle, type DocumentFormat } from './DocumentEditor';
import {
  getArtifactContext,
  getProjectHistory,
  saveArtifact,
  decideProposal,
  revertChangeSet,
  streamAdaptiveRun,
  getProviderCatalog,
  listObjectTypes,
  getTypeTemplates,
  instantiateObjectFromTemplate,
  getObjectInstance,
  createTypedObject,
  deriveArtifact,
} from '../api/ipc';
import type {
  Artifact,
  ArtifactContext,
  ProjectHistory as IProjectHistory,
  ObjectType,
  ObjectInstance,
  DerivationType,
} from '../types/sthira';

interface FocusViewProps {
  registerBeforeLeave?: (guard: (() => Promise<boolean>) | null) => void;
  onDocumentLoaded?: (artifact: Artifact, resolvedProjectId: string | null) => void;
  initialScrollTop?: number;
  onScrollPosition?: (top: number) => void;
  onOpenInNewTab?: () => void;
  sessionNamespace?: string;
  projectId?: string | null;
  artifactId?: string;
  onBack: () => void;
  backLabel?: string;
  onOpenStandaloneNotes?: () => void;
  onOpenCanvas: (canvasId?: string) => void;
  onOpenSearch?: () => void;
  onContentUpdate?: (content: string) => void;
  onSave?: (savedArtifact: Artifact) => void;
  onTitleUpdate?: (artifactId: string, newTitle: string) => void;
  /** Phase 17: used by the Page Action Menu to redirect context (e.g. after duplicating). */
  onOpenArtifact?: (artifactId: string, projectId?: string | null) => void;
  onOpenProject?: (projectId: string) => void;
  realtimeTitleUpdate?: { id: string; title: string } | null;
  /**
   * Phase 19/20: live streaming run (research pipeline stages or plan tokens) to render
   * as a progress banner and live stream inside the full-width center document canvas.
   */
  documentRun?: DocumentRunState | null;
  onDismissDocumentRun?: () => void;
}

export const FocusView: React.FC<FocusViewProps> = ({
  registerBeforeLeave, onDocumentLoaded, initialScrollTop = 0, onScrollPosition, onOpenInNewTab, sessionNamespace,
  projectId,
  artifactId,
  onBack,
  backLabel = 'Home',
  onOpenStandaloneNotes,
  onOpenSearch,
  onContentUpdate,
  onSave,
  onTitleUpdate,
  onOpenArtifact,
  onOpenProject,
  realtimeTitleUpdate,
  documentRun,
  onDismissDocumentRun,
}) => {
  const [context, setContext] = useState<ArtifactContext | null>(null);
  const workflows = useWorkflowSettings();
  const [breadcrumbProject, setBreadcrumbProject] = useState<{ id: string; name: string } | null>(null);
  // Prefer the selected project for shared notes; identity-only entry points can
  // resolve their location from the document's saved membership instead.
  const contextProjectId = !artifactId || context?.artifact.id === artifactId ? context?.project_id : null;
  const breadcrumbProjectId = projectId && projectId !== 'default_project' ? projectId
    : contextProjectId && contextProjectId !== 'default_project' ? contextProjectId : null;
  useEffect(() => {
    let active = true;
    setBreadcrumbProject(null);
    if (breadcrumbProjectId) {
      getWorkspaceEntity({ kind: 'project', id: breadcrumbProjectId }).then(({ entity }) => {
        if (active && entity.kind === 'project' && entity.id === breadcrumbProjectId) {
          setBreadcrumbProject({ id: entity.id, name: entity.title });
        }
      }).catch(() => { /* Project metadata must not block the document editor. */ });
    }
    return () => { active = false; };
  }, [breadcrumbProjectId]);
  const [showTitleSuggestions, setShowTitleSuggestions] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const [showResume, setShowResume] = useState(false);
  const [history, setHistory] = useState<IProjectHistory | null>(null);

  const [editorContent, setEditorContent] = useState('');
  const [title, setTitle] = useState('');
  const [isHistoryOpen, setIsHistoryOpen] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [saveSuccess, setSaveSuccess] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [isLoadingDocument, setIsLoadingDocument] = useState(true);
  const [actionError, setActionError] = useState<string | null>(null);
  const loadRequestRef = useRef(0);
  const saveRequestRef = useRef(0);
  const [hoveredQuote, setHoveredQuote] = useState<string | null>(null);
  const [undoToast, setUndoToast] = useState<{ message: string; changeSetId: string; artifactId?: string } | null>(null);
  const [pendingRefinement, setPendingRefinement] = useState<{
    targetArtifactId: string;
    refinedContent: string;
    refinedTitle: string;
  } | null>(null);
  const [isSavingConflict, setIsSavingConflict] = useState(false);
  const isSavingConflictRef = useRef(false);
  const conflictOpRef = useRef(0);
  const [conflictError, setConflictError] = useState<string | null>(null);

  // Auxiliary document context stays separate from the editable page.
  const [showMargin, setShowMargin] = useState(false);
  const [showConnections, setShowConnections] = useState(() => window.innerWidth >= 1280);
  const [connectionScope, setConnectionScope] = useState<'page' | 'project'>('page');
  const [connectionPreview, setConnectionPreview] = useState(false);
  const [requestedPreview, setRequestedPreview] = useState<{ entity: EntityRef; sequence: number } | null>(null);
  const connectionsButtonRef = useRef<HTMLButtonElement>(null);
  useEffect(() => { setConnectionScope('page'); setRequestedPreview(null); setConnectionPreview(false); setIsRefineModalOpen(false); }, [artifactId, projectId]);
  const [chatApplying, setChatApplying] = useState(false);
  const [chatLoadError, setChatLoadError] = useState<string | null>(null);
  const [chatReload, setChatReload] = useState(0);
  const [chatArtifactId, setChatArtifactId] = useState<string | null>(null);
  const [conversation, setConversation] = useState<Conversation | null>(null);
  const [showChat, setShowChat] = useState(() => { try { return localStorage.getItem('sthira-document-chat-open') === 'true'; } catch { return false; } });
  const chatButtonRef = useRef<HTMLButtonElement>(null);
  const setChatOpen = (open: boolean) => {
    setShowChat(open);
    try { localStorage.setItem('sthira-document-chat-open', String(open)); } catch { /* Preference is optional. */ }
    if (open) { setShowConnections(false); setShowMargin(false); }
    else chatButtonRef.current?.focus();
  };
  useEffect(() => {
    let active = true;
    setChatLoadError(null);
    if (chatArtifactId !== artifactId) { setConversation(null); setChatArtifactId(null); setChatApplying(false); }
    if (!artifactId) return;
    const mode = documentRun?.artifactId === artifactId ? documentRun.kind : 'document';
    getDocumentConversation(artifactId, mode, mode ? documentRun?.query : undefined).then(value => {
      if (active) { setConversation(value); setChatArtifactId(artifactId); if (value && showChat) { setShowConnections(false); setShowMargin(false); } }
    }).catch(() => { if (active) setChatLoadError('Couldn’t load this conversation. Try again.'); });
    return () => { active = false; };
  }, [artifactId, documentRun?.kind, documentRun?.artifactId, chatReload]);
  const openReference = (entity: EntityRef) => {
    setShowConnections(true); setShowMargin(false); setShowChat(false);
    setRequestedPreview({ entity, sequence: Date.now() });
  };
  const closeConnections = () => {
    setShowConnections(false); setConnectionPreview(false); setRequestedPreview(null);
    connectionsButtonRef.current?.focus();
  };
  const [marginTab, setMarginTab] = useState<'insights' | 'sources'>('insights');
  const [activeModelName, setActiveModelName] = useState('Active Model');
  const [activeModelId, setActiveModelId] = useState<string | undefined>(undefined);

  // Chained Synthesis: Refine & Derive State
  const [isRefineModalOpen, setIsRefineModalOpen] = useState(false);
  const [isDeriveMenuOpen, setIsDeriveMenuOpen] = useState(false);
  const [isDeriving, setIsDeriving] = useState(false);
  const deriveMenuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (deriveMenuRef.current && !deriveMenuRef.current.contains(e.target as Node)) {
        setIsDeriveMenuOpen(false);
      }
    };
    const handleEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setIsDeriveMenuOpen(false);
        deriveMenuRef.current?.querySelector('button')?.focus();
      }
    };
    if (isDeriveMenuOpen) {
      document.addEventListener('mousedown', handleClickOutside);
      document.addEventListener('keydown', handleEscape);
    }
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
      document.removeEventListener('keydown', handleEscape);
    };
  }, [isDeriveMenuOpen]);

  const handleDerive = async (derivationType: DerivationType) => {
    const currentArtId = context?.artifact?.id || artifactId;
    if (!currentArtId || isDeriving) return;
    const workflow = derivationType === 'plan_from_research' ? 'plan' : 'draft';
    if (workflows.loading || workflows.error || !workflows.settings[workflow].enabled) { openWorkflowSettings(); return; }
    setIsDeriveMenuOpen(false);
    setIsDeriving(true);
    setActionError(null);
    const sourceRequest = loadRequestRef.current;
    try {
      if (!(await handleSave())) return;
      const targetProjectId = (currentProjectIdRef.current && currentProjectIdRef.current !== 'default_project' ? currentProjectIdRef.current : (projectId && projectId !== 'default_project' ? projectId : null));
      const requestId = crypto.randomUUID();
      const newArtId = await deriveArtifact(
        currentArtId,
        derivationType,
        null,
        workflows.settings[workflow].model_id || activeModelId,
        targetProjectId,
        requestId
      );
      if (newArtId && onOpenArtifact && sourceRequest === loadRequestRef.current) {
        onOpenArtifact(newArtId, targetProjectId);
      }
    } catch (err: any) {
      console.error('Derivation failed:', err);
      if (sourceRequest === loadRequestRef.current) {
        setActionError('Couldn’t derive a new artifact. Check the selected provider and try again.');
      }
    } finally {
      if (sourceRequest === loadRequestRef.current) setIsDeriving(false);
    }
  };

  // Object Types & Inline Quick Creator State
  const [objectTypes, setObjectTypes] = useState<ObjectType[]>([]);
  const [inlineObjectCreator, setInlineObjectCreator] = useState<{
    isOpen: boolean;
    objectType: ObjectType;
    position: { top: number; left: number };
    caretIndex: number;
  } | null>(null);

  // Phase 16: Template selector popover + object-instance property header state
  const [templateSelector, setTemplateSelector] = useState<{
    isOpen: boolean;
    objectType: ObjectType;
    position: { top: number; left: number };
    caretIndex: number;
  } | null>(null);
  const [pendingTemplateId, setPendingTemplateId] = useState<string | null>(null);
  const [objectInstance, setObjectInstance] = useState<ObjectInstance | null>(null);

  // Slash Command Popover State
  const [slashMenu, setSlashMenu] = useState<{
    isOpen: boolean;
    filterQuery: string;
    position: { top: number; left: number };
    caretIndex: number;
  }>({
    isOpen: false,
    filterQuery: '',
    position: { top: 0, left: 0 },
    caretIndex: 0,
  });

  // Inline AI streaming state
  const [isAiStreaming, setIsAiStreaming] = useState(false);
  const [aiStreamingMessage, setAiStreamingMessage] = useState<string | null>(null);
  useEffect(() => {
    setIsDeriving(false); setIsDeriveMenuOpen(false); setIsAiStreaming(false); setAiStreamingMessage(null);
    setInlineObjectCreator(null); setTemplateSelector(null); setSlashMenu((prev) => ({ ...prev, isOpen: false }));
    setPendingRefinement(null);
    setConflictError(null);
    setIsSavingConflict(false);
    isSavingConflictRef.current = false;
    ++conflictOpRef.current;
  }, [artifactId, projectId]);

  const titleInputRef = useRef<HTMLInputElement>(null);
  const editorBodyRef = useRef<HTMLDivElement>(null);
  const documentEditorRef = useRef<DocumentEditorHandle>(null);
  const isInitialLoadedRef = useRef(false);
  const lastSavedContentRef = useRef('');
  const lastSavedTitleRef = useRef('');
  const autoSaveTimerRef = useRef<NodeJS.Timeout | null>(null);
  const currentTitleRef = useRef('');
  const currentContentRef = useRef('');
  const currentArtIdRef = useRef('');
  const currentProjectIdRef = useRef<string | null | undefined>(undefined);

  useEffect(() => {
    currentTitleRef.current = title;
  }, [title]);

  useEffect(() => {
    currentContentRef.current = editorContent;
  }, [editorContent]);

  useEffect(() => {
    currentArtIdRef.current = context?.artifact?.id || artifactId || '';
  }, [context, artifactId]);

  useEffect(() => {
    loadProjectData();
    loadActiveModel();
    loadObjectTypes();

    const handleRefresh = () => loadObjectTypes();
    const refreshModel = () => { void loadActiveModel(); };
    window.addEventListener('sthira:catalog-changed', refreshModel);
    window.addEventListener('sthira:refresh_objects', handleRefresh);
    return () => { window.removeEventListener('sthira:refresh_objects', handleRefresh); window.removeEventListener('sthira:catalog-changed', refreshModel); };
  }, [projectId, artifactId]);

  const loadObjectTypes = async () => {
    try {
      const types = await listObjectTypes();
      setObjectTypes(types);
    } catch (err) {
      console.error('Failed to load object types', err);
    }
  };

  // Phase 16: Resolve whether the open artifact is an `object_instance` so the
  // interactive property header can mount directly below the page title.
  useEffect(() => {
    const aid = context?.artifact?.id || artifactId || '';
    if (!aid) {
      setObjectInstance(null);
      return;
    }
    let cancelled = false;
    (async () => {
      try {
        const inst = await getObjectInstance(aid);
        if (!cancelled) setObjectInstance(inst || null);
      } catch (err) {
        console.warn('No object instance for artifact', aid, err);
        if (!cancelled) setObjectInstance(null);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [artifactId, context?.artifact?.id]);

  const objectTypeForInstance = objectInstance
    ? objectTypes.find((t) => t.id === objectInstance.object_type_id) || null
    : null;

  const loadActiveModel = async () => {
    try {
      const catalog = await getProviderCatalog();
      for (const prov of catalog) {
        if (!prov.is_enabled) continue;
        const def = prov.models.find((m) => m.is_default && m.is_enabled);
        if (def) {
          setActiveModelName(def.display_name || def.model_identifier);
          setActiveModelId(def.id);
          return;
        }
      }
      setActiveModelName('Choose a model');
      setActiveModelId(undefined);
    } catch {
      // Keep default
    }
  };

  useEffect(() => {
    const currentId = context?.artifact?.id || artifactId || '';
    if (
      realtimeTitleUpdate &&
      realtimeTitleUpdate.id === currentId &&
      realtimeTitleUpdate.title &&
      realtimeTitleUpdate.title !== currentTitleRef.current
    ) {
      setTitle(realtimeTitleUpdate.title);
      currentTitleRef.current = realtimeTitleUpdate.title;
      lastSavedTitleRef.current = realtimeTitleUpdate.title;
      setEditorContent((prev) => {
        const lines = prev.split('\n');
        if (lines.length > 0 && lines[0].startsWith('# ')) {
          lines[0] = `# ${realtimeTitleUpdate.title}`;
          const updated = lines.join('\n');
          currentContentRef.current = updated;
          lastSavedContentRef.current = updated;
          return updated;
        }
        return prev;
      });
    }
  }, [realtimeTitleUpdate, context, artifactId]);

  useEffect(() => {
    if (onContentUpdate) {
      onContentUpdate(editorContent);
    }
  }, [editorContent, onContentUpdate]);

  const installDocument = (ctx: ArtifactContext) => {
    const nextTitle = ctx.artifact.title || '';
    setContext(ctx);
    onDocumentLoaded?.(ctx.artifact, projectId || ctx.project_id || null);
    setEditorContent(ctx.artifact.content_md);
    setTitle(nextTitle);
    currentContentRef.current = ctx.artifact.content_md;
    currentTitleRef.current = nextTitle;
    currentArtIdRef.current = ctx.artifact.id;
    currentProjectIdRef.current = projectId || ctx.project_id || null;
    lastSavedContentRef.current = ctx.artifact.content_md;
    lastSavedTitleRef.current = nextTitle;
    isInitialLoadedRef.current = true;
    setSaveError(null);
    setIsSaving(false);
  };

  const loadProjectData = async () => {
    const request = ++loadRequestRef.current;
    setIsLoadingDocument(true);
    setLoadError(null);
    // Keep the old document's identity until its dirty text is safely persisted.
    // Sidebar navigation can change Focus props without unmounting the editor.
    if (isInitialLoadedRef.current && currentArtIdRef.current &&
        (currentContentRef.current !== lastSavedContentRef.current || currentTitleRef.current !== lastSavedTitleRef.current)) {
      const pendingContent = currentContentRef.current;
      const pendingTitle = currentTitleRef.current;
      try {
        await saveArtifact(currentArtIdRef.current, pendingContent, pendingTitle || null, currentProjectIdRef.current || null);
        if (request !== loadRequestRef.current) return;
        lastSavedContentRef.current = pendingContent;
        lastSavedTitleRef.current = pendingTitle;
      } catch {
        if (request === loadRequestRef.current) {
          setLoadError('Couldn’t save the open document before switching. Your edits are still held here; try again.');
          setIsLoadingDocument(false);
        }
        return;
      }
    }
    isInitialLoadedRef.current = false;
    let loadedArtId = artifactId;
    const hasProject = projectId && projectId !== 'default_project';
    // History is auxiliary: an unavailable timeline must not block editing.
    setHistory(null);
    if (hasProject) {
      getProjectHistory(projectId).then((value) => {
        if (request === loadRequestRef.current) setHistory(value);
      }).catch((error) => console.warn('Project history unavailable', error));

    }
    if (request !== loadRequestRef.current) return;
    setShowResume(false);
    try {
      if (loadedArtId) {
        const ctx = await getArtifactContext(loadedArtId);
        if (request !== loadRequestRef.current) return;
        installDocument(ctx);
      } else {
        installDocument({
          artifact: {
            id: '', artifact_type: 'note', authorship: 'user', title: 'Untitled Note',
            content_md: '# Untitled Note\n\n', content_hash: '', version: 1,
            is_unfinished_draft: false, created_at: Date.now(), updated_at: Date.now(),
          },
          epistemic_nodes: [], pending_proposals: [], project_id: projectId || null,
          is_unfinished_draft: false,
        });
      }
    } catch (error) {
      console.error('Failed to load artifact context', error);
      setLoadError('Couldn’t open this document. Your saved work is still in the vault.');
    } finally {
      if (request === loadRequestRef.current) setIsLoadingDocument(false);
    }
  };

  const handleSave = useCallback(async () => {
    if (!isInitialLoadedRef.current || isSavingConflictRef.current) return false;
    const artId = context?.artifact?.id || artifactId || '';
    if (artId && currentArtIdRef.current && currentArtIdRef.current !== artId) return false;
    const currentT = currentTitleRef.current.trim() || context?.artifact?.title || null;
    const currentC = currentContentRef.current;
    const saveRequest = ++saveRequestRef.current;
    const documentRequest = loadRequestRef.current;
    const isCurrentSave = () => saveRequest === saveRequestRef.current && documentRequest === loadRequestRef.current && (!artId || currentArtIdRef.current === artId);

    setIsSaving(true);
    setSaveSuccess(false);
    setSaveError(null);
    try {
      const targetProjectId = currentProjectIdRef.current && currentProjectIdRef.current !== 'default_project' ? currentProjectIdRef.current : null;
      const updated = await saveArtifact(artId, currentC, currentT, targetProjectId);
      if (!isCurrentSave()) return false;
      lastSavedContentRef.current = currentC;
      lastSavedTitleRef.current = updated.title || currentT || '';
      if (currentTitleRef.current === currentT) {
        setTitle(updated.title || currentT || '');
      }
      try {
        const ctx = await getArtifactContext(updated.id);
        if (!isCurrentSave()) return false;
        setContext(ctx);
      } catch {
        setContext((prev) =>
          prev
            ? { ...prev, artifact: updated }
            : {
                artifact: updated,
                epistemic_nodes: [],
                pending_proposals: [],
                project_id: projectId || null,
                is_unfinished_draft: false,
              }
        );
      }
      setSaveSuccess(true);
      if (onSave) onSave(updated);
      setTimeout(() => setSaveSuccess(false), 2500);
      return true;
    } catch (err) {
      if (!isCurrentSave()) return false;
      console.error('Failed to save artifact', err);
      setSaveError('Couldn’t save your changes. Try saving again.');
      return false;
    } finally {
      if (isCurrentSave()) setIsSaving(false);
    }
  }, [context, artifactId, projectId, onSave]);

  useEffect(() => {
    registerBeforeLeave?.(async () => {
      if (chatApplying || isAiStreaming || isDeriving || isSavingConflict) { setActionError('Wait for the current page change to finish before switching.'); return false; }
      if (autoSaveTimerRef.current) clearTimeout(autoSaveTimerRef.current);
      if (!isInitialLoadedRef.current) return true;
      if (currentContentRef.current === lastSavedContentRef.current && currentTitleRef.current === lastSavedTitleRef.current) return true;
      return handleSave();
    });
    return () => registerBeforeLeave?.(null);
  }, [registerBeforeLeave, handleSave, chatApplying, isAiStreaming, isDeriving, isSavingConflict]);
  useEffect(() => {
    if (isLoadingDocument) return;
    const frame = requestAnimationFrame(() => { if (scrollRef.current) scrollRef.current.scrollTop = initialScrollTop; });
    return () => cancelAnimationFrame(frame);
  }, [isLoadingDocument, artifactId]);
  const applySuggestedTitle = async (suggested: string, basis: TitleBasis) => {
    if (basis.title !== currentTitleRef.current || basis.content !== currentContentRef.current) { setActionError('The page changed. Request new title suggestions.'); return false; }
    if (!basis.contentHash) { setActionError('Request new title suggestions before applying.'); return false; }
    if (autoSaveTimerRef.current) clearTimeout(autoSaveTimerRef.current);
    const id = currentArtIdRef.current;
    setChatApplying(true);
    try {
      const updated = await applyDocumentTitle(id,suggested,basis.contentHash,basis.title || null);
      if (currentArtIdRef.current !== id) return false;
      installDocument({...context!,artifact:updated});
      onTitleUpdate?.(id,updated.title || suggested); onSave?.(updated);
      return true;
    } catch (error) { setActionError(`Could not apply the title. ${String(error)}`); return false; }
    finally { setChatApplying(false); }
  };

  // Debounced Auto-Save
  useEffect(() => {
    if (!isInitialLoadedRef.current) return;
    if (
      editorContent === lastSavedContentRef.current &&
      title === lastSavedTitleRef.current
    ) {
      return;
    }

    if (autoSaveTimerRef.current) {
      clearTimeout(autoSaveTimerRef.current);
    }

    autoSaveTimerRef.current = setTimeout(() => {
      void handleSave();
    }, 600);

    return () => {
      if (autoSaveTimerRef.current) {
        clearTimeout(autoSaveTimerRef.current);
      }
    };
  }, [editorContent, title, handleSave]);

  // Phase 19/20: when a run finishes writing into this page, reload the body so the
  // grounded document replaces the draft placeholder.
  useEffect(() => {
    if (!documentRun || documentRun.refreshToken === 0) return;
    if (artifactId && documentRun.artifactId !== artifactId) return;
    loadProjectData();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [documentRun?.refreshToken, documentRun?.artifactId]);

  // Flush save on unmount only if dirty
  useEffect(() => {
    return () => {
      if (autoSaveTimerRef.current) {
        clearTimeout(autoSaveTimerRef.current);
      }
      if (
        isInitialLoadedRef.current &&
        (currentContentRef.current !== lastSavedContentRef.current ||
          currentTitleRef.current !== lastSavedTitleRef.current)
      ) {
        const artId = currentArtIdRef.current;
        if (artId) {
          const targetProjectId =
            currentProjectIdRef.current && currentProjectIdRef.current !== 'default_project'
              ? currentProjectIdRef.current
              : null;
          saveArtifact(
            artId,
            currentContentRef.current,
            currentTitleRef.current || null,
            targetProjectId
          )?.catch?.((e: any) => console.warn('Unmount auto-save failed', e));
        }
      }
    };
  }, []);

  const handleTitleChange = (newT: string) => {
    setTitle(newT);
    currentTitleRef.current = newT;
    if (context) {
      setContext({
        ...context,
        artifact: { ...context.artifact, title: newT },
      });
    }
    setEditorContent((prev) => {
      const lines = prev.split('\n');
      if (lines.length > 0 && lines[0].startsWith('# ')) {
        lines[0] = `# ${newT}`;
        const updated = lines.join('\n');
        currentContentRef.current = updated;
        return updated;
      }
      return prev;
    });
    const currentId = context?.artifact?.id || artifactId || '';
    if (currentId && onTitleUpdate) {
      onTitleUpdate(currentId, newT);
    }
  };

  const handleEditorChange = (newC: string) => {
    setEditorContent(newC);
    currentContentRef.current = newC;

    // Auto-sync heading from line 1
    const lines = newC.split('\n');
    if (lines.length > 0 && lines[0].startsWith('# ')) {
      const extracted = lines[0].replace(/^#+\s*/, '').trim();
      if (extracted && extracted !== currentTitleRef.current) {
        setTitle(extracted);
        currentTitleRef.current = extracted;
        if (context) {
          setContext((prev) =>
            prev ? { ...prev, artifact: { ...prev.artifact, title: extracted } } : null
          );
        }
        const currentId = context?.artifact?.id || artifactId || '';
        if (currentId && onTitleUpdate) {
          onTitleUpdate(currentId, extracted);
        }
      }
    }

  };

  // Phase 16: Begin typed-object creation. When more than one template exists for the
  // type we surface a template selector popover first; otherwise the default template is
  // auto-selected and the Quick Object Creator opens immediately.
  const beginTypedObjectCreation = async (
    objectType: ObjectType,
    position: { top: number; left: number },
    caretIndex: number,
  ) => {
    let templates: Awaited<ReturnType<typeof getTypeTemplates>> = [];
    try {
      templates = (await getTypeTemplates(objectType.id)) || [];
    } catch (err) {
      console.warn('Falling back to default template', err);
      templates = [];
    }

    if (templates.length > 1) {
      setTemplateSelector({ isOpen: true, objectType, position, caretIndex });
      return;
    }

    setPendingTemplateId(templates.length === 1 ? templates[0].id : null);
    setInlineObjectCreator({ isOpen: true, objectType, position, caretIndex });
  };

  // Slash Command Selection Handler
  const handleSelectSlashCommand = async (item: SlashCommandItem) => {
    setSlashMenu((prev) => ({ ...prev, isOpen: false }));

    if (item.isObjectAction && item.objectType) {
      await beginTypedObjectCreation(
        item.objectType,
        slashMenu.position,
        slashMenu.caretIndex,
      );
      return;
    }

    if (item.isAiAction) {
      await handleRunInlineAi(item.aiMode || 'plan');
      return;
    }

    documentEditorRef.current?.insertMarkdown(item.template, true);
    if (item.id === 'decision') void handleSave();
  };

  // Inline AI Generator
  const handleRunInlineAi = async (mode: 'plan' | 'research' | 'expand') => {
    const workflow = mode === 'expand' ? 'draft' : mode;
    if (workflows.loading || workflows.error || !workflows.settings[workflow].enabled) { openWorkflowSettings(); return; }
    const insertion = documentEditorRef.current?.startInsertion();
    const sourceId = currentArtIdRef.current;
    const sourceRequest = loadRequestRef.current;
    const topic = title.trim() || 'Software Architecture & Planning';
    const actionLabel = mode === 'plan' ? 'Generating Plan' : mode === 'research' ? 'Researching Findings' : 'Expanding Section';

    setIsAiStreaming(true);
    setAiStreamingMessage(`${actionLabel} with ${activeModelName}...`);

    let accumulatedTokens = '';

    try {
      await streamAdaptiveRun(topic, mode === 'expand' ? 'act' : mode, workflows.settings[workflow].model_id || activeModelId, (chunk) => {
        if (chunk.type === 'text_token') {
          accumulatedTokens += chunk.payload;
          if (currentArtIdRef.current === sourceId && sourceRequest === loadRequestRef.current) insertion?.replace(accumulatedTokens);
        }
      });
      if (sourceRequest === loadRequestRef.current) void handleSave();
    } catch (err) {
      console.error('Inline AI streaming failed', err);
      if (sourceRequest === loadRequestRef.current) setActionError('The model stream stopped. Your current draft is still available.');
    } finally {
      if (sourceRequest === loadRequestRef.current) { setIsAiStreaming(false); setAiStreamingMessage(null); }
    }
  };

  // Quick Object Creator Submission Handler
  const handleCreateAndInsertObject = async (objTitle: string, properties: Record<string, any>) => {
    if (!inlineObjectCreator) return;
    const { objectType } = inlineObjectCreator;
    setInlineObjectCreator(null);

    try {
      // 1. Create the backing artifact hydrated with the resolved template Markdown body
      //    and merged default property values (cmd_instantiate_object_from_template).
      const createdArtifactId = await instantiateObjectFromTemplate(
        objectType.id,
        pendingTemplateId,
        objTitle,
        projectId,
      );
      setPendingTemplateId(null);

      // 2. Insert embedded object directive into document markdown
      const directive = `\n\`\`\`sthira-object\n${JSON.stringify(
        {
          artifact_id: createdArtifactId,
          object_type_id: objectType.id,
          title: objTitle,
          properties,
        },
        null,
        2
      )}\n\`\`\`\n`;

      documentEditorRef.current?.insertMarkdown(directive, true);
      void handleSave();

      // 3. Refresh Left Sidebar object counts automatically
      window.dispatchEvent(new CustomEvent('sthira:refresh_objects'));
    } catch (err) {
      console.error('Failed to create typed object instance', err);
    }
  };

  const handleApplyFormat = (format: DocumentFormat) => documentEditorRef.current?.format(format);

  const handleBack = async () => {
    if (
      currentContentRef.current !== lastSavedContentRef.current ||
      currentTitleRef.current !== lastSavedTitleRef.current
    ) {
      if (!(await handleSave())) return;
    }
    onBack();
  };

  const navigateToArtifact = async (aid: string) => {
    if (currentContentRef.current !== lastSavedContentRef.current || currentTitleRef.current !== lastSavedTitleRef.current) {
      if (!(await handleSave())) return;
    }
    if (onOpenArtifact) {
      onOpenArtifact(aid, projectId || null);
    } else {
      try { installDocument(await getArtifactContext(aid)); }
      catch { setSaveError('Couldn’t open the selected document. Your current document is still open.'); }
    }
    setIsHistoryOpen(false);
  };

  const handleAcceptProposal = async (propId: string) => {
    try {
      const csId = await decideProposal(propId, 'accept');
      if (context) {
        setContext({
          ...context,
          pending_proposals: context.pending_proposals.filter((p) => p.id !== propId),
        });
      }
      setUndoToast({
        message: 'Link added to knowledge graph',
        changeSetId: csId,
      });
    } catch (err) {
      console.error('Failed to accept proposal', err);
    }
  };

  const handleDismissProposal = async (propId: string) => {
    try {
      await decideProposal(propId, 'dismiss');
      if (context) {
        setContext({
          ...context,
          pending_proposals: context.pending_proposals.filter((p) => p.id !== propId),
        });
      }
    } catch (err) {
      console.error('Failed to dismiss proposal', err);
    }
  };

  const handleUndo = async () => {
    if (!undoToast) return;
    const request = loadRequestRef.current;
    if (undoToast.artifactId && currentArtIdRef.current !== undoToast.artifactId) {
      setActionError('Open the original document to undo this edit.'); return;
    }
    try {
      if (undoToast.artifactId) {
        if (isDirty) { setActionError('Save or resolve your newer changes before undoing this edit.'); return; }
        setChatApplying(true);
        const restored = await undoDocumentEdit(undoToast.artifactId, undoToast.changeSetId);
        if (request !== loadRequestRef.current) return;
        currentContentRef.current = restored.content_md;
        lastSavedContentRef.current = restored.content_md;
        setEditorContent(restored.content_md);
        setContext(previous => previous ? { ...previous, artifact: restored } : previous);
        onSave?.(restored);
        const restoredConversation = await getDocumentConversation(undoToast.artifactId);
        if (request !== loadRequestRef.current) return;
        setConversation(restoredConversation);
      } else await revertChangeSet(undoToast.changeSetId);
      setUndoToast(null);
      loadProjectData();
    } catch (err) {
      console.error('Failed to undo change set', err);
      if (request === loadRequestRef.current) setActionError(String(err));
    } finally { if (request === loadRequestRef.current) setChatApplying(false); }
  };

  const sourceLinks = Array.from(editorContent.matchAll(/\[([^\]]+)\]\((https?:\/\/[^\)]+)\)/g));
  const sourceCount = new Set([context?.artifact?.source_url, ...sourceLinks.map((match) => match[2])].filter(Boolean)).size;
  const projectLabel = breadcrumbProjectId
    ? (breadcrumbProject?.id === breadcrumbProjectId && breadcrumbProject.name) || 'Project workspace'
    : 'Pages';
  const isDirty = editorContent !== lastSavedContentRef.current || title !== lastSavedTitleRef.current;
  const connectionArtifactId = artifactId || context?.artifact?.id;
  const connectionProjectId = projectId && projectId !== 'default_project' ? projectId : null;
  const connectionEntity: EntityRef | null = connectionScope === 'project' && connectionProjectId
    ? { kind: 'project', id: connectionProjectId }
    : connectionArtifactId ? { kind: 'artifact', id: connectionArtifactId }
    : connectionProjectId ? { kind: 'project', id: connectionProjectId } : null;
  const insertReference = (target: WorkspaceEntity) => {
    if (!connectionArtifactId || currentArtIdRef.current !== connectionArtifactId) return;
    documentEditorRef.current?.insertLink(target.title, entityHref(target));
  };

  const resolveConflictChoice = async (action: 'keep' | 'apply') => {
    if (!pendingRefinement || isSavingConflictRef.current) return;
    const targetArtId = pendingRefinement.targetArtifactId;
    const currentLoadReq = loadRequestRef.current;
    const opId = ++conflictOpRef.current;

    let bodyToSave = '';
    let titleToSave: string | null = null;

    if (action === 'keep') {
      bodyToSave = currentContentRef.current;
      titleToSave = currentTitleRef.current.trim() || title?.trim() || null;
    } else {
      bodyToSave = pendingRefinement.refinedContent;
      titleToSave = pendingRefinement.refinedTitle || currentTitleRef.current.trim() || title?.trim() || null;
      const lines = bodyToSave.split('\n');
      if (lines.length > 0 && lines[0].startsWith('# ')) {
        const extracted = lines[0].replace(/^#+\s*/, '').trim();
        if (extracted) {
          titleToSave = extracted;
        }
      }
    }

    const targetProjectId =
      currentProjectIdRef.current && currentProjectIdRef.current !== 'default_project'
        ? currentProjectIdRef.current
        : (projectId && projectId !== 'default_project' ? projectId : null);

    if (autoSaveTimerRef.current) {
      clearTimeout(autoSaveTimerRef.current);
      autoSaveTimerRef.current = null;
    }
    isSavingConflictRef.current = true;
    setIsSavingConflict(true);
    setConflictError(null);

    const isCurrent = () =>
      currentArtIdRef.current === targetArtId &&
      loadRequestRef.current === currentLoadReq &&
      conflictOpRef.current === opId;

    try {
      const updated = await saveArtifact(targetArtId, bodyToSave, titleToSave, targetProjectId);
      if (!isCurrent()) return;

      if (action === 'apply') {
        if (titleToSave) {
          setTitle(titleToSave);
          currentTitleRef.current = titleToSave;
        }
        handleEditorChange(bodyToSave);
        onContentUpdate?.(bodyToSave);
      }

      lastSavedContentRef.current = bodyToSave;
      lastSavedTitleRef.current = updated.title || titleToSave || '';

      try {
        const ctx = await getArtifactContext(updated.id);
        if (isCurrent()) {
          setContext(ctx);
        }
      } catch {
        if (isCurrent()) {
          setContext((prev) => (prev ? { ...prev, artifact: updated } : prev));
        }
      }

      if (!isCurrent()) return;

      setPendingRefinement(null);
      setConflictError(null);
      setSaveSuccess(true);
      if (onSave) onSave(updated);
      setTimeout(() => {
        if (isCurrent()) setSaveSuccess(false);
      }, 2500);
    } catch (err: any) {
      if (!isCurrent()) return;
      console.error(`Failed to ${action} conflict choice:`, err);
      setConflictError(
        action === 'keep'
          ? 'Couldn’t save your draft. Try keeping again.'
          : 'Couldn’t apply refinement. Try applying again.'
      );
    } finally {
      if (isCurrent()) {
        isSavingConflictRef.current = false;
        setIsSavingConflict(false);
      }
    }
  };

  const handleApplyRefinement = () => resolveConflictChoice('apply');
  const handleKeepDraft = () => resolveConflictChoice('keep');

  return (
    <div className="focus-workspace flex-1 h-full overflow-hidden flex flex-col" data-chat-open={showChat && !!conversation && chatArtifactId === artifactId} data-margin-open={showMargin} data-connections-open={showConnections && !!connectionEntity} data-connection-preview={connectionPreview}>
      <header className="focus-header">
        <nav className="focus-breadcrumbs" aria-label="Document location">
          <button type="button" onClick={handleBack} className="focus-home-button" title={`Return to ${backLabel}`} aria-label={backLabel}><ArrowLeft size={16} /><span>{backLabel}</span></button>
          <span aria-hidden="true">/</span>
          <button type="button" className="focus-project-breadcrumb focus-breadcrumb-link" title={breadcrumbProjectId ? `Open project: ${projectLabel}` : 'Browse standalone pages'}
            disabled={isLoadingDocument || !!loadError || (breadcrumbProjectId ? !onOpenProject : !onOpenStandaloneNotes)}
            onClick={async () => {
              if (isDirty && !(await handleSave())) return;
              if (breadcrumbProjectId) onOpenProject?.(breadcrumbProjectId); else onOpenStandaloneNotes?.();
            }}>{projectLabel}</button>
          <span aria-hidden="true">/</span>
          <button type="button" className="focus-current-breadcrumb focus-breadcrumb-link" title="Edit document title" aria-label={`Edit document title: ${title || 'Untitled Note'}`}
            disabled={isLoadingDocument || !!loadError} onClick={() => { titleInputRef.current?.focus(); titleInputRef.current?.select(); }}>{title || 'Untitled Note'}</button>
        </nav>
        <div className="focus-header-actions">
          {onOpenSearch && <button type="button" onClick={onOpenSearch} className="focus-search-action focus-icon-button" aria-label="Search workspace" title="Search workspace"><Search size={16} /></button>}
          {projectId && projectId !== 'default_project' && <button type="button" className="focus-header-action" aria-label="Resume" aria-expanded={showResume} disabled={isLoadingDocument||!!loadError} onClick={async()=>{const target=projectId;if(isDirty&&!(await handleSave()))return;if(target===currentProjectIdRef.current)setShowResume(value=>!value);}}><Sparkles size={16}/><span>Resume</span></button>}
          <button type="button" className="focus-header-action" onClick={() => setIsHistoryOpen(true)} disabled={!history} aria-label="History" title={history ? 'Project history' : 'History is available for project documents'}><History size={16} /><span>History</span></button>
          {artifactId && <button type="button" disabled={!conversation && !chatLoadError} ref={chatButtonRef} className="focus-header-action" aria-label="Conversation" aria-expanded={showChat} aria-controls="document-chat" onClick={() => { if (chatLoadError) setChatReload(value => value + 1); else setChatOpen(!showChat); }}><MessageSquare size={16} /><span>Chat</span></button>}
          <button type="button" ref={connectionsButtonRef} className="focus-header-action" onClick={() => { if (showConnections) closeConnections(); else { setShowConnections(true); setShowMargin(false); setShowChat(false); } }} disabled={!connectionEntity} aria-label="Connections" aria-expanded={showConnections && !!connectionEntity} aria-controls="workspace-connections" title={connectionEntity ? 'Show connected pages, projects and objects' : 'Save this page to connect it'}><Link2 size={16} /><span>Connections</span></button>
          <button type="button" data-testid="btn-save-artifact" className={`focus-save-status ${saveError ? 'focus-save-error' : ''}`} onClick={handleSave} disabled={isLoadingDocument || !!loadError || isSaving || chatApplying} title={saveError ? 'Retry saving your changes' : 'Changes save automatically. Click to save now.'}>
            {isSaving ? <Loader2 size={15} className="animate-spin" /> : saveError ? <AlertTriangle size={15} /> : <Check size={15} />}
            <span aria-live="polite">{saveError ? 'Save failed' : isSaving ? 'Saving...' : saveSuccess ? 'Saved' : isDirty ? 'Unsaved changes' : 'Auto-saved'}</span>
          </button>
        </div>
      </header>

      {/* Main Full-Height Workspace Body */}
      <div className="focus-workspace-body">
        {/* Generous Full-Height Document Canvas */}
        <div className="focus-document-scroll" data-testid="document-scroll-region" ref={scrollRef} onScroll={event => onScrollPosition?.(event.currentTarget.scrollTop)}>
          <div
            data-testid="editor-canvas"
            className="focus-document-sheet flex-1 min-h-full relative" inert={chatApplying}
          >
            {chatLoadError && <div className="focus-error focus-save-notice" role="alert"><span>{chatLoadError}</span><button type="button" onClick={() => setChatReload(value => value + 1)}>Retry conversation</button></div>}
            {actionError && <div className="focus-error focus-save-notice" role="alert"><span>{actionError}</span><button type="button" onClick={() => setActionError(null)}>Dismiss</button></div>}
            {saveError && <div className="focus-error focus-save-notice" role="alert"><span>{saveError}</span><button type="button" onClick={handleSave}>Retry save</button></div>}
            {pendingRefinement && (
              <div
                data-testid="banner-refinement-conflict"
                className="focus-save-notice p-3 bg-amber-50 dark:bg-amber-950/40 border border-amber-200 dark:border-amber-900 rounded-lg text-xs text-amber-800 dark:text-amber-200 mb-3"
                role="status"
              >
                <div className="flex items-center justify-between gap-3">
                  <div className="flex items-center gap-2">
                    <Sparkles className="w-4 h-4 text-amber-600 dark:text-amber-400 shrink-0" />
                    <span>
                      A background refinement completed with committed changes, but your newer edits were preserved.
                    </span>
                  </div>
                  <div className="flex items-center gap-2 shrink-0">
                    <button
                      type="button"
                      data-testid="btn-apply-pending-refinement"
                      disabled={isSavingConflict}
                      onClick={handleApplyRefinement}
                      className="px-2.5 py-1 text-xs font-semibold rounded bg-amber-600 hover:bg-amber-700 text-white transition-colors cursor-pointer disabled:opacity-50"
                    >
                      Apply Refinement
                    </button>
                    <button
                      type="button"
                      data-testid="btn-dismiss-pending-refinement"
                      disabled={isSavingConflict}
                      onClick={handleKeepDraft}
                      className="px-2.5 py-1 text-xs font-medium rounded hover:bg-amber-100 dark:hover:bg-amber-900/50 text-amber-800 dark:text-amber-300 transition-colors cursor-pointer disabled:opacity-50"
                    >
                      Keep My Draft
                    </button>
                  </div>
                </div>
                {conflictError && (
                  <div
                    data-testid="refinement-conflict-error"
                    className="text-red-700 dark:text-red-400 font-medium pl-6 pt-1"
                    role="alert"
                  >
                    {conflictError}
                  </div>
                )}
              </div>
            )}
            {isLoadingDocument ? <div className="focus-document-state" role="status"><Loader2 size={20} className="animate-spin" /> Opening document…</div> : loadError ? <div className="focus-document-state" role="alert"><AlertTriangle size={20} /><p>{loadError}</p><button type="button" className="sthira-button-secondary" onClick={loadProjectData}>Try again</button></div> : <>
            {/* Phase 19/20: Live Run Banner — the pipeline's real stages (research) or
                the LLM token stream (plan) render here, inside the full-width center
                document canvas. */}
            {documentRun && (
              <div
                data-testid="document-run-banner"
                data-run-kind={documentRun.kind}
                data-run-status={documentRun.status}
                data-run-local={documentRun.mode === 'local_vault_only' || documentRun.message.includes('local context') || documentRun.message.includes('Web search returned no results')}
                className="focus-run-banner"
              >
                {documentRun.status === 'running' ? (
                  <Loader2 className="w-4 h-4 mt-0.5 shrink-0 animate-spin" />
                ) : documentRun.status === 'done' &&
                  documentRun.mode !== 'local_vault_only' &&
                  !documentRun.message.includes('Web search returned no results') &&
                  !documentRun.message.includes('local context') ? (
                  <ShieldCheck className="w-4 h-4 mt-0.5 shrink-0" />
                ) : (
                  <AlertTriangle className="w-4 h-4 mt-0.5 shrink-0" />
                )}

                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2">
                    <span className="font-bold text-[11px] uppercase tracking-wider flex items-center gap-1">
                      {documentRun.kind === 'plan' ? (
                        <Sparkles className="w-3 h-3" />
                      ) : (
                        <Globe className="w-3 h-3" />
                      )}
                      {documentRun.kind === 'plan'
                        ? 'Planning'
                        : documentRun.status === 'error' ? 'Research failed'
                        : documentRun.mode === 'local_vault_only' ||
                          documentRun.message.includes('Web search returned no results') ||
                          documentRun.message.includes('local context')
                          ? 'Local Vault Synthesis'
                          : 'Web Search'}
                    </span>
                    {documentRun.status === 'running' && (
                      <span className="text-[10px] opacity-70 animate-pulse">live</span>
                    )}
                  </div>
                  <p className={`mt-0.5 ${documentRun.status === 'error' ? 'whitespace-normal break-words' : 'truncate'}`} title={documentRun.message} data-testid="document-run-message">
                    {documentRun.message}
                  </p>
                  <p className="mt-0.5 text-[10px] opacity-70 truncate">
                    {documentRun.kind === 'plan' ? 'Goal' : 'Inquiry'}: {documentRun.query}
                  </p>
                </div>

                {documentRun.status !== 'running' && onDismissDocumentRun && (
                  <button
                    type="button"
                    data-testid="btn-dismiss-document-run"
                    onClick={onDismissDocumentRun}
                    className="p-1 rounded-lg hover:bg-black/5 dark:hover:bg-white/10 transition-colors cursor-pointer shrink-0"
                    title="Dismiss"
                  >
                    <X className="w-3.5 h-3.5" />
                  </button>
                )}
              </div>
            )}

            {/* Phase 20: live token stream for `plan` runs, rendered directly in the
                center document canvas while the model is still generating. */}
            {documentRun?.status === 'running' && documentRun.streamedMarkdown && (
              <div
                data-testid="document-live-stream"
                className="focus-live-stream"
              >
                <div className="text-[10px] font-bold uppercase tracking-wider text-sthira-text-subtle mb-2">
                  Live stream
                </div>
                <pre className="whitespace-pre-wrap break-words font-sans text-[13px] leading-relaxed text-sthira-text max-h-[50vh] overflow-y-auto">
                  {documentRun.streamedMarkdown}
                </pre>
              </div>
            )}

            {showResume && projectId && projectId !== 'default_project' && <ProjectResume key={`${projectId}:${artifactId}`} projectId={projectId} onSelectArtifact={navigateToArtifact} />}

            {/* Lineage Header Chip */}
            {context?.artifact?.derived_from_artifact_id && (
              <div className="mb-2">
                <button
                  type="button"
                  data-testid="badge-lineage-derived-from"
                  onClick={() => {
                    if (context.artifact.derived_from_artifact_id && onOpenArtifact) {
                      onOpenArtifact(context.artifact.derived_from_artifact_id, projectId || null);
                    }
                  }}
                  className="focus-lineage inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-medium bg-sthira-surface-muted text-sthira-text-muted border border-sthira-border hover:bg-sthira-surface-muted hover:text-sthira-text transition-colors cursor-pointer group"
                  title={`Navigate to parent artifact: ${context.derived_from_title || context.artifact.derived_from_artifact_id}`}
                >
                  <span className="text-xs">🔗</span>
                  <span className="font-semibold text-sthira-text">Derived from:</span>
                  <span className="underline decoration-stone-300 dark:decoration-stone-600 underline-offset-2 group-hover:decoration-stone-500">
                    {context.derived_from_title || 'Parent Note'}
                  </span>
                  {context.artifact.derivation_type && (
                    <span className="text-[10px] px-1.5 py-0.2 rounded bg-stone-200/80 dark:bg-stone-800 text-stone-600 dark:text-stone-300 font-mono">
                      {context.artifact.derivation_type}
                    </span>
                  )}
                </button>
              </div>
            )}

            {/* Document Title Header */}
            <div className="focus-title-header">
              <input
                type="text"
                ref={titleInputRef}
                data-testid="input-artifact-title"
                value={title}
                disabled={chatApplying || isLoadingDocument}
                onChange={(e) => handleTitleChange(e.target.value)}
                onBlur={handleSave}
                placeholder="Document title..."
                aria-label="Document title"
                title={title}
                className="text-3xl sm:text-4xl font-editorial font-bold tracking-tight text-sthira-text bg-transparent border-none outline-none w-full placeholder-sthira-text-subtle"
              />
            </div>

            <div className="focus-title-tools"><button type="button" className="focus-brief-source" disabled={!context?.artifact.id || isLoadingDocument || chatApplying || !!documentRun && documentRun.status === 'running'} aria-expanded={showTitleSuggestions} onClick={() => setShowTitleSuggestions(value => !value)}><Sparkles size={14}/> Suggest a title</button>
              {onOpenInNewTab && <button type="button" className="focus-brief-source" onClick={onOpenInNewTab}>Open another document</button>}
            </div>
            {showTitleSuggestions && context?.artifact.id && <TitleSuggestionsPanel key={context.artifact.id} artifactId={context.artifact.id} current={{content:editorContent,title}} snapshot={() => ({content:currentContentRef.current,title:currentTitleRef.current})} beforeRequest={async () => {
              if (currentContentRef.current !== lastSavedContentRef.current || currentTitleRef.current !== lastSavedTitleRef.current) return handleSave();
              return true;
            }} onApply={applySuggestedTitle} onClose={() => setShowTitleSuggestions(false)}/>}

            {/* Phase 16: Notion-style interactive property tray for typed objects */}
            {objectInstance && objectTypeForInstance && (
              <DocumentPropertyHeader
                artifactId={context?.artifact?.id || artifactId || objectInstance.artifact_id}
                objectType={objectTypeForInstance}
                instance={objectInstance}
                onInstanceUpdated={(updated) => setObjectInstance(updated)}
              />
            )}

            {/* Notion-Style Formatting Toolbar & Rich Preview Toggle */}
            <div className="focus-format-toolbar">
              {/* Left: Quick Formatting Actions */}
              <div className="focus-format-actions" role="group" aria-label="Document formatting">
                <button
                  type="button"
                  onClick={() => handleApplyFormat('bold')}
                  className="p-1.5 rounded-lg hover:bg-sthira-surface-muted hover:text-sthira-text transition-colors cursor-pointer"
                  title="Bold (Cmd+B)"
                >
                  <Bold className="w-3.5 h-3.5" />
                </button>
                <button
                  type="button"
                  onClick={() => handleApplyFormat('italic')}
                  className="p-1.5 rounded-lg hover:bg-sthira-surface-muted hover:text-sthira-text transition-colors cursor-pointer"
                  title="Italic (Cmd+I)"
                >
                  <Italic className="w-3.5 h-3.5" />
                </button>
                <div className="w-px h-4 bg-sthira-border-strong mx-1" />
                <button
                  type="button"
                  onClick={() => handleApplyFormat('h1')}
                  className="px-2 py-1 rounded-lg hover:bg-sthira-surface-muted hover:text-sthira-text font-semibold text-xs transition-colors cursor-pointer"
                  title="Heading 1"
                >
                  H1
                </button>
                <button
                  type="button"
                  onClick={() => handleApplyFormat('h2')}
                  className="px-2 py-1 rounded-lg hover:bg-sthira-surface-muted hover:text-sthira-text font-semibold text-xs transition-colors cursor-pointer"
                  title="Heading 2"
                >
                  H2
                </button>
                <button
                  type="button"
                  onClick={() => handleApplyFormat('todo')}
                  className="p-1.5 rounded-lg hover:bg-sthira-surface-muted hover:text-sthira-text transition-colors cursor-pointer"
                  title="Checklist Item"
                >
                  <CheckSquare className="w-3.5 h-3.5" />
                </button>
                <button
                  type="button"
                  onClick={() => handleApplyFormat('code')}
                  className="p-1.5 rounded-lg hover:bg-sthira-surface-muted hover:text-sthira-text transition-colors cursor-pointer"
                  title="Code Block"
                >
                  <CodeIcon className="w-3.5 h-3.5" />
                </button>
                <button
                  type="button"
                  onClick={() => handleApplyFormat('table')}
                  className="p-1.5 rounded-lg hover:bg-sthira-surface-muted hover:text-sthira-text transition-colors cursor-pointer"
                  title="Markdown Table"
                >
                  <TableIcon className="w-3.5 h-3.5" />
                </button>

                <div className="w-px h-4 bg-sthira-border-strong mx-1" />

                <button
                  type="button"
                  onClick={() => handleApplyFormat('slash')}
                  className="inline-flex items-center gap-1 px-2.5 py-1 rounded-lg bg-sthira-surface-muted hover:bg-sthira-surface-muted text-sthira-text font-medium transition-colors cursor-pointer"
                  title="Open Slash Command Popover"
                >
                  <span className="font-mono font-bold">/</span>
                  <span>Insert Block</span>
                </button>
              </div>

              {/* Right: Clean Binary Mode Switcher & Page Action Menu */}
              <div className="focus-document-actions">
                {/* Chained Synthesis: Refine & Derive Toolbar Actions */}
                <div className="focus-ai-actions">
                  <button
                    type="button"
                    data-testid="btn-refine-artifact"
                    onClick={async () => { if (await handleSave()) setIsRefineModalOpen(true); }}
                    disabled={!context?.artifact?.id && !artifactId}
                    className="inline-flex items-center gap-1.5 px-2.5 py-1.5 rounded-md text-xs font-medium bg-sthira-surface border border-sthira-border text-sthira-text-muted hover:bg-sthira-surface-muted hover:text-sthira-text transition-all shadow-2xs cursor-pointer disabled:opacity-50"
                    title="Iteratively refine or re-research note with custom critique"
                  >
                    <RefreshCw className="w-3.5 h-3.5 text-sthira-primary" />
                    <span>Refine</span>
                  </button>

                  <div className="relative" ref={deriveMenuRef}>
                    <button
                      type="button"
                      data-testid="btn-derive-menu"
                      aria-expanded={isDeriveMenuOpen}
                      aria-controls="focus-derive-options"
                      onClick={() => setIsDeriveMenuOpen((v) => !v)}
                      disabled={(!context?.artifact?.id && !artifactId) || isDeriving}
                      className="inline-flex items-center gap-1 px-2.5 py-1.5 rounded-md text-xs font-medium bg-sthira-surface border border-sthira-border text-sthira-text-muted hover:bg-sthira-surface-muted hover:text-sthira-text transition-all shadow-2xs cursor-pointer disabled:opacity-50"
                      title="Derive execution plan, discrete ideas, or task checklist"
                    >
                      {isDeriving ? (
                        <Loader2 className="w-3.5 h-3.5 animate-spin text-amber-600" />
                      ) : (
                        <Zap className="w-3.5 h-3.5 text-sthira-accent" />
                      )}
                      <span>Derive</span>
                      <ChevronDown className="w-3 h-3 text-sthira-text-subtle" />
                    </button>

                    {isDeriveMenuOpen && (
                      <div
                        data-testid="derive-dropdown-menu"
                        id="focus-derive-options"
                        className="absolute right-0 top-full mt-1.5 w-60 bg-sthira-surface-raised border border-sthira-border rounded-xl shadow-lg py-1.5 z-40 animate-in fade-in duration-100 flex flex-col"
                      >
                        <div className="px-3 py-1.5 text-[10px] font-semibold uppercase tracking-wider text-sthira-text-subtle border-b border-sthira-border mb-1">
                          Derive Chained Artifact
                        </div>
                        <button
                          type="button"
                          data-testid="btn-derive-plan"
                          onClick={() => handleDerive('plan_from_research')}
                          className="flex items-center gap-2.5 px-3 py-2 text-left text-xs text-sthira-text hover:bg-sthira-surface-muted transition-colors cursor-pointer group"
                        >
                          <BookOpen size={16} />
                          <div className="flex flex-col">
                            <span className="font-medium group-hover:text-emerald-700 dark:group-hover:text-emerald-400">
                              Derive Execution Plan
                            </span>
                            <span className="text-[10px] text-sthira-text-subtle">
                              Synthesizes phased implementation milestones
                            </span>
                          </div>
                        </button>

                        <button
                          type="button"
                          data-testid="btn-derive-ideas"
                          onClick={() => handleDerive('extract_ideas')}
                          className="flex items-center gap-2.5 px-3 py-2 text-left text-xs text-sthira-text hover:bg-sthira-surface-muted transition-colors cursor-pointer group"
                        >
                          <Lightbulb size={16} />
                          <div className="flex flex-col">
                            <span className="font-medium group-hover:text-amber-700 dark:group-hover:text-amber-400">
                              Extract Discrete Ideas
                            </span>
                            <span className="text-[10px] text-sthira-text-subtle">
                              Deconstructs into discrete hypothesis objects
                            </span>
                          </div>
                        </button>

                        <button
                          type="button"
                          data-testid="btn-derive-tasks"
                          onClick={() => handleDerive('extract_tasks')}
                          className="flex items-center gap-2.5 px-3 py-2 text-left text-xs text-sthira-text hover:bg-sthira-surface-muted transition-colors cursor-pointer group"
                        >
                          <CheckSquare size={16} />
                          <div className="flex flex-col">
                            <span className="font-medium group-hover:text-blue-700 dark:group-hover:text-blue-400">
                              Extract Action Checklist
                            </span>
                            <span className="text-[10px] text-sthira-text-subtle">
                              Consolidated checklist of actionable tasks
                            </span>
                          </div>
                        </button>
                      </div>
                    )}
                  </div>
                </div>

                {/* Phase 17: Page Action Menu (top-right, beside the Edit ⟷ Preview toggle) */}
                {(context?.artifact?.id || artifactId) && (
                  <PageActionMenu
                    variant="toolbar"
                    onOpenEvidence={() => { setShowMargin(true); setShowChat(false); closeConnections(); }}
                    artifactId={context?.artifact?.id || artifactId || ''}
                    artifactTitle={title || context?.artifact?.title || 'Untitled Note'}
                    contentMd={editorContent}
                    projectId={projectId || null}
                    objectTypes={objectTypes}
                    onChanged={() => {
                      loadProjectData();
                    }}
                    onOpenArtifact={(id, pid) => {
                      if (onOpenArtifact) {
                        onOpenArtifact(id, pid ?? null);
                      } else {
                        loadProjectData();
                      }
                    }}
                    onProjectCreated={(newProjectId) => {
                      onOpenArtifact?.(
                        context?.artifact?.id || artifactId || '',
                        newProjectId
                      );
                    }}
                    onConverted={() => {
                      loadProjectData();
                    }}
                    onDeleted={() => {
                      onBack();
                    }}
                  />
                )}
              </div>
            </div>

            {/* Inline AI Streaming Status Banner */}
            {isAiStreaming && (
              <div className="focus-inline-stream">
                <div className="flex items-center gap-2">
                  <Sparkles className="w-4 h-4 text-sthira-primary animate-spin" />
                  <span className="font-semibold">{aiStreamingMessage || 'Generating with active model...'}</span>
                </div>
                <span className="text-[10px] text-sthira-text-muted font-mono">streaming tokens</span>
              </div>
            )}

            <div className="focus-editor-body" ref={editorBodyRef}>
              <DocumentEditor
                key={context?.artifact?.id || artifactId || 'draft'}
                ref={documentEditorRef}
                content={editorContent}
                title={title}
                objectTypes={objectTypes}
                disabled={isAiStreaming || chatApplying}
                onChange={handleEditorChange}
                onBlur={() => { if (isDirty && !isLoadingDocument) void handleSave(); }}
                onOpenEntity={openReference}
                onSlash={(query, position) => setSlashMenu({ isOpen: query !== null, filterQuery: query || '', position, caretIndex: 0 })}
                onRecordDecision={async (text) => {
                  try {
                    await createTypedObject('decision', text.slice(0, 60).replace(/[*_#`]/g, '').trim() || 'Recorded Decision', { status: 'Recorded', rationale: text });
                    window.dispatchEvent(new CustomEvent('sthira:refresh_objects'));
                  } catch { setActionError('Couldn’t record this decision. Your page is still saved.'); }
                }}
              />
              {slashMenu.isOpen && <SlashCommandMenu anchorElement={editorBodyRef.current} filterQuery={slashMenu.filterQuery} customObjectTypes={objectTypes} onSelect={handleSelectSlashCommand} onClose={() => setSlashMenu((prev) => ({ ...prev, isOpen: false }))} position={slashMenu.position} activeModelName={activeModelName} />}
              {templateSelector?.isOpen && <TemplateSelectorPopover objectType={templateSelector.objectType} position={templateSelector.position} onSelect={(templateId) => {
                setPendingTemplateId(templateId);
                setInlineObjectCreator({ isOpen: true, objectType: templateSelector.objectType, position: templateSelector.position, caretIndex: 0 });
                setTemplateSelector(null);
              }} onClose={() => setTemplateSelector(null)} />}
              {inlineObjectCreator?.isOpen && <QuickObjectCreatorCard objectType={inlineObjectCreator.objectType} position={inlineObjectCreator.position} onSubmit={handleCreateAndInsertObject} onClose={() => setInlineObjectCreator(null)} />}
            </div>
            </>}
          </div>
        </div>

        {conversation && artifactId && chatArtifactId === artifactId && <DocumentChatPanel sessionNamespace={sessionNamespace} key={artifactId} artifactId={artifactId} initialConversation={conversation}
          open={showChat} busy={isLoadingDocument || isAiStreaming || (documentRun?.artifactId === artifactId && documentRun?.status === 'running')} modelId={conversation.model_id || activeModelId} modelName={conversation.model_name || activeModelName}
          currentContent={editorContent} onApplyingChange={setChatApplying} onClose={() => setChatOpen(false)} beforeRequest={async () => {
            if (autoSaveTimerRef.current) clearTimeout(autoSaveTimerRef.current);
            const id = artifactId;
            const request = loadRequestRef.current;
            const saved = await handleSave();
            return saved && request === loadRequestRef.current && currentArtIdRef.current === id;
          }}
          onApplied={(artifact, changeSetId) => {
            if (currentArtIdRef.current !== artifact.id) return;
            currentContentRef.current = artifact.content_md;
            lastSavedContentRef.current = artifact.content_md;
            setEditorContent(artifact.content_md);
            setContext(previous => previous ? { ...previous, artifact } : previous);
            onSave?.(artifact);
            if (changeSetId) setUndoToast({ message: 'Conversation edit applied', changeSetId, artifactId: artifact.id });
          }} />}
        {showConnections && connectionEntity && <ConnectionsPanel
          key={`${connectionEntity.kind}-${connectionEntity.id}`}
          entity={connectionEntity}
          title={connectionEntity.kind === 'project' ? projectLabel : title || 'Untitled page'}
          project={connectionProjectId && connectionArtifactId ? { id: connectionProjectId, title: projectLabel } : null}
          scope={connectionScope}
          onScopeChange={(scope) => { setConnectionScope(scope); setConnectionPreview(false); setRequestedPreview(null); }}
          onClose={closeConnections}
          onPreviewChange={setConnectionPreview}
          requestedPreview={requestedPreview}
          onInsertLink={!isLoadingDocument && !loadError ? insertReference : undefined}
          onOpenFull={onOpenArtifact ? async (entity) => {
            if (isDirty && !(await handleSave())) return;
            if (entity.kind === 'project') onOpenProject?.(entity.id);
            else onOpenArtifact(entity.id, entity.project_id);
          } : undefined}
        />}

        {/* Right Docked Collapsible Sidebar: Epistemic Margin */}
        {showMargin && (
          <aside
            data-testid="epistemic-margin-sidebar"
            id="focus-evidence-margin"
            aria-label="Epistemic margin"
            className="focus-margin"
          >
            <div className="focus-margin-heading">
              <div className="flex items-center gap-2">
                <Quote className="w-4 h-4 text-sthira-primary" />
                <h3 className="text-xs font-bold text-sthira-text uppercase tracking-wider font-sans">
                  Epistemic Margin
                </h3>
              </div>
              <div className="flex items-center gap-1.5">
                <span className="text-[10px] px-2 py-0.5 rounded-full bg-sthira-surface-muted text-sthira-primary font-bold">
                  {/* Anchored Quote Preview */}
            {hoveredQuote && (
              <div className="focus-quote-preview mb-4 bg-sthira-canvas border border-sthira-border-strong rounded-xl px-4 py-2.5 text-xs text-[#2D5A27] dark:text-[#88D080] font-medium flex items-center gap-2 animate-in fade-in">
                <Quote className="w-3.5 h-3.5 shrink-0" />
                <span className="truncate">Anchored Quote: &ldquo;{hoveredQuote}&rdquo;</span>
              </div>
            )}

            {marginTab === 'insights' ? (context?.epistemic_nodes?.length || 0) : sourceCount}
                </span>
                <button
                  type="button"
                  onClick={() => setShowMargin(false)}
                  className="text-sthira-text-subtle hover:text-sthira-text p-1 rounded-lg hover:bg-sthira-surface-muted transition-colors cursor-pointer"
                  title="Close epistemic margin"
                  aria-label="Close epistemic margin"
                >
                  <X className="w-3.5 h-3.5" />
                </button>
              </div>
            </div>

            {/* Segmented Control: Insights vs Sources (Matching Mockup 3) */}
            <div className="focus-margin-tabs" role="group" aria-label="Evidence view">
              <button
                type="button"
                data-testid="btn-margin-tab-insights"
                aria-pressed={marginTab === 'insights'}
                onClick={() => setMarginTab('insights')}
                className={`flex-1 py-1 rounded-lg text-center font-medium transition-all cursor-pointer flex items-center justify-center gap-1.5 ${
                  marginTab === 'insights'
                    ? 'bg-sthira-surface text-sthira-text font-semibold shadow-2xs'
                    : 'text-sthira-text-muted hover:text-sthira-text'
                }`}
              >
                <Sparkles className="w-3 h-3 text-sthira-primary" />
                <span>Insights</span>
              </button>
              <button
                type="button"
                data-testid="btn-margin-tab-sources"
                aria-pressed={marginTab === 'sources'}
                onClick={() => setMarginTab('sources')}
                className={`flex-1 py-1 rounded-lg text-center font-medium transition-all cursor-pointer flex items-center justify-center gap-1.5 ${
                  marginTab === 'sources'
                    ? 'bg-sthira-surface text-sthira-text font-semibold shadow-2xs'
                    : 'text-sthira-text-muted hover:text-sthira-text'
                }`}
              >
                <Globe className="w-3 h-3 text-sthira-primary" />
                <span>Sources</span>
              </button>
            </div>

            {marginTab === 'insights' ? (
              <>
                {/* Grounded Epistemic Nodes */}
                {context?.epistemic_nodes && context.epistemic_nodes.length > 0 ? (
                  <div className="space-y-3">
                    {context.epistemic_nodes.map((node) => (
                      <div
                        key={node.id}
                        tabIndex={0}
                        onFocus={() => setHoveredQuote(node.quote || null)}
                        onBlur={() => setHoveredQuote(null)}
                        onMouseEnter={() => setHoveredQuote(node.quote || null)}
                        onMouseLeave={() => setHoveredQuote(null)}
                        className="focus-evidence-card bg-sthira-surface border border-sthira-border rounded-xl p-3 text-xs hover:border-sthira-border-strong hover:shadow-2xs transition-all"
                      >
                        <div className="flex items-center justify-between mb-1.5">
                          <EpistemicBadge origin={node.origin} />
                          <span className="text-[10px] text-sthira-text-muted font-medium capitalize">
                            {node.node_type}
                          </span>
                        </div>
                        <p className="font-semibold text-sthira-text mb-1">
                          {node.statement}
                        </p>
                        {node.quote && (
                          <p className="text-[11px] text-sthira-text-muted italic bg-sthira-canvas p-2 rounded-lg border border-sthira-border mt-1.5">
                            &ldquo;{node.quote}&rdquo;
                          </p>
                        )}
                      </div>
                    ))}
                  </div>
                ) : (
                  <div className="text-xs text-sthira-text-muted leading-relaxed bg-sthira-surface border border-sthira-border rounded-xl p-4 text-center">
                    <Info className="w-4 h-4 mx-auto mb-2 text-sthira-text-subtle" />
                    No extracted claims for this document yet. Recorded statements and their provenance will appear here.
                  </div>
                )}

                {/* Ambient Graph Proposals */}
                {context?.pending_proposals && context.pending_proposals.length > 0 && (
                  <div className="pt-2 border-t border-sthira-border space-y-3">
                    <h4 className="text-xs font-bold text-sthira-text-muted uppercase tracking-wider flex items-center gap-1.5">
                      <Sparkles className="w-3.5 h-3.5 text-[#8C6D1F] dark:text-[#D4A738]" />
                      Ambient Proposals
                    </h4>
                    {context.pending_proposals.map((prop) => (
                      <AmbientProposalPill
                        key={prop.id}
                        proposal={prop}
                        onAccept={handleAcceptProposal}
                        onDismiss={handleDismissProposal}
                      />
                    ))}
                  </div>
                )}
              </>
            ) : (
              /* Sources Tab Content */
              <div className="space-y-3">
                {context?.artifact?.source_url ? (
                  <div className="bg-sthira-surface border border-sthira-border rounded-xl p-3 text-xs space-y-1.5 shadow-2xs">
                    <div className="flex items-center justify-between">
                      <span className="text-[10px] uppercase font-bold text-sthira-primary tracking-wider">
                        Primary Source
                      </span>
                      <ExternalLink className="w-3 h-3 text-sthira-text-subtle" />
                    </div>
                    <a
                      href={context.artifact.source_url}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="font-medium text-sthira-text hover:underline break-all block"
                    >
                      {context.artifact.source_url}
                    </a>
                  </div>
                ) : null}

                {(() => {
                  const linkMatches = sourceLinks;
                  if (!context?.artifact?.source_url && linkMatches.length === 0) {
                    return (
                      <div className="text-xs text-sthira-text-muted leading-relaxed bg-sthira-surface border border-sthira-border rounded-xl p-4 text-center">
                        <Globe className="w-4 h-4 mx-auto mb-2 text-sthira-text-subtle" />
                        No external sources linked yet. External citations or web search references will appear here.
                      </div>
                    );
                  }
                  return (
                    <div className="space-y-2">
                      {linkMatches.map((match, i) => (
                        <div
                          key={i}
                          className="bg-sthira-surface border border-sthira-border rounded-xl p-3 text-xs shadow-2xs hover:border-sthira-border-strong transition-all"
                        >
                          <div className="flex items-center justify-between gap-1 mb-1">
                            <span className="font-semibold text-sthira-text truncate">
                              {match[1]}
                            </span>
                            <ExternalLink className="w-3 h-3 text-sthira-text-subtle shrink-0" />
                          </div>
                          <a
                            href={match[2]}
                            target="_blank"
                            rel="noopener noreferrer"
                            className="text-[11px] text-sthira-primary hover:underline truncate block"
                          >
                            {match[2]}
                          </a>
                        </div>
                      ))}
                    </div>
                  );
                })()}
              </div>
            )}
          </aside>
        )}
      </div>

      {/* History Drawer */}
      {history && (
        <ProjectHistoryDrawer
          history={history}
          isOpen={isHistoryOpen}
          onClose={() => setIsHistoryOpen(false)}
          onSelectArtifact={navigateToArtifact}
        />
      )}

      {/* Undo Toast */}
      {undoToast && (
        <UndoToast
          message={undoToast.message}
          onUndo={handleUndo}
          onClose={() => setUndoToast(null)}
        />
      )}

      {/* Chained Synthesis: Refine / Re-Research Modal */}
      <RefineArtifactModal
        isOpen={isRefineModalOpen}
        onClose={() => setIsRefineModalOpen(false)}
        artifactId={context?.artifact?.id || artifactId || ''}
        artifactTitle={title || context?.artifact?.title || 'Untitled Note'}
        baseContent={currentContentRef.current}
        baseTitle={currentTitleRef.current || title || context?.artifact?.title || ''}
        onRefined={(newContent, targetArtifactId, baseSnapshot) => {
          const currentId = context?.artifact?.id || artifactId;
          if (targetArtifactId && currentId && targetArtifactId !== currentId) {
            return;
          }

          // If the user made newer edits to the body or title after refinement began,
          // preserve the newer local draft. Do not silently overwrite or feed into autosave.
          if (baseSnapshot) {
            const normalize = (s: string) => s.replace(/\r\n/g, '\n').trim();
            const isDraftModified =
              normalize(currentContentRef.current) !== normalize(baseSnapshot.content) ||
              currentTitleRef.current.trim() !== baseSnapshot.title.trim();

            if (isDraftModified) {
              setPendingRefinement({
                targetArtifactId: targetArtifactId || currentId || '',
                refinedContent: newContent,
                refinedTitle: title || context?.artifact?.title || 'Untitled Note',
              });
              return;
            }
          }

          handleEditorChange(newContent);
          onContentUpdate?.(newContent);
          loadProjectData();
        }}
      />
    </div>
  );
};

