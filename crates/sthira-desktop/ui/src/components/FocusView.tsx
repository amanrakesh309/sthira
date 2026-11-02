import React, { useState, useEffect, useRef, useCallback } from 'react';
import {
  ArrowLeft,
  Sparkles,
  History,
  LayoutGrid,
  Check,
  Quote,
  Search,
  Loader2,
  Bold,
  Italic,
  CheckSquare,
  Code as CodeIcon,
  Table as TableIcon,
  Eye,
  Edit3,
  PanelRightClose,
  PanelRightOpen,
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
} from 'lucide-react';
import { RefineArtifactModal } from './RefineArtifactModal';
import { ContextualResumePane } from './ContextualResumePane';
import { ProjectHistoryDrawer } from './ProjectHistoryDrawer';
import { AmbientProposalPill } from './AmbientProposalPill';
import { UndoToast } from './UndoToast';
import { EpistemicBadge } from './EpistemicBadge';
import { SlashCommandMenu, type SlashCommandItem } from './SlashCommandMenu';
import type { DocumentRunState } from '../types/sthira';
import { ThemeToggle } from './ThemeToggle';
import { QuickObjectCreatorCard } from './objects/QuickObjectCreatorCard';
import { TemplateSelectorPopover } from './objects/TemplateSelectorPopover';
import { DocumentPropertyHeader } from './objects/DocumentPropertyHeader';
import { PageActionMenu } from './PageActionMenu';
import type { EmbeddedObjectData } from './objects/EmbeddedObjectCard';
import { RichDocumentRenderer } from './RichDocumentRenderer';
import {
  getArtifactContext,
  getResumeBrief,
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
  ResumeBrief,
  ProjectHistory as IProjectHistory,
  ObjectType,
  ObjectInstance,
  DerivationType,
} from '../types/sthira';

interface FocusViewProps {
  projectId?: string | null;
  artifactId?: string;
  onBack: () => void;
  onOpenCanvas: (canvasId?: string) => void;
  onOpenSearch?: () => void;
  onContentUpdate?: (content: string) => void;
  onSave?: (savedArtifact: Artifact) => void;
  onTitleUpdate?: (artifactId: string, newTitle: string) => void;
  /** Phase 17: used by the Page Action Menu to redirect context (e.g. after duplicating). */
  onOpenArtifact?: (artifactId: string, projectId?: string | null) => void;
  realtimeTitleUpdate?: { id: string; title: string } | null;
  /**
   * Phase 19/20: live streaming run (research pipeline stages or plan tokens) to render
   * as a progress banner and live stream inside the full-width center document canvas.
   */
  documentRun?: DocumentRunState | null;
  onDismissDocumentRun?: () => void;
}

export const FocusView: React.FC<FocusViewProps> = ({
  projectId,
  artifactId,
  onBack,
  onOpenCanvas,
  onOpenSearch,
  onContentUpdate,
  onSave,
  onTitleUpdate,
  onOpenArtifact,
  realtimeTitleUpdate,
  documentRun,
  onDismissDocumentRun,
}) => {
  const [context, setContext] = useState<ArtifactContext | null>(null);
  const [brief, setBrief] = useState<ResumeBrief | null>(null);
  const [history, setHistory] = useState<IProjectHistory | null>(null);

  const [editorContent, setEditorContent] = useState('');
  const [title, setTitle] = useState('');
  const [isBriefOpen, setIsBriefOpen] = useState(true);
  const [isExpandedReview, setIsExpandedReview] = useState(false);
  const [isHistoryOpen, setIsHistoryOpen] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [saveSuccess, setSaveSuccess] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [isLoadingDocument, setIsLoadingDocument] = useState(true);
  const [isReviewLoading, setIsReviewLoading] = useState(false);
  const [reviewError, setReviewError] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const loadRequestRef = useRef(0);
  const briefProjectRef = useRef<string | null>(null);
  const [hoveredQuote, setHoveredQuote] = useState<string | null>(null);
  const [undoToast, setUndoToast] = useState<{ message: string; changeSetId: string } | null>(null);

  // Layout & Clean Binary Mode State ([ ✏️ Edit ] ⟷ [ 👁️ Preview ])
  const [showMargin, setShowMargin] = useState(false);
  const [marginTab, setMarginTab] = useState<'insights' | 'sources'>('insights');
  const [viewMode, setViewMode] = useState<'edit' | 'preview'>('edit');
  const [activeModelName, setActiveModelName] = useState('Active Model');
  const [activeModelId, setActiveModelId] = useState<string | undefined>(undefined);

  // Global Cmd/Ctrl + E Shortcut to Toggle Between Edit and Preview
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'e') {
        e.preventDefault();
        setViewMode((prev) => (prev === 'edit' ? 'preview' : 'edit'));
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

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
    setIsDeriveMenuOpen(false);
    setIsDeriving(true);
    setActionError(null);
    try {
      const newArtId = await deriveArtifact(currentArtId, derivationType, null, activeModelId);
      if (newArtId && onOpenArtifact) {
        onOpenArtifact(newArtId, projectId || null);
      }
    } catch (err) {
      console.error('Derivation failed:', err);
      setActionError('Couldn’t derive a new artifact. Check the selected provider and try again.');
    } finally {
      setIsDeriving(false);
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

  const textareaRef = useRef<HTMLTextAreaElement>(null);
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
    window.addEventListener('sthira:refresh_objects', handleRefresh);
    return () => window.removeEventListener('sthira:refresh_objects', handleRefresh);
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
      for (const prov of catalog) {
        if (!prov.is_enabled) continue;
        const first = prov.models.find((m) => m.is_enabled);
        if (first) {
          setActiveModelName(first.display_name || first.model_identifier);
          setActiveModelId(first.id);
          return;
        }
      }
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
    setEditorContent(ctx.artifact.content_md);
    setTitle(nextTitle);
    currentContentRef.current = ctx.artifact.content_md;
    currentTitleRef.current = nextTitle;
    currentArtIdRef.current = ctx.artifact.id;
    currentProjectIdRef.current = projectId || null;
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
    let projectBrief: ResumeBrief | null = null;
    const hasProject = projectId && projectId !== 'default_project';
    // History is auxiliary: an unavailable timeline must not block editing.
    setHistory(null);
    if (hasProject) {
      getProjectHistory(projectId).then((value) => {
        if (request === loadRequestRef.current) setHistory(value);
      }).catch((error) => console.warn('Project history unavailable', error));
      try {
        projectBrief = await getResumeBrief(projectId, 1);
        loadedArtId ||= projectBrief?.primary_continuation_artifact_id || undefined;
      } catch (error) {
        console.warn('Brief not available for project', projectId, error);
      }
    }
    if (request !== loadRequestRef.current) return;
    setBrief(projectBrief);
    setIsExpandedReview(false);
    setReviewError(null);
    if (briefProjectRef.current !== (projectId || null)) {
      briefProjectRef.current = projectId || null;
      setIsBriefOpen(!projectBrief?.pane_hint || 'show' in projectBrief.pane_hint);
    }
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
    if (!isInitialLoadedRef.current) return false;
    const artId = context?.artifact?.id || artifactId || '';
    const currentT = currentTitleRef.current.trim() || context?.artifact?.title || null;
    const currentC = currentContentRef.current;

    setIsSaving(true);
    setSaveSuccess(false);
    setSaveError(null);
    try {
      const targetProjectId = currentProjectIdRef.current && currentProjectIdRef.current !== 'default_project' ? currentProjectIdRef.current : null;
      const updated = await saveArtifact(artId, currentC, currentT, targetProjectId);
      lastSavedContentRef.current = currentC;
      lastSavedTitleRef.current = updated.title || currentT || '';
      if (currentTitleRef.current === currentT) {
        setTitle(updated.title || currentT || '');
      }
      try {
        const ctx = await getArtifactContext(updated.id);
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
      console.error('Failed to save artifact', err);
      setSaveError('Couldn’t save your changes. Try saving again.');
      return false;
    } finally {
      setIsSaving(false);
    }
  }, [context, artifactId, projectId, onSave]);

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
      handleSave();
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

  const handleEditorChange = (newC: string, e?: React.ChangeEvent<HTMLTextAreaElement>) => {
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

    // Caret tracking for Slash Command Menu
    if (e && textareaRef.current) {
      const caret = textareaRef.current.selectionStart;
      const textBefore = newC.slice(0, caret);
      const currentLine = textBefore.split('\n').pop() || '';
      const slashMatch = currentLine.match(/(?:^|\s)\/([a-zA-Z0-9_-]*)$/);

      if (slashMatch) {
        const query = slashMatch[1];
        const lineCount = textBefore.split('\n').length;
        const top = Math.min(lineCount * 22 + 60, 480);
        setSlashMenu({
          isOpen: true,
          filterQuery: query,
          position: { top, left: 40 },
          caretIndex: caret,
        });
      } else {
        setSlashMenu((prev) => (prev.isOpen ? { ...prev, isOpen: false } : prev));
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

    const caret = slashMenu.caretIndex || (textareaRef.current ? textareaRef.current.selectionStart : editorContent.length);
    const before = editorContent.slice(0, caret);
    const after = editorContent.slice(caret);

    const lastSlashIdx = before.lastIndexOf('/');
    const cleanBefore = lastSlashIdx >= 0 ? before.slice(0, lastSlashIdx) : before;

    const newContent = `${cleanBefore}${item.template}${after}`;
    setEditorContent(newContent);
    currentContentRef.current = newContent;

    if (item.id === 'decision') {
      handleSave();
    }

    setTimeout(() => {
      if (textareaRef.current) {
        const newCaret = cleanBefore.length + item.template.length;
        textareaRef.current.focus();
        textareaRef.current.setSelectionRange(newCaret, newCaret);
      }
    }, 50);
  };

  // Inline AI Generator
  const handleRunInlineAi = async (mode: 'plan' | 'research' | 'expand') => {
    const caret = slashMenu.caretIndex || (textareaRef.current ? textareaRef.current.selectionStart : editorContent.length);
    const before = editorContent.slice(0, caret);
    const after = editorContent.slice(caret);

    const lastSlashIdx = before.lastIndexOf('/');
    const cleanBefore = lastSlashIdx >= 0 ? before.slice(0, lastSlashIdx) : before;

    const topic = title.trim() || 'Software Architecture & Planning';
    const actionLabel = mode === 'plan' ? 'Generating Plan' : mode === 'research' ? 'Researching Findings' : 'Expanding Section';

    setIsAiStreaming(true);
    setAiStreamingMessage(`${actionLabel} with ${activeModelName}...`);

    let accumulatedTokens = '';

    try {
      await streamAdaptiveRun(topic, mode === 'expand' ? 'act' : mode, activeModelId, (chunk) => {
        if (chunk.type === 'text_token') {
          accumulatedTokens += chunk.payload;
          const liveContent = `${cleanBefore}\n\n${accumulatedTokens}\n\n${after}`;
          setEditorContent(liveContent);
          currentContentRef.current = liveContent;
        }
      });
      handleSave();
    } catch (err) {
      console.error('Inline AI streaming failed', err);
      setActionError('The model stream stopped. Your current draft is still available.');
    } finally {
      setIsAiStreaming(false);
      setAiStreamingMessage(null);
    }
  };

  // Quick Object Creator Submission Handler
  const handleCreateAndInsertObject = async (objTitle: string, properties: Record<string, any>) => {
    if (!inlineObjectCreator) return;
    const { objectType, caretIndex } = inlineObjectCreator;
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

      const caret = caretIndex || (textareaRef.current ? textareaRef.current.selectionStart : editorContent.length);
      const before = editorContent.slice(0, caret);
      const after = editorContent.slice(caret);

      const lastSlashIdx = before.lastIndexOf('/');
      const cleanBefore = lastSlashIdx >= 0 ? before.slice(0, lastSlashIdx) : before;

      const newContent = `${cleanBefore}${directive}${after}`;
      setEditorContent(newContent);
      currentContentRef.current = newContent;
      handleSave();

      // 3. Refresh Left Sidebar object counts automatically
      window.dispatchEvent(new CustomEvent('sthira:refresh_objects'));
    } catch (err) {
      console.error('Failed to create typed object instance', err);
    }
  };

  // Embedded Object Live Property Updates
  const handleUpdateEmbeddedObject = (updated: EmbeddedObjectData) => {
    const regex = new RegExp(
      `\`\`\`sthira-object\\s*\\n[\\s\\S]*?"artifact_id"\\s*:\\s*"${updated.artifact_id}"[\\s\\S]*?\\n\`\`\``,
      'g'
    );
    const replacement = `\`\`\`sthira-object\n${JSON.stringify(updated, null, 2)}\n\`\`\``;
    const newContent = editorContent.replace(regex, replacement);
    if (newContent !== editorContent) {
      setEditorContent(newContent);
      currentContentRef.current = newContent;
      handleSave();
      window.dispatchEvent(new CustomEvent('sthira:refresh_objects'));
    }
  };

  // Toolbar Formatting helper
  const handleApplyFormat = (type: 'bold' | 'italic' | 'h1' | 'h2' | 'todo' | 'code' | 'table' | 'slash') => {
    if (!textareaRef.current) return;
    const start = textareaRef.current.selectionStart;
    const end = textareaRef.current.selectionEnd;
    const selected = editorContent.substring(start, end);

    let insertion = '';
    let cursorOffset = 0;

    switch (type) {
      case 'bold':
        insertion = `**${selected || 'bold text'}**`;
        cursorOffset = insertion.length;
        break;
      case 'italic':
        insertion = `*${selected || 'italic text'}*`;
        cursorOffset = insertion.length;
        break;
      case 'h1':
        insertion = `\n# ${selected || 'Heading 1'}\n`;
        cursorOffset = insertion.length;
        break;
      case 'h2':
        insertion = `\n## ${selected || 'Heading 2'}\n`;
        cursorOffset = insertion.length;
        break;
      case 'todo':
        insertion = `\n- [ ] ${selected || 'Action item'}\n`;
        cursorOffset = insertion.length;
        break;
      case 'code':
        insertion = `\n\`\`\`rust\n${selected || '// Implementation'}\n\`\`\`\n`;
        cursorOffset = insertion.length;
        break;
      case 'table':
        insertion = `\n| Criteria | Option A | Option B |\n| :--- | :--- | :--- |\n| Metric | High | Medium |\n`;
        cursorOffset = insertion.length;
        break;
      case 'slash':
        setSlashMenu({
          isOpen: true,
          filterQuery: '',
          position: { top: 120, left: 60 },
          caretIndex: start,
        });
        return;
    }

    const nextContent = editorContent.slice(0, start) + insertion + editorContent.slice(end);
    setEditorContent(nextContent);
    currentContentRef.current = nextContent;

    setTimeout(() => {
      if (textareaRef.current) {
        textareaRef.current.focus();
        textareaRef.current.setSelectionRange(start + cursorOffset, start + cursorOffset);
      }
    }, 50);
  };

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

  const handleExpandReview = async () => {
    if (!brief || !projectId || projectId === 'default_project' || isReviewLoading) return;
    setIsReviewLoading(true);
    setReviewError(null);
    try {
      const nextLevel = isExpandedReview ? 1 : 2;
      const b = await getResumeBrief(projectId, nextLevel);
      setBrief(b);
      setIsExpandedReview(!isExpandedReview);
    } catch (err) {
      console.error('Failed to toggle expanded review', err);
      setReviewError('Couldn’t load the review. Try again when the vault is available.');
    } finally {
      setIsReviewLoading(false);
    }
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
    try {
      await revertChangeSet(undoToast.changeSetId);
      setUndoToast(null);
      loadProjectData();
    } catch (err) {
      console.error('Failed to undo change set', err);
    }
  };

  const sourceLinks = Array.from(editorContent.matchAll(/\[([^\]]+)\]\((https?:\/\/[^\)]+)\)/g));
  const sourceCount = new Set([context?.artifact?.source_url, ...sourceLinks.map((match) => match[2])].filter(Boolean)).size;
  const projectLabel = projectId
    ? brief?.active_working_breadcrumb?.replace(/^Project:\s*/i, '').split(/\s*[>›]\s*/)[0] || 'Project workspace'
    : 'Notes';
  const isDirty = editorContent !== lastSavedContentRef.current || title !== lastSavedTitleRef.current;

  return (
    <div className="focus-workspace flex-1 h-full overflow-hidden flex flex-col" data-margin-open={showMargin}>
      <header className="focus-header">
        <nav className="focus-breadcrumbs" aria-label="Document location">
          <button type="button" onClick={handleBack} className="focus-home-button" title="Return to Home" aria-label="Home"><ArrowLeft size={16} /><span>Home</span></button>
          <span aria-hidden="true">/</span><span className="focus-project-breadcrumb" title={projectLabel}>{projectLabel}</span>
          <span aria-hidden="true">/</span><span className="focus-current-breadcrumb" title={title}>{title || 'Untitled Note'}</span>
        </nav>
        <div className="focus-header-actions">
          {onOpenSearch && <button type="button" onClick={onOpenSearch} className="focus-search-action focus-icon-button" aria-label="Search workspace" title="Search workspace"><Search size={16} /></button>}
          <button type="button" className="focus-header-action" onClick={() => onOpenCanvas()} aria-label="Canvas" title="Open canvas"><LayoutGrid size={16} /><span>Canvas</span></button>
          <button type="button" className="focus-header-action" onClick={() => setIsHistoryOpen(true)} disabled={!history} aria-label="History" title={history ? 'Project history' : 'History is available for project documents'}><History size={16} /><span>History</span></button>
          <button type="button" className="focus-header-action" data-testid="btn-toggle-epistemic-margin" onClick={() => setShowMargin((shown) => !shown)} aria-expanded={showMargin} aria-controls="focus-evidence-margin" title="Toggle Epistemic Margin Sidebar" aria-label="Margin">
            {showMargin ? <PanelRightClose size={16} /> : <PanelRightOpen size={16} />}<span>Margin</span>
          </button>
          <ThemeToggle variant="icon" />
          <button type="button" data-testid="btn-save-artifact" className={`focus-save-status ${saveError ? 'focus-save-error' : ''}`} onClick={handleSave} disabled={isLoadingDocument || !!loadError || isSaving} title={saveError ? 'Retry saving your changes' : 'Changes save automatically. Click to save now.'}>
            {isSaving ? <Loader2 size={15} className="animate-spin" /> : saveError ? <AlertTriangle size={15} /> : <Check size={15} />}
            <span aria-live="polite">{saveError ? 'Save failed' : isSaving ? 'Saving...' : saveSuccess ? 'Saved' : isDirty ? 'Unsaved changes' : 'Auto-saved'}</span>
          </button>
        </div>
      </header>

      {/* Main Full-Height Workspace Body */}
      <div className="focus-workspace-body">
        {/* Generous Full-Height Document Canvas */}
        <div className="focus-document-scroll" data-testid="document-scroll-region">
          <div
            data-testid="editor-canvas"
            className="focus-document-sheet flex-1 min-h-full relative"
          >
            {actionError && <div className="focus-error focus-save-notice" role="alert"><span>{actionError}</span><button type="button" onClick={() => setActionError(null)}>Dismiss</button></div>}
            {saveError && <div className="focus-error focus-save-notice" role="alert"><span>{saveError}</span><button type="button" onClick={handleSave}>Retry save</button></div>}
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
                  <p className="mt-0.5 truncate" data-testid="document-run-message">
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

            {/* Contextual Resume Brief */}
            {brief && (
              <div className="focus-resume-region">
                <ContextualResumePane
                  brief={brief}
                  isOpen={isBriefOpen}
                  onToggleOpen={() => setIsBriefOpen(!isBriefOpen)}
                  onExpandReview={handleExpandReview}
                  isExpandedReview={isExpandedReview}
                  isReviewLoading={isReviewLoading}
                  reviewError={reviewError}
                  onSelectArtifact={navigateToArtifact}
                />
              </div>
            )}

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
                data-testid="input-artifact-title"
                value={title}
                onChange={(e) => handleTitleChange(e.target.value)}
                onBlur={handleSave}
                placeholder="Document title..."
                aria-label="Document title"
                title={title}
                className="text-3xl sm:text-4xl font-editorial font-bold tracking-tight text-sthira-text bg-transparent border-none outline-none w-full placeholder-sthira-text-subtle"
              />
            </div>

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
              <div className="focus-format-actions" role="group" aria-label="Markdown formatting">
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
                <div role="group" aria-label="Document view" className="focus-view-modes inline-flex p-0.5 rounded-lg bg-sthira-surface-muted border border-sthira-border-strong">
                  <button
                    type="button"
                    data-testid="btn-toggle-edit-mode"
                    aria-pressed={viewMode === 'edit'}
                    onClick={() => setViewMode('edit')}
                    className={`inline-flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-medium transition-all cursor-pointer ${
                      viewMode === 'edit'
                        ? 'bg-sthira-surface text-sthira-text font-semibold shadow-2xs'
                        : 'text-sthira-text-muted hover:text-sthira-text'
                    }`}
                    title="Edit Markdown (Cmd+E)"
                  >
                    <Edit3 className="w-3.5 h-3.5" />
                    <span>Edit</span>
                  </button>

                  <button
                    type="button"
                    data-testid="btn-toggle-preview-mode"
                    aria-pressed={viewMode === 'preview'}
                    onClick={() => setViewMode('preview')}
                    className={`inline-flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-medium transition-all cursor-pointer ${
                      viewMode === 'preview'
                        ? 'bg-sthira-surface text-sthira-text font-semibold shadow-2xs'
                        : 'text-sthira-text-muted hover:text-sthira-text'
                    }`}
                    title="Component-Decorated Rich Workspace (Cmd+E)"
                  >
                    <Eye className="w-3.5 h-3.5" />
                    <span>Preview</span>
                  </button>
                </div>

                {/* Chained Synthesis: Refine & Derive Toolbar Actions */}
                <div className="focus-ai-actions">
                  <button
                    type="button"
                    data-testid="btn-refine-artifact"
                    onClick={() => setIsRefineModalOpen(true)}
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

            {/* Main Canvas Body: Live Edit vs Component-Decorated Rich Preview */}
            <div className="focus-editor-body">
              {viewMode === 'edit' ? (
                <div className="flex-1 flex flex-col relative">
                  <textarea
                    ref={textareaRef}
                    data-testid="textarea-editor"
                    value={editorContent}
                    onChange={(e) => handleEditorChange(e.target.value, e)}
                    onBlur={handleSave}
                    aria-label="Document Markdown"
                    spellCheck={false}
                    placeholder="Type '/' to insert blocks, record epistemic decisions, or invoke inline AI compute..."
                    className="flex-1 w-full bg-transparent border-none outline-none resize-none font-mono text-sm leading-relaxed text-sthira-text placeholder-sthira-text-subtle min-h-[calc(100vh-140px)]"
                  />

                  {/* Caret-Positioned Notion-Style Slash Command Menu Popover */}
                  {slashMenu.isOpen && (
                    <SlashCommandMenu
                      filterQuery={slashMenu.filterQuery}
                      customObjectTypes={objectTypes}
                      onSelect={handleSelectSlashCommand}
                      onClose={() => setSlashMenu((prev) => ({ ...prev, isOpen: false }))}
                      position={slashMenu.position}
                      activeModelName={activeModelName}
                    />
                  )}

                  {/* Phase 16: Template selector popover when multiple presets exist */}
                  {templateSelector && templateSelector.isOpen && (
                    <TemplateSelectorPopover
                      objectType={templateSelector.objectType}
                      position={templateSelector.position}
                      onSelect={(templateId) => {
                        setPendingTemplateId(templateId);
                        setInlineObjectCreator({
                          isOpen: true,
                          objectType: templateSelector.objectType,
                          position: templateSelector.position,
                          caretIndex: templateSelector.caretIndex,
                        });
                        setTemplateSelector(null);
                      }}
                      onClose={() => setTemplateSelector(null)}
                    />
                  )}

                  {/* Inline Quick Object Creator Card at Caret */}
                  {inlineObjectCreator && inlineObjectCreator.isOpen && (
                    <QuickObjectCreatorCard
                      objectType={inlineObjectCreator.objectType}
                      position={inlineObjectCreator.position}
                      onSubmit={handleCreateAndInsertObject}
                      onClose={() => setInlineObjectCreator(null)}
                    />
                  )}
                </div>
              ) : (
                /* Rich Component-Decorated Markdown Preview Canvas */
                <div data-testid="rich-preview-canvas" className="focus-preview">
                  <RichDocumentRenderer
                    content={editorContent}
                    documentTitle={title}
                    className="focus-prose"
                    projectId={projectId}
                    objectTypes={objectTypes}
                    onUpdateContent={(updated) => {
                      setEditorContent(updated);
                      currentContentRef.current = updated;
                      if (context?.artifact?.id || artifactId) {
                        saveArtifact(context?.artifact?.id || artifactId || '', updated, title).catch(console.warn);
                      }
                    }}
                    onUpdateObject={handleUpdateEmbeddedObject}
                    onRecordDecision={async (decText) => {
                      try {
                        const cleanTitle = decText.slice(0, 60).replace(/[*_#`]/g, '').trim();
                        await createTypedObject('system_decision', cleanTitle || 'Recorded Decision', {
                          status: 'Recorded',
                          rationale: decText,
                        });
                        window.dispatchEvent(new CustomEvent('sthira:refresh_objects'));
                        setSaveSuccess(true);
                        setTimeout(() => setSaveSuccess(false), 2000);
                      } catch (err) {
                        console.warn('Failed to record decision', err);
                      }
                    }}
                  />
                </div>
              )}
            </div>
            </>}
          </div>
        </div>

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
        onRefined={(newContent) => {
          handleEditorChange(newContent);
          onContentUpdate?.(newContent);
          loadProjectData();
        }}
      />
    </div>
  );
};

