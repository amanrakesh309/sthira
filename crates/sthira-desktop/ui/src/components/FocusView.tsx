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
} from 'lucide-react';
import { ContextualResumePane } from './ContextualResumePane';
import { ProjectHistoryDrawer } from './ProjectHistoryDrawer';
import { AmbientProposalPill } from './AmbientProposalPill';
import { UndoToast } from './UndoToast';
import { EpistemicBadge } from './EpistemicBadge';
import { SlashCommandMenu, type SlashCommandItem } from './SlashCommandMenu';
import { ThemeToggle } from './ThemeToggle';
import { QuickObjectCreatorCard } from './objects/QuickObjectCreatorCard';
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
  createTypedObject,
} from '../api/ipc';
import type {
  Artifact,
  ArtifactContext,
  ResumeBrief,
  ProjectHistory as IProjectHistory,
  ObjectType,
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
  realtimeTitleUpdate?: { id: string; title: string } | null;
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
  realtimeTitleUpdate,
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
  const [hoveredQuote, setHoveredQuote] = useState<string | null>(null);
  const [undoToast, setUndoToast] = useState<{ message: string; changeSetId: string } | null>(null);

  // Layout & Clean Binary Mode State ([ ✏️ Edit ] ⟷ [ 👁️ Preview ])
  const [isMarginOpen, setIsMarginOpen] = useState(true);
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

  // Object Types & Inline Quick Creator State
  const [objectTypes, setObjectTypes] = useState<ObjectType[]>([]);
  const [inlineObjectCreator, setInlineObjectCreator] = useState<{
    isOpen: boolean;
    objectType: ObjectType;
    position: { top: number; left: number };
    caretIndex: number;
  } | null>(null);

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
    currentProjectIdRef.current = projectId;
  }, [projectId]);

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

  const loadProjectData = async () => {
    let loadedArtId = artifactId;

    if (!loadedArtId && projectId && projectId !== 'default_project') {
      try {
        const b = await getResumeBrief(projectId, 1);
        setBrief(b);
        if (b.primary_continuation_artifact_id) {
          loadedArtId = b.primary_continuation_artifact_id;
        }
      } catch (err) {
        console.warn('Brief not available for project', projectId, err);
      }
    }

    if (loadedArtId) {
      try {
        const ctx = await getArtifactContext(loadedArtId);
        setContext(ctx);
        setEditorContent(ctx.artifact.content_md);
        setTitle(ctx.artifact.title || '');
        lastSavedContentRef.current = ctx.artifact.content_md;
        lastSavedTitleRef.current = ctx.artifact.title || '';
        isInitialLoadedRef.current = true;
      } catch (err) {
        console.error('Failed to load artifact context for', loadedArtId, err);
      }
    } else {
      setContext({
        artifact: {
          id: '',
          artifact_type: 'note',
          authorship: 'user',
          title: 'Untitled Note',
          content_md: '# Untitled Note\n\n',
          content_hash: '',
          version: 1,
          is_unfinished_draft: false,
          created_at: Date.now(),
          updated_at: Date.now(),
        },
        epistemic_nodes: [],
        pending_proposals: [],
        project_id: projectId || null,
        is_unfinished_draft: false,
      });
      setEditorContent('# Untitled Note\n\n');
      setTitle('Untitled Note');
      lastSavedContentRef.current = '# Untitled Note\n\n';
      lastSavedTitleRef.current = 'Untitled Note';
      isInitialLoadedRef.current = true;
    }

    if (projectId && projectId !== 'default_project') {
      try {
        if (!brief) {
          const b = await getResumeBrief(projectId, 1);
          setBrief(b);
        }
        const h = await getProjectHistory(projectId);
        setHistory(h);
      } catch (err) {
        console.warn('Auxiliary project history load warning:', err);
      }
    } else {
      setBrief(null);
      setHistory(null);
    }
  };

  const handleSave = useCallback(async () => {
    const artId = context?.artifact?.id || artifactId || '';
    const currentT = currentTitleRef.current.trim() || context?.artifact?.title || null;
    const currentC = currentContentRef.current;

    setIsSaving(true);
    setSaveSuccess(false);
    try {
      const targetProjectId = projectId && projectId !== 'default_project' ? projectId : null;
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
    } catch (err) {
      console.error('Failed to save artifact', err);
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

    setIsSaving(true);
    autoSaveTimerRef.current = setTimeout(() => {
      handleSave();
    }, 600);

    return () => {
      if (autoSaveTimerRef.current) {
        clearTimeout(autoSaveTimerRef.current);
      }
    };
  }, [editorContent, title, handleSave]);

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
          ).catch((e) => console.warn('Unmount auto-save failed', e));
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

  // Slash Command Selection Handler
  const handleSelectSlashCommand = async (item: SlashCommandItem) => {
    setSlashMenu((prev) => ({ ...prev, isOpen: false }));

    if (item.isObjectAction && item.objectType) {
      setInlineObjectCreator({
        isOpen: true,
        objectType: item.objectType,
        position: slashMenu.position,
        caretIndex: slashMenu.caretIndex,
      });
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
      // 1. Create backing artifact and save object instance
      const created = await createTypedObject(objectType.id, objTitle, properties, projectId);

      // 2. Insert embedded object directive into document markdown
      const directive = `\n\`\`\`sthira-object\n${JSON.stringify(
        {
          artifact_id: created.artifact_id,
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
      await handleSave();
    }
    onBack();
  };

  const handleExpandReview = async () => {
    if (!brief || !projectId || projectId === 'default_project') return;
    try {
      const nextLevel = isExpandedReview ? 1 : 2;
      const b = await getResumeBrief(projectId, nextLevel);
      setBrief(b);
      setIsExpandedReview(!isExpandedReview);
    } catch (err) {
      console.error('Failed to toggle expanded review', err);
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

  return (
    <div className="flex-1 h-full overflow-hidden bg-[#FAF8F3] dark:bg-[#121310] text-[#1E201C] dark:text-[#EDEAE2] flex flex-col font-sans">
      {/* Context-Aware Top Navigation Header */}
      <header className="w-full bg-[#FFFFFF] dark:bg-[#161714] border-b border-[#E8E5DC] dark:border-[#2C2E27] px-6 py-3 flex items-center justify-between shrink-0 z-40">
        {/* Left: Breadcrumbs */}
        <div className="flex items-center gap-2 min-w-0">
          <button
            type="button"
            onClick={handleBack}
            className="inline-flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl bg-[#EFECE4] dark:bg-[#22241F] hover:bg-[#E5E1D6] dark:hover:bg-[#2C2E27] text-xs font-semibold text-[#1E201C] dark:text-[#EDEAE2] transition-colors cursor-pointer shrink-0"
            title="Return to Home"
          >
            <ArrowLeft className="w-3.5 h-3.5" />
            <span className="hidden sm:inline">Home</span>
          </button>

          <span className="text-[#8C8A82] dark:text-[#6E6D66] text-xs font-medium">/</span>

          <span className="text-xs font-semibold text-[#6E6D66] dark:text-[#9E9C93] hover:text-[#1E201C] dark:hover:text-[#EDEAE2] transition-colors truncate max-w-40 sm:max-w-64">
            {brief?.purpose ? brief.purpose.slice(0, 36) : (projectId ? 'Project Workspace' : 'Notes')}
          </span>

          <span className="text-[#8C8A82] dark:text-[#6E6D66] text-xs font-medium">/</span>

          <span className="text-xs font-bold text-[#1E201C] dark:text-[#EDEAE2] truncate max-w-48 sm:max-w-80">
            {title || context?.artifact.title || 'Untitled Note'}
          </span>
        </div>

        {/* Right Controls */}
        <div className="flex items-center gap-2">
          {/* Search Trigger */}
          <button
            type="button"
            onClick={onOpenSearch}
            className="hidden md:flex items-center gap-2 px-3 py-1.5 rounded-xl bg-[#EFECE4] dark:bg-[#22241F] hover:bg-[#E5E1D6] dark:hover:bg-[#2C2E27] text-xs text-[#6E6D66] dark:text-[#9E9C93] transition-colors cursor-pointer border border-[#E8E5DC]/80 dark:border-[#2C2E27]"
            title="Command Palette (Cmd+K)"
          >
            <Search className="w-3.5 h-3.5" />
            <span>Search...</span>
            <kbd className="text-[10px] bg-white/70 dark:bg-[#1C1D19] px-1.5 py-0.5 rounded text-[#3A3B36] dark:text-[#D4D1C7] font-mono border border-[#E8E5DC] dark:border-[#2C2E27]">
              ⌘K
            </kbd>
          </button>

          {/* Canvas Button */}
          <button
            type="button"
            onClick={() => onOpenCanvas()}
            className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-xl bg-[#EFECE4] dark:bg-[#22241F] hover:bg-[#E5E1D6] dark:hover:bg-[#2C2E27] text-xs font-medium text-[#3A3B36] dark:text-[#D4D1C7] transition-colors cursor-pointer"
          >
            <LayoutGrid className="w-3.5 h-3.5" />
            <span className="hidden sm:inline">Canvas</span>
          </button>

          {/* History Button */}
          <button
            type="button"
            onClick={() => setIsHistoryOpen(true)}
            className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-xl bg-[#EFECE4] dark:bg-[#22241F] hover:bg-[#E5E1D6] dark:hover:bg-[#2C2E27] text-xs font-medium text-[#3A3B36] dark:text-[#D4D1C7] transition-colors cursor-pointer"
          >
            <History className="w-3.5 h-3.5" />
            <span className="hidden sm:inline">History</span>
          </button>

          {/* Epistemic Margin Toggle */}
          <button
            type="button"
            data-testid="btn-toggle-epistemic-margin"
            onClick={() => setIsMarginOpen(!isMarginOpen)}
            className={`inline-flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-xs font-medium transition-colors cursor-pointer ${
              isMarginOpen
                ? 'bg-[#282C25] text-white dark:bg-[#EDEAE2] dark:text-[#121310] shadow-2xs'
                : 'bg-[#EFECE4] dark:bg-[#22241F] hover:bg-[#E5E1D6] dark:hover:bg-[#2C2E27] text-[#3A3B36] dark:text-[#D4D1C7]'
            }`}
            title="Toggle Epistemic Margin Sidebar"
          >
            {isMarginOpen ? (
              <PanelRightClose className="w-3.5 h-3.5" />
            ) : (
              <PanelRightOpen className="w-3.5 h-3.5" />
            )}
            <span className="hidden sm:inline">Margin</span>
          </button>

          {/* Theme Mode Toggle */}
          <ThemeToggle variant="icon" />

          {/* Auto-Save Status Pill */}
          <button
            type="button"
            data-testid="btn-save-artifact"
            onClick={handleSave}
            title="Auto-saving enabled. Click to sync immediately."
            className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-xl bg-[#FAF9F5] dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] text-xs font-medium transition-all cursor-pointer shadow-2xs hover:bg-[#EFECE4] dark:hover:bg-[#252820]"
          >
            {isSaving ? (
              <>
                <Loader2 className="w-3.5 h-3.5 animate-spin text-[#8C8A82] dark:text-[#6E6D66]" />
                <span className="text-[#8C8A82] dark:text-[#6E6D66]">Saving...</span>
              </>
            ) : saveSuccess ? (
              <>
                <Check className="w-3.5 h-3.5 text-emerald-600 dark:text-emerald-400" />
                <span className="text-emerald-700 dark:text-emerald-300 font-semibold">Saved</span>
              </>
            ) : (
              <>
                <Check className="w-3.5 h-3.5 text-[#8C8A82] dark:text-[#6E6D66]" />
                <span className="text-[#6E6D66] dark:text-[#9E9C93]">Auto-saved</span>
              </>
            )}
          </button>
        </div>
      </header>

      {/* Main Full-Height Workspace Body */}
      <div className="flex-1 flex overflow-hidden w-full relative">
        {/* Generous Full-Height Document Canvas */}
        <div className="flex-1 h-full overflow-y-auto bg-[#FAF8F3] dark:bg-[#121310]">
          <div className="max-w-4xl lg:max-w-5xl mx-auto px-12 py-10 min-h-full bg-white dark:bg-[#181915] border-x border-[#E8E5DC] dark:border-[#262822] flex flex-col shadow-2xs relative">
            {/* Contextual Resume Brief */}
            {brief && (
              <div className="mb-6">
                <ContextualResumePane
                  brief={brief}
                  isOpen={isBriefOpen}
                  onToggleOpen={() => setIsBriefOpen(!isBriefOpen)}
                  onExpandReview={handleExpandReview}
                  isExpandedReview={isExpandedReview}
                  onSelectArtifact={(aid) => {
                    getArtifactContext(aid).then((c) => {
                      setContext(c);
                      setEditorContent(c.artifact.content_md);
                      setTitle(c.artifact.title || '');
                    });
                  }}
                />
              </div>
            )}

            {/* Anchored Quote Preview */}
            {hoveredQuote && (
              <div className="mb-4 bg-[#FAF8F3] dark:bg-[#1C261A] border border-[#CCDCCC] dark:border-[#2E4A28] rounded-xl px-4 py-2.5 text-xs text-[#2D5A27] dark:text-[#88D080] font-medium flex items-center gap-2 animate-in fade-in">
                <Quote className="w-3.5 h-3.5 shrink-0" />
                <span className="truncate">Anchored Quote: &ldquo;{hoveredQuote}&rdquo;</span>
              </div>
            )}

            {/* Document Title Header */}
            <div className="pb-3 mb-4 border-b border-[#ECE9E0] dark:border-[#262822]">
              <input
                type="text"
                data-testid="input-artifact-title"
                value={title}
                onChange={(e) => handleTitleChange(e.target.value)}
                onBlur={handleSave}
                placeholder="Document title..."
                className="text-2xl sm:text-3xl font-bold tracking-tight text-[#1E201C] dark:text-[#EDEAE2] bg-transparent border-none outline-none w-full placeholder-[#A09D94] dark:placeholder-[#6E6D66]"
              />
            </div>

            {/* Notion-Style Formatting Toolbar & Rich Preview Toggle */}
            <div className="flex items-center justify-between pb-3 mb-5 border-b border-[#ECE9E0] dark:border-[#262822] text-xs text-[#6E6D66] dark:text-[#9E9C93]">
              {/* Left: Quick Formatting Actions */}
              <div className="flex items-center gap-1 flex-wrap">
                <button
                  type="button"
                  onClick={() => handleApplyFormat('bold')}
                  className="p-1.5 rounded-lg hover:bg-[#EFECE4] dark:hover:bg-[#252820] hover:text-[#1E201C] dark:hover:text-[#EDEAE2] transition-colors cursor-pointer"
                  title="Bold (Cmd+B)"
                >
                  <Bold className="w-3.5 h-3.5" />
                </button>
                <button
                  type="button"
                  onClick={() => handleApplyFormat('italic')}
                  className="p-1.5 rounded-lg hover:bg-[#EFECE4] dark:hover:bg-[#252820] hover:text-[#1E201C] dark:hover:text-[#EDEAE2] transition-colors cursor-pointer"
                  title="Italic (Cmd+I)"
                >
                  <Italic className="w-3.5 h-3.5" />
                </button>
                <div className="w-px h-4 bg-[#E0DCD3] dark:bg-[#2C2E27] mx-1" />
                <button
                  type="button"
                  onClick={() => handleApplyFormat('h1')}
                  className="px-2 py-1 rounded-lg hover:bg-[#EFECE4] dark:hover:bg-[#252820] hover:text-[#1E201C] dark:hover:text-[#EDEAE2] font-semibold text-xs transition-colors cursor-pointer"
                  title="Heading 1"
                >
                  H1
                </button>
                <button
                  type="button"
                  onClick={() => handleApplyFormat('h2')}
                  className="px-2 py-1 rounded-lg hover:bg-[#EFECE4] dark:hover:bg-[#252820] hover:text-[#1E201C] dark:hover:text-[#EDEAE2] font-semibold text-xs transition-colors cursor-pointer"
                  title="Heading 2"
                >
                  H2
                </button>
                <button
                  type="button"
                  onClick={() => handleApplyFormat('todo')}
                  className="p-1.5 rounded-lg hover:bg-[#EFECE4] dark:hover:bg-[#252820] hover:text-[#1E201C] dark:hover:text-[#EDEAE2] transition-colors cursor-pointer"
                  title="Checklist Item"
                >
                  <CheckSquare className="w-3.5 h-3.5" />
                </button>
                <button
                  type="button"
                  onClick={() => handleApplyFormat('code')}
                  className="p-1.5 rounded-lg hover:bg-[#EFECE4] dark:hover:bg-[#252820] hover:text-[#1E201C] dark:hover:text-[#EDEAE2] transition-colors cursor-pointer"
                  title="Code Block"
                >
                  <CodeIcon className="w-3.5 h-3.5" />
                </button>
                <button
                  type="button"
                  onClick={() => handleApplyFormat('table')}
                  className="p-1.5 rounded-lg hover:bg-[#EFECE4] dark:hover:bg-[#252820] hover:text-[#1E201C] dark:hover:text-[#EDEAE2] transition-colors cursor-pointer"
                  title="Markdown Table"
                >
                  <TableIcon className="w-3.5 h-3.5" />
                </button>

                <div className="w-px h-4 bg-[#E0DCD3] dark:bg-[#2C2E27] mx-1" />

                <button
                  type="button"
                  onClick={() => handleApplyFormat('slash')}
                  className="inline-flex items-center gap-1 px-2.5 py-1 rounded-lg bg-[#EFECE4] dark:bg-[#22241F] hover:bg-[#E5E1D6] dark:hover:bg-[#2C2E27] text-[#282C25] dark:text-[#EDEAE2] font-medium transition-colors cursor-pointer"
                  title="Open Slash Command Popover"
                >
                  <span className="font-mono font-bold">/</span>
                  <span>Insert Block</span>
                </button>
              </div>

              {/* Right: Clean Binary Mode Switcher: [ ✏️ Edit ] ⟷ [ 👁️ Preview ] */}
              <div className="inline-flex p-0.5 rounded-lg bg-[#EFECE4] dark:bg-[#22241F] border border-[#E0DCD3] dark:border-[#2C2E27]">
                <button
                  type="button"
                  data-testid="btn-toggle-edit-mode"
                  onClick={() => setViewMode('edit')}
                  className={`inline-flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-medium transition-all cursor-pointer ${
                    viewMode === 'edit'
                      ? 'bg-white dark:bg-[#1C1D19] text-[#1E201C] dark:text-[#EDEAE2] font-semibold shadow-2xs'
                      : 'text-[#6E6D66] dark:text-[#9E9C93] hover:text-[#1E201C] dark:hover:text-[#EDEAE2]'
                  }`}
                  title="Edit Markdown (Cmd+E)"
                >
                  <Edit3 className="w-3.5 h-3.5" />
                  <span>✏️ Edit</span>
                </button>

                <button
                  type="button"
                  data-testid="btn-toggle-preview-mode"
                  onClick={() => setViewMode('preview')}
                  className={`inline-flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-medium transition-all cursor-pointer ${
                    viewMode === 'preview'
                      ? 'bg-white dark:bg-[#1C1D19] text-[#1E201C] dark:text-[#EDEAE2] font-semibold shadow-2xs'
                      : 'text-[#6E6D66] dark:text-[#9E9C93] hover:text-[#1E201C] dark:hover:text-[#EDEAE2]'
                  }`}
                  title="Component-Decorated Rich Workspace (Cmd+E)"
                >
                  <Eye className="w-3.5 h-3.5" />
                  <span>👁️ Preview</span>
                </button>
              </div>
            </div>

            {/* Inline AI Streaming Status Banner */}
            {isAiStreaming && (
              <div className="mb-4 p-3 rounded-xl bg-[#F6F4FF] dark:bg-[#1C162E] border border-[#E0D7FA] dark:border-[#382E59] text-xs text-purple-900 dark:text-purple-200 flex items-center justify-between animate-pulse">
                <div className="flex items-center gap-2">
                  <Sparkles className="w-4 h-4 text-purple-600 dark:text-purple-400 animate-spin" />
                  <span className="font-semibold">{aiStreamingMessage || 'Generating with active model...'}</span>
                </div>
                <span className="text-[10px] text-purple-700 dark:text-purple-300 font-mono">streaming tokens</span>
              </div>
            )}

            {/* Main Canvas Body: Live Edit vs Component-Decorated Rich Preview */}
            <div className="flex-1 flex flex-col min-h-[520px] relative">
              {viewMode === 'edit' ? (
                <div className="flex-1 flex flex-col relative">
                  <textarea
                    ref={textareaRef}
                    data-testid="textarea-editor"
                    value={editorContent}
                    onChange={(e) => handleEditorChange(e.target.value, e)}
                    onBlur={handleSave}
                    placeholder="Type '/' to insert blocks, record epistemic decisions, or invoke inline AI compute..."
                    className="flex-1 w-full bg-transparent border-none outline-none resize-none font-mono text-sm leading-relaxed text-[#1E201C] dark:text-[#EDEAE2] placeholder-[#A09D94] dark:placeholder-[#6E6D66] min-h-[calc(100vh-140px)]"
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
                <div data-testid="rich-preview-canvas" className="flex-1 py-2 space-y-4">
                  <RichDocumentRenderer
                    content={editorContent}
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
          </div>
        </div>

        {/* Right Docked Collapsible Sidebar: Epistemic Margin */}
        {isMarginOpen && (
          <aside
            data-testid="epistemic-margin-sidebar"
            className="w-80 shrink-0 border-l border-[#E8E5DC] dark:border-[#262822] bg-[#FAF9F6] dark:bg-[#141512] h-full overflow-y-auto p-5 flex flex-col space-y-5 animate-in slide-in-from-right duration-150"
          >
            <div className="flex items-center justify-between pb-3 border-b border-[#E8E5DC] dark:border-[#262822]">
              <div className="flex items-center gap-2">
                <Quote className="w-4 h-4 text-[#282C25] dark:text-[#EDEAE2]" />
                <h3 className="text-xs font-bold text-[#282C25] dark:text-[#EDEAE2] uppercase tracking-wider">
                  Epistemic Margin
                </h3>
              </div>
              <div className="flex items-center gap-1.5">
                <span className="text-[10px] px-2 py-0.5 rounded-full bg-[#EFECE4] dark:bg-[#22241F] text-[#6E6D66] dark:text-[#9E9C93] font-bold">
                  {context?.epistemic_nodes?.length || 0}
                </span>
                <button
                  type="button"
                  onClick={() => setIsMarginOpen(false)}
                  className="text-[#8C8A82] dark:text-[#6E6D66] hover:text-[#1E201C] dark:hover:text-[#EDEAE2] p-1 rounded-lg hover:bg-[#EFECE4] dark:hover:bg-[#252820] transition-colors"
                  title="Collapse Sidebar"
                >
                  <X className="w-3.5 h-3.5" />
                </button>
              </div>
            </div>

            {/* Grounded Epistemic Nodes */}
            {context?.epistemic_nodes && context.epistemic_nodes.length > 0 ? (
              <div className="space-y-3">
                {context.epistemic_nodes.map((node) => (
                  <div
                    key={node.id}
                    onMouseEnter={() => setHoveredQuote(node.quote || null)}
                    onMouseLeave={() => setHoveredQuote(null)}
                    className="bg-[#FFFFFF] dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-xl p-3 text-xs hover:border-[#D8D4C8] dark:hover:border-[#3E4237] hover:shadow-xs transition-all"
                  >
                    <div className="flex items-center justify-between mb-1.5">
                      <EpistemicBadge origin={node.origin} />
                      <span className="text-[10px] text-[#6E6D66] dark:text-[#9E9C93] font-medium capitalize">
                        {node.node_type}
                      </span>
                    </div>
                    <p className="font-semibold text-[#1E201C] dark:text-[#EDEAE2] mb-1">
                      {node.statement}
                    </p>
                    {node.quote && (
                      <p className="text-[11px] text-[#6E6D66] dark:text-[#9E9C93] italic bg-[#FAF8F3] dark:bg-[#161714] p-2 rounded-lg border border-[#ECE9E0] dark:border-[#262822] mt-1.5">
                        &ldquo;{node.quote}&rdquo;
                      </p>
                    )}
                  </div>
                ))}
              </div>
            ) : (
              <div className="text-xs text-[#6E6D66] dark:text-[#9E9C93] leading-relaxed bg-[#FFFFFF] dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-xl p-4 text-center">
                <Info className="w-4 h-4 mx-auto mb-2 text-[#8C8A82] dark:text-[#6E6D66]" />
                Type <span className="font-mono font-semibold text-[#1E201C] dark:text-[#EDEAE2]">/decision</span> or write notes to register verified decisions in the knowledge graph.
              </div>
            )}

            {/* Ambient Graph Proposals */}
            {context?.pending_proposals && context.pending_proposals.length > 0 && (
              <div className="pt-2 border-t border-[#E8E5DC] dark:border-[#262822] space-y-3">
                <h4 className="text-xs font-bold text-[#6E6D66] dark:text-[#9E9C93] uppercase tracking-wider flex items-center gap-1.5">
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
          </aside>
        )}
      </div>

      {/* History Drawer */}
      {history && (
        <ProjectHistoryDrawer
          history={history}
          isOpen={isHistoryOpen}
          onClose={() => setIsHistoryOpen(false)}
          onSelectArtifact={(aid) => {
            getArtifactContext(aid).then((c) => {
              setContext(c);
              setEditorContent(c.artifact.content_md);
              setIsHistoryOpen(false);
            });
          }}
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
    </div>
  );
};

