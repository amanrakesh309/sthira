import { defaultResumeSettings } from '../api/resume';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { WorkflowSettingsPanel } from '../components/WorkflowSettingsPanel';
import { UniversalInputBar } from '../components/UniversalInputBar';
import { DocumentChatPanel } from '../components/DocumentChatPanel';
import { FocusView } from '../components/FocusView';
import { defaultWorkflowSettings, saveWorkflowSetting } from '../api/workflowSettings';
import type { ProviderEntry } from '../types/sthira';

vi.unmock('../hooks/useWorkflowSettings');
const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke, Channel: class { onmessage = () => {}; } }));
const providers: ProviderEntry[] = [{ id: 'qa', display_name: 'QA provider', provider_type: 'ollama', base_url: 'http://127.0.0.1:1', has_api_key: false, is_enabled: true, models: [
  { id: 'primary', provider_id: 'qa', display_name: 'Primary model', model_identifier: 'one', is_enabled: true, is_default: true, context_window: 8192, supports_structured_output: true },
  { id: 'secondary', provider_id: 'qa', display_name: 'Secondary model', model_identifier: 'two', is_enabled: true, is_default: false, context_window: 8192, supports_structured_output: true },
] }];
let settings = defaultWorkflowSettings();
beforeEach(() => {
  settings = defaultWorkflowSettings();
  invoke.mockReset().mockImplementation(async (command, args) => {
    if (command === 'cmd_get_resume_settings') return defaultResumeSettings();
    if (command === 'cmd_get_resume_pack_status') return { label: 'Local pack', ready: false, downloading: false, supported: false, download_bytes: 0, notice: null };
    if (command === 'cmd_get_workflow_settings') return structuredClone(settings);
    if (command === 'cmd_save_workflow_setting') { settings[args.workflow as keyof typeof settings] = args.setting; return; }
    if (command === 'cmd_get_provider_catalog') return providers;
    if (command === 'cmd_create_page') return { id: 'captured' };
    if (command === 'cmd_save_artifact') return { id: 'captured', title: args.title, content_md: args.contentMd };
    return [];
  });
});

describe('Workflow settings and readiness', () => {
  it('lets users explicitly clear a removed model before re-enabling a workflow', async () => {
    const user = userEvent.setup();
    settings.research.model_id = 'removed';
    render(<WorkflowSettingsPanel providers={providers} />);
    const reset = await screen.findByRole('button', { name: 'Use workspace default' });
    await user.click(reset);
    await waitFor(() => expect(settings.research.model_id).toBeNull());
    expect(settings.research.enabled).toBe(false);
    await waitFor(() => expect(screen.getByRole('checkbox', { name: 'Enable Research' })).toBeEnabled());
    await user.click(screen.getByRole('checkbox', { name: 'Enable Research' }));
    await waitFor(() => expect(settings.research.enabled).toBe(true));
  });
  it('opens saved project notes without automatically generating an unfinished Resume brief', async () => {
    const original = invoke.getMockImplementation()!;
    invoke.mockImplementation((command, args) => {
      if (command === 'cmd_get_artifact_context') return Promise.resolve({ artifact: { id: 'saved', title: 'Saved note', artifact_type: 'note', authorship: 'user', content_md: '# Saved note\n\nKeep this content.', version: 1, is_unfinished_draft: false, created_at: 0, updated_at: 0 }, project_id: 'qa-project', epistemic_nodes: [], pending_proposals: [], is_unfinished_draft: false });
      if (command === 'cmd_get_project_history') return Promise.resolve({ project_id: 'qa-project', narrative_segments: [], milestones: [] });
      return original(command, args);
    });
    render(<FocusView artifactId="saved" projectId="qa-project" onBack={vi.fn()} onOpenCanvas={vi.fn()} />);
    expect(await screen.findByRole('textbox', { name: 'Document content' })).toHaveTextContent('Keep this content.');
    expect(invoke.mock.calls.some(([command]) => command === 'cmd_get_resume_brief')).toBe(false);
    expect(invoke.mock.calls.some(([command]) => command === 'cmd_get_project_history')).toBe(true);
  });
  it('keeps Capture available when preferences cannot load and blocks an unavailable pinned model', async () => {
    const user = userEvent.setup();
    const original = invoke.getMockImplementation()!;
    invoke.mockImplementation((command, args) => command === 'cmd_get_workflow_settings' ? Promise.reject(new Error('Preferences unavailable')) : original(command, args));
    const research = vi.fn();
    const view = render(<UniversalInputBar onStartResearch={research} />);
    await user.type(screen.getByRole('textbox'), 'Save offline');
    await user.keyboard('{Enter}');
    await waitFor(() => expect(invoke).toHaveBeenCalledWith('cmd_create_page', expect.anything()));
    await user.click(screen.getByTestId('mode-btn-research'));
    expect(screen.getByText('Workflow settings are unavailable.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Submit input' })).toBeDisabled();
    view.unmount();
    settings.research = { enabled: true, model_id: 'removed', templates_enabled: false };
    invoke.mockImplementation(original);
    render(<UniversalInputBar onStartResearch={research} />);
    await user.click(screen.getByTestId('mode-btn-research'));
    await waitFor(() => expect(screen.getByText('Choose an available model in Settings to continue.')).toBeInTheDocument());
    expect(screen.getByRole('button', { name: 'Submit input' })).toBeDisabled();
    expect(research).not.toHaveBeenCalled();
  });
  it('keeps Capture usable and blocks disabled AI generation without clearing the input', async () => {
    const user = userEvent.setup();
    const research = vi.fn();
    render(<UniversalInputBar onStartResearch={research} />);
    expect(screen.queryByTestId('home-model-selector')).not.toBeInTheDocument();
    await user.type(screen.getByRole('textbox'), 'Keep my words');
    await user.keyboard('{Enter}');
    await waitFor(() => expect(invoke).toHaveBeenCalledWith('cmd_save_artifact', expect.objectContaining({ contentMd: '# Keep my words\n\nKeep my words' })));
    await user.click(screen.getByTestId('mode-btn-research'));
    await user.type(screen.getByRole('textbox'), 'Investigate this');
    await user.keyboard('{Enter}');
    expect(research).not.toHaveBeenCalled();
    expect(screen.getByRole('textbox')).toHaveValue('Investigate this');
    expect(screen.getByRole('button', { name: 'Submit input' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Open Settings' })).toBeInTheDocument();
    expect(screen.getByTestId('mode-btn-act')).toHaveTextContent('Draft');
  });

  it('saves enabling and model choices, updates mounted controls, and retains settings on reopen', async () => {
    const user = userEvent.setup();
    const research = vi.fn().mockResolvedValue({});
    const view = render(<><WorkflowSettingsPanel providers={providers} /><UniversalInputBar onStartResearch={research} /></>);
    const enable = await screen.findByRole('checkbox', { name: 'Enable Research' });
    await waitFor(() => expect(enable).toBeEnabled());
    expect(screen.queryByRole('checkbox', { name: 'Enable Research templates' })).not.toBeInTheDocument();
    await user.click(enable);
    await waitFor(() => expect(screen.getByLabelText('Default model', { selector: '#workflow-model-research' })).toBeEnabled());
    await user.selectOptions(screen.getByLabelText('Default model', { selector: '#workflow-model-research' }), 'secondary');
    await waitFor(() => expect(settings.research.model_id).toBe('secondary'));
    await user.click(screen.getByRole('checkbox', { name: 'Enable Research templates' }));
    await waitFor(() => expect(settings.research.templates_enabled).toBe(true));
    await user.click(screen.getByTestId('mode-btn-research'));
    await waitFor(() => expect(screen.getByTestId('home-model-selector')).toHaveTextContent('Secondary model'));
    await user.type(screen.getByRole('textbox'), 'Compare the options');
    await user.keyboard('{Enter}');
    expect(research).toHaveBeenCalledWith('Compare the options', expect.anything(), null, null, 'secondary');
    view.unmount();
    render(<WorkflowSettingsPanel providers={providers} />);
    await waitFor(() => expect(screen.getByRole('checkbox', { name: 'Enable Research' })).toBeChecked());
    expect(screen.getByRole('checkbox', { name: 'Enable Research templates' })).toBeChecked();
  });

  it('does not display a failed preference save as enabled', async () => {
    const user = userEvent.setup();
    const original = invoke.getMockImplementation()!;
    invoke.mockImplementation((command, args) => command === 'cmd_save_workflow_setting' ? Promise.reject(new Error('Disk unavailable')) : original(command, args));
    render(<WorkflowSettingsPanel providers={providers} />);
    const enable = screen.getByRole('checkbox', { name: 'Enable Plan' });
    await waitFor(() => expect(enable).toBeEnabled());
    await user.click(enable);
    expect(await screen.findByRole('alert')).toHaveTextContent('Disk unavailable');
    expect(enable).not.toBeChecked();
  });

  it('keeps historical conversation and unsent text readable while chat is disabled', async () => {
    const user = userEvent.setup();
    render(<DocumentChatPanel artifactId="old-note" initialConversation={{ mode: 'document', initial_prompt: '', turns: [{ id: 'old', user_text: 'Earlier question', reply: 'Earlier answer', error: null, basis_content_md: '', proposed_content_md: null, proposal_state: 'pending', change_set_id: null }] }} open busy={false} modelName="Primary model" currentContent="Original note" onClose={vi.fn()} onApplyingChange={vi.fn()} beforeRequest={vi.fn().mockResolvedValue(true)} onApplied={vi.fn()} />);
    expect(screen.getByText('Earlier answer')).toBeInTheDocument();
    await user.type(screen.getByRole('textbox', { name: 'Message about this document' }), 'My next question');
    await user.keyboard('{Enter}');
    expect(screen.getByRole('button', { name: 'Send message' })).toBeDisabled();
    expect(invoke.mock.calls.some(([command]) => command === 'cmd_discuss_document')).toBe(false);
    await saveWorkflowSetting('chat', { enabled: true, model_id: 'secondary', templates_enabled: false });
    await waitFor(() => expect(screen.getByRole('button', { name: 'Send message' })).toBeEnabled());
    expect(screen.getByRole('textbox', { name: 'Message about this document' })).toHaveValue('My next question');
    expect(screen.getByLabelText('Conversation model')).toHaveValue('secondary');
  });
});
