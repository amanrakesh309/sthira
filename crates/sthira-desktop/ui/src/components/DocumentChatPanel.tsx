import { useWorkflowSettings } from '../hooks/useWorkflowSettings';
import { openWorkflowSettings } from '../api/workflowSettings';
import React, { useEffect, useRef, useState } from 'react';
import { ArrowUp, MessageSquare, X, Loader2 } from 'lucide-react';
import ReactMarkdown from 'react-markdown';
import { DocumentEditReview } from './DocumentEditReview';
import { decideDocumentEdit, discussDocument, getDocumentConversation, selectDocumentConversationModel, type Conversation, type ConversationTurn } from '../api/conversation';
import type { Artifact, ProviderEntry } from '../types/sthira';
import { getProviderCatalog } from '../api/ipc';

interface Props {
  sessionNamespace?: string;
  artifactId: string; initialConversation: Conversation; open: boolean; busy: boolean;
  modelId?: string; modelName: string; currentContent: string;
  onClose: () => void; onApplyingChange: (applying: boolean) => void; beforeRequest: () => Promise<boolean>;
  onApplied: (artifact: Artifact, changeSetId: string | null) => void;
}
export function DocumentChatPanel({ sessionNamespace, artifactId, initialConversation, open, busy, modelId, modelName, currentContent, onClose, onApplyingChange, beforeRequest, onApplied }: Props) {
  const workflows = useWorkflowSettings();
  const chatEnabled = !workflows.loading && !workflows.error && workflows.settings.chat.enabled;
  const normalize = (text: string) => text.replace(/^\uFEFF/, '').replace(/\r\n?/g, '\n').replace(/\n+$/, '').normalize('NFC');
  const [conversation, setConversation] = useState(initialConversation);
  const [catalog, setCatalog] = useState<ProviderEntry[]>([]);
  const [catalogLoading, setCatalogLoading] = useState(true);
  const [catalogError, setCatalogError] = useState<string | null>(null);
  const [catalogReload, setCatalogReload] = useState(0);
  const [changingModel, setChangingModel] = useState(false);
  const [draft, setDraft] = useState(() => { try { return sessionNamespace ? sessionStorage.getItem(`sthira.chat-draft:${sessionNamespace}:${artifactId}`) || '' : ''; } catch { return ''; } });
  useEffect(() => { if(sessionNamespace) try { const saved=sessionStorage.getItem(`sthira.chat-draft:${sessionNamespace}:${artifactId}`); if(saved)setDraft(current => current || saved); } catch { /* Keep the current draft. */ } }, [sessionNamespace, artifactId]);
  const updateDraft = (value: string) => {
    setDraft(value);
    if(sessionNamespace) try {const key=`sthira.chat-draft:${sessionNamespace}:${artifactId}`;if(value)sessionStorage.setItem(key,value);else sessionStorage.removeItem(key);} catch { /* Keep the current draft. */ }
  };
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [review, setReview] = useState<ConversationTurn | null>(null);
  const [proposeEdit, setProposeEdit] = useState(false);
  const alive = useRef(true);
  const running = useRef(false);
  const mutation = useRef(0);
  const input = useRef<HTMLTextAreaElement>(null);
  const end = useRef<HTMLDivElement>(null);
  useEffect(() => { setConversation(initialConversation); }, [initialConversation]);
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
  useEffect(() => { if (open) input.current?.focus(); }, [open]);
  useEffect(() => { if (open) end.current?.scrollIntoView?.({ block: 'nearest' }); }, [conversation.turns.length, pending, open]);
  const refresh = async () => {
    const version = mutation.current;
    const next = await getDocumentConversation(artifactId);
    if (alive.current && next && !running.current && version === mutation.current) setConversation(next);
  };
  useEffect(() => {
    if (!open) return;
    void refresh().catch(e => { if (alive.current) setError(String(e)); });
    const timer = conversation.turns.some(turn => !turn.reply && !turn.error)
      ? window.setInterval(() => { void refresh().catch(() => {}); }, 3000) : undefined;
    return () => { if (timer) window.clearInterval(timer); };
  }, [open, conversation.turns.some(turn => !turn.reply && !turn.error)]);
  useEffect(() => {
    if (!open) return;
    let active = true;
    setCatalogLoading(true); setCatalogError(null);
    getProviderCatalog().then(value => { if (active) setCatalog(value); })
      .catch(() => { if (active) setCatalogError('Couldn’t load available models.'); })
      .finally(() => { if (active) setCatalogLoading(false); });
    return () => { active = false; };
  }, [open, catalogReload]);
  useEffect(() => {
    const refresh = () => setCatalogReload(value => value + 1);
    window.addEventListener('sthira:catalog-changed', refresh);
    return () => window.removeEventListener('sthira:catalog-changed', refresh);
  }, []);
  const availableProviders = catalog.filter(provider => provider.is_enabled);
  const availableModels = availableProviders.flatMap(provider => provider.models.filter(model => model.is_enabled));
  const defaultModel = availableModels.find(model => model.is_default) || availableModels[0];
  const selectedId = conversation.model_id || workflows.settings.chat.model_id || modelId || defaultModel?.id || '';
  const selectedModel = availableModels.find(model => model.id === selectedId);
  const selectedName = selectedModel?.display_name || selectedModel?.model_identifier || conversation.model_name || modelName;
  const unavailable = !catalogLoading && !catalogError && !!selectedId && !selectedModel;
  const canSend = chatEnabled && !pending && !changingModel && !busy && !catalogLoading && !!selectedId && !unavailable;
  const chooseModel = async (nextId: string) => {
    if (running.current || busy || nextId === selectedId) return;
    running.current = true; mutation.current += 1;
    setChangingModel(true); setError(null);
    try {
      const next = await selectDocumentConversationModel(artifactId, nextId);
      if (alive.current) setConversation(next);
    } catch (error) { if (alive.current) setError(String(error)); }
    finally { mutation.current += 1; running.current = false; if (alive.current) setChangingModel(false); }
  };
  const send = async (event?: React.FormEvent) => {
    event?.preventDefault();
    if (!draft.trim() || running.current || !canSend) return;
    running.current = true; mutation.current += 1; setPending(true); setError(null);
    const message = draft.trim();
    try {
      if (!(await beforeRequest()) || !alive.current) return;
      const next = await discussDocument(artifactId, message, selectedId, proposeEdit);
      if (!alive.current) return;
      setConversation(next);
      // Keep unsent text when the provider failed; the attempted message is durable too.
      if (!next.turns.at(-1)?.error) { setDraft(current => { if(current.trim() !== message)return current; if(sessionNamespace)try{sessionStorage.removeItem(`sthira.chat-draft:${sessionNamespace}:${artifactId}`);}catch{} return ''; }); }
    } catch (e) { if (alive.current) setError(String(e)); }
    finally { mutation.current += 1; running.current = false; if (alive.current) setPending(false); }
  };
  const decide = async (turn: ConversationTurn, apply: boolean) => {
    if (running.current || busy) return;
    running.current = true; mutation.current += 1; setPending(true); setError(null);
    if (apply) onApplyingChange(true);
    try {
      if (apply && (!(await beforeRequest()) || !alive.current)) return;
      const artifact = await decideDocumentEdit(artifactId, turn.id, apply);
      if (!alive.current) return;
      if (apply) onApplied(artifact, null);
      const next = await getDocumentConversation(artifactId);
      if (!alive.current) return;
      if (apply) onApplied(artifact, next?.turns.find(t => t.id === turn.id)?.change_set_id || null);
      if (next) setConversation(next);
      setReview(null);
    } catch (e) { if (alive.current) setError(String(e)); }
    finally { mutation.current += 1; running.current = false; if (alive.current) { setPending(false); onApplyingChange(false); } }
  };
  return <aside id="document-chat" className="document-chat" aria-label="Document conversation" hidden={!open}
    onKeyDown={e => { if (e.key === 'Escape') { e.stopPropagation(); if (review) setReview(null); else onClose(); } }}>
    <header className="document-chat-header"><div><MessageSquare size={17} /><h2>Conversation</h2></div>
      <button type="button" className="focus-icon-button" aria-label="Close conversation" onClick={onClose}><X size={18} /></button></header>
    <p className="document-chat-context">{initialConversation.mode === 'document' ? 'Document' : initialConversation.mode === 'plan' ? 'Plan' : 'Research'}<br />Discuss this document. Edits are yours to approve.</p>
    {!chatEnabled && <p className="workflow-setup-notice" role="status">{workflows.loading ? 'Loading chat settings…' : workflows.error ? 'Chat settings are unavailable.' : 'Document chat is disabled. Your conversation remains saved.'} <button type="button" onClick={openWorkflowSettings}>Open Settings</button></p>}
    <div className="document-chat-model-control">
      <label htmlFor="document-chat-model">Model</label>
      <select id="document-chat-model" aria-label="Conversation model" title={selectedName} value={selectedId}
        disabled={pending || changingModel || busy || catalogLoading || availableModels.length === 0}
        onChange={event => void chooseModel(event.target.value)}>
        {!selectedId && <option value="">{catalogLoading ? 'Loading models…' : 'Choose a model'}</option>}
        {selectedId && !selectedModel && <option value={selectedId}>{selectedName}{unavailable ? ' (unavailable)' : ''}</option>}
        {availableProviders.map(provider => <optgroup key={provider.id} label={provider.display_name}>
          {provider.models.filter(model => model.is_enabled).map(model => <option key={model.id} value={model.id}>{model.display_name || model.model_identifier}</option>)}
        </optgroup>)}
      </select>
      {changingModel && <span role="status">Saving model…</span>}
      {catalogError && <p role="status">{catalogError} <button type="button" onClick={() => setCatalogReload(value => value + 1)}>Retry models</button></p>}
      {unavailable && <p role="status">This model is unavailable. Choose an enabled model to continue.</p>}
      {!catalogLoading && !catalogError && availableModels.length === 0 && <p role="status">Enable a model in Settings to continue.</p>}
    </div>
    {error && <div role="alert" className="document-chat-error">{error}<button type="button" onClick={() => { void refresh().catch(e => setError(String(e))); }}>Reload conversation</button></div>}
      <div className="document-chat-messages" role="log" aria-label="Conversation messages" aria-live="polite">
        {conversation.turns.length === 0 && <div className="document-chat-empty"><h3>Think it through.</h3><p>Ask a follow-up, explore an alternative, or request an edit to this document.</p></div>}
        {conversation.initial_prompt && <details className="document-chat-goal"><summary>Original goal</summary><p>{conversation.initial_prompt}</p></details>}
        {conversation.turns.map(turn => <article className="document-chat-turn" key={turn.id}>
          <div className="document-chat-user"><span>You</span><p>{turn.user_text}</p></div>
          <div className="document-chat-assistant"><span className="document-chat-model-attribution">Sthira · {turn.model_name || 'Model not recorded'}{turn.provider_name ? ` · ${turn.provider_name}` : ''}</span>
            {turn.reply ? <ReactMarkdown>{turn.reply}</ReactMarkdown> : <p className="document-chat-error">{turn.error || 'No reply saved yet. If this request was interrupted, you can send it again.'}</p>}
            {turn.proposed_content_md && turn.proposal_state === 'pending' && <button type="button" className="document-chat-review-button" onClick={() => setReview(turn)}>Review suggested edit</button>}
            {turn.proposed_content_md && turn.proposal_state !== 'pending' && <p className="document-chat-decision">{turn.proposal_state === 'applied' ? 'Edit applied' : turn.proposal_state === 'reverted' ? 'Edit undone' : 'Suggestion dismissed'}</p>}
          </div>
        </article>)}
        {pending && <p className="document-chat-pending" role="status"><Loader2 size={16} className="animate-spin" /> Thinking…</p>}
        <div ref={end} />
      </div>
      <form className="document-chat-compose" onSubmit={send}>
        <label className="document-chat-edit-option"><input type="checkbox" checked={proposeEdit} onChange={e => setProposeEdit(e.target.checked)} disabled={pending} /> Suggest a document edit</label>
        <label className="sr-only" htmlFor="document-chat-input">Message about this document</label>
        <textarea ref={input} id="document-chat-input" rows={3} maxLength={8000} value={draft} onChange={e => updateDraft(e.target.value)} placeholder="Ask a follow-up…" disabled={pending}
          onKeyDown={e => { if (e.key === 'Enter' && !e.shiftKey && !e.nativeEvent.isComposing) { e.preventDefault(); void send(); } }} />
        <div><span>{busy ? 'Available when generation finishes.' : 'Enter to send · Shift+Enter for a new line'}</span><button type="submit" className="sthira-button-primary" disabled={!canSend || !draft.trim()} aria-label="Send message"><ArrowUp size={17} /></button></div>
      </form>
    {review && open && <DocumentEditReview current={currentContent} proposed={review.proposed_content_md || ''} summary={review.reply || ''}
      stale={normalize(currentContent) !== normalize(review.basis_content_md)} pending={pending} busy={busy} error={error}
      onClose={() => setReview(null)} onApply={() => void decide(review, true)} onDismiss={() => void decide(review, false)} />}
  </aside>;
}
