import { useState } from 'react';
import { ResumeSettingsPanel } from './ResumeSettingsPanel';
import { useWorkflowSettings } from '../hooks/useWorkflowSettings';
import {
  saveWorkflowSetting,
  getEnabledModels,
  getWorkspaceDefaultModel,
  resolveWorkflowModel,
  type WorkflowId,
  type WorkflowSetting,
} from '../api/workflowSettings';
import type { ProviderEntry } from '../types/sthira';
import { Sparkles, CheckCircle2, AlertTriangle, ArrowUpRight, ShieldCheck } from 'lucide-react';

const workflows = [
  {
    id: 'research',
    name: 'Research',
    description: 'Investigate a question using saved context and optional web sources.',
    purpose: 'Unlocks Research under Explore & Plan in the sidebar and in Universal Input.',
  },
  {
    id: 'plan',
    name: 'Plan',
    description: 'Turn a goal into steps, milestones, and open decisions.',
    purpose: 'Unlocks Plan under Explore & Plan in the sidebar and in Universal Input.',
  },
  {
    id: 'draft',
    name: 'Draft',
    description: 'Create a deliverable with AI, such as a report or invitation.',
    purpose: 'Unlocks AI drafting mode in the Universal Input Bar.',
  },
  {
    id: 'chat',
    name: 'Document chat',
    description: 'Discuss a saved document and review suggested changes before applying.',
    purpose: 'Unlocks conversational sidebar review in the document editor.',
  },
] as const;

export interface WorkflowSettingsPanelProps {
  providers: ProviderEntry[];
  onNavigateTab?: (tab: string) => void;
}

export function WorkflowSettingsPanel({ providers, onNavigateTab }: WorkflowSettingsPanelProps) {
  const { settings, loading, error, retry } = useWorkflowSettings();
  const [pending, setPending] = useState<WorkflowId | null>(null);
  const [notice, setNotice] = useState('');
  const [saveError, setSaveError] = useState('');
  const models = getEnabledModels(providers);
  const workspaceModel = getWorkspaceDefaultModel(providers);

  const handleNavigateToModels = () => {
    if (onNavigateTab) {
      onNavigateTab('models');
    } else {
      window.dispatchEvent(new CustomEvent('sthira:open-settings', { detail: 'models' }));
    }
  };

  const update = async (id: WorkflowId, patch: Partial<WorkflowSetting>) => {
    if (pending) return;
    setPending(id);
    setSaveError('');
    setNotice('');
    try {
      await saveWorkflowSetting(id, { ...settings[id], ...patch });
      setNotice('Saved');
    } catch (error) {
      setSaveError(String(error));
    } finally {
      setPending(null);
    }
  };

  return (
    <section className="workflow-settings space-y-6">
      <div className="settings-section-header">
        <h3 className="settings-section-title">Workflows</h3>
        <p className="settings-section-sub">
          Choose how AI supports your work. Sthira is local-first: your notes are private and AI models never run in the background without explicit opt-in.
        </p>
      </div>

      {/* Why Toggle & How It Works Guide Banner */}
      <div className="workflow-guide-banner" data-testid="workflow-guide-banner">
        <div className="flex flex-col sm:flex-row items-start justify-between gap-3">
          <div className="space-y-1">
            <h4 className="flex items-center gap-2 text-sm font-semibold text-sthira-text">
              <Sparkles className="w-4 h-4 text-sthira-primary shrink-0" />
              <span>Why Toggle Workflows &amp; How They Work</span>
            </h4>
            <p className="text-xs text-sthira-text-muted leading-relaxed">
              <strong>Local-first privacy:</strong> Sthira never calls AI models automatically. Toggling a workflow <strong>On</strong> unlocks that AI capability in the workspace (such as Explore &amp; Plan in the left sidebar). Toggling it <strong>Off</strong> guarantees that no external network requests or tokens are sent.
            </p>
          </div>
          <button
            type="button"
            data-testid="btn-go-to-models-guide"
            onClick={handleNavigateToModels}
            className="shrink-0 px-3 py-1.5 rounded-lg border border-sthira-border bg-sthira-surface hover:bg-sthira-surface-muted text-xs font-semibold text-sthira-primary transition-colors cursor-pointer flex items-center gap-1.5 self-start"
          >
            <span>AI &amp; Models</span>
            <ArrowUpRight className="w-3.5 h-3.5" />
          </button>
        </div>

        <div className="workflow-guide-steps">
          <div className="workflow-guide-step">
            <span className="text-[10px] font-bold text-sthira-primary tracking-wide uppercase">Step 1</span>
            <strong>Connect a Provider</strong>
            <p>Add and enable Ollama, OpenRouter, Anthropic, Gemini, or OpenAI in AI &amp; Models.</p>
          </div>
          <div className="workflow-guide-step">
            <span className="text-[10px] font-bold text-sthira-primary tracking-wide uppercase">Step 2</span>
            <strong>Toggle Workflow On</strong>
            <p>Turn the switch On for workflows you want available in the left sidebar and input bar.</p>
          </div>
          <div className="workflow-guide-step">
            <span className="text-[10px] font-bold text-sthira-primary tracking-wide uppercase">Step 3</span>
            <strong>Assign Model</strong>
            <p>Select a specific model or inherit your workspace default to achieve Ready status.</p>
          </div>
        </div>
      </div>

      <div className="workflow-capture flex items-center gap-3 py-3 border-b border-sthira-border text-xs text-sthira-text-muted">
        <ShieldCheck className="w-4 h-4 text-emerald-600 dark:text-emerald-400 shrink-0" />
        <div>
          <strong className="text-sthira-text mr-2">Capture &amp; Core Editing</strong>
          <span>Always available · Saves your words locally without needing any model.</span>
        </div>
      </div>

      {error && (
        <p role="alert" className="p-3 rounded-lg bg-red-50 dark:bg-red-950/30 text-red-600 dark:text-red-400 text-xs">
          {error} <button type="button" className="underline ml-2" onClick={retry}>Try again</button>
        </p>
      )}
      {saveError && (
        <p role="alert" className="p-3 rounded-lg bg-red-50 dark:bg-red-950/30 text-red-600 dark:text-red-400 text-xs">
          {saveError}
        </p>
      )}
      <p role="status" className="workflow-save-status text-xs text-sthira-text-muted">
        {loading ? 'Loading preferences…' : pending ? 'Saving…' : notice}
      </p>

      {workflows.map(({ id, name, description, purpose }) => {
        const setting = settings[id];
        const model = resolveWorkflowModel(setting, providers);
        return (
          <article className="workflow-card" key={id} data-testid={`workflow-card-${id}`}>
            <div className="workflow-card-heading">
              <div>
                <h4>{name}</h4>
                <p>{description}</p>
                <p className="text-xs text-sthira-text-subtle mt-0.5">{purpose}</p>
              </div>
              <label className="workflow-switch">
                <input
                  type="checkbox"
                  aria-label={`Enable ${name}`}
                  checked={setting.enabled}
                  disabled={loading || !!error || !!pending}
                  onChange={(event) => void update(id, { enabled: event.target.checked })}
                />
                <span>{setting.enabled ? 'On' : 'Off'}</span>
              </label>
            </div>

            {/* Clear Status & Action Guidance */}
            {!setting.enabled ? (
              <div className="mt-3 pt-3 border-t border-sthira-border/60 flex items-center justify-between text-xs text-sthira-text-muted">
                <span className="flex items-center gap-1.5">
                  <span className="w-2 h-2 rounded-full bg-sthira-text-subtle/50" />
                  <span>Disabled · Turn switch On to make {name} accessible in the sidebar and workspace.</span>
                </span>
                <span className="text-[11px] font-medium text-sthira-text-subtle">No model calls</span>
              </div>
            ) : model ? (
              <div className="mt-3 pt-3 border-t border-sthira-border/60 flex items-center justify-between text-xs">
                <span className="flex items-center gap-1.5 text-emerald-700 dark:text-emerald-400 font-medium">
                  <CheckCircle2 className="w-3.5 h-3.5 shrink-0" />
                  <span>Ready · Active in sidebar &amp; tools using {model.display_name || model.model_identifier}.</span>
                </span>
              </div>
            ) : (
              <div className="mt-3 pt-3 border-t border-sthira-border/60 flex items-center justify-between gap-2 text-xs">
                <span className="flex items-center gap-1.5 text-amber-700 dark:text-amber-400 font-medium">
                  <AlertTriangle className="w-3.5 h-3.5 shrink-0" />
                  <span>Action needed: No enabled model is available. Connect a provider or select an active model below.</span>
                </span>
                <button
                  type="button"
                  data-testid={`btn-configure-provider-${id}`}
                  onClick={handleNavigateToModels}
                  className="text-xs text-sthira-primary font-semibold hover:underline cursor-pointer flex items-center gap-1 shrink-0"
                >
                  <span>Go to AI &amp; Models</span>
                  <ArrowUpRight className="w-3 h-3" />
                </button>
              </div>
            )}

            {!setting.enabled && setting.model_id && !model && (
              <p className="workflow-setup-notice mt-2 text-xs text-amber-600 dark:text-amber-400">
                The saved model is unavailable.{' '}
                <button
                  type="button"
                  disabled={loading || !!error || !!pending}
                  onClick={() => void update(id, { model_id: null })}
                  className="underline"
                >
                  Use workspace default
                </button>
              </p>
            )}

            {setting.enabled && (
              <>
                <div className="workflow-model-row">
                  <label htmlFor={`workflow-model-${id}`}>Default model</label>
                  <select
                    id={`workflow-model-${id}`}
                    value={setting.model_id || ''}
                    disabled={loading || !!error || !!pending}
                    onChange={(event) => void update(id, { model_id: event.target.value || null })}
                  >
                    <option value="">
                      Use workspace default{workspaceModel ? ` · ${workspaceModel.display_name}` : ''}
                    </option>
                    {setting.model_id && !models.some((m) => m.id === setting.model_id) && (
                      <option value={setting.model_id}>Selected model unavailable</option>
                    )}
                    {models.map((m) => (
                      <option key={m.id} value={m.id}>
                        {m.display_name || m.model_identifier}
                      </option>
                    ))}
                  </select>
                  <span className="workflow-readiness">
                    {!setting.enabled
                      ? 'Disabled'
                      : model
                      ? 'Ready'
                      : 'Choose a model in AI & Models'}
                  </span>
                </div>
                {(id === 'research' || id === 'plan') && (
                  <label className="workflow-template-option">
                    <input
                      type="checkbox"
                      aria-label={`Enable ${name} templates`}
                      checked={setting.templates_enabled}
                      disabled={loading || !!error || !!pending}
                      onChange={(event) => void update(id, { templates_enabled: event.target.checked })}
                    />
                    <span>
                      Use templates
                      <small>
                        Choose a reusable structure when starting {name.toLowerCase()}. Template management is coming next.
                      </small>
                    </span>
                  </label>
                )}
              </>
            )}
          </article>
        );
      })}

      <ResumeSettingsPanel providers={providers} />
    </section>
  );
}
