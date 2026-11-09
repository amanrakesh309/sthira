import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor, fireEvent, act } from '@testing-library/react';
import { LeftSidebar } from '../components/LeftSidebar';
import { ExplorePlanView } from '../views/ExplorePlanView';
import { WorkflowSettingsPanel } from '../components/WorkflowSettingsPanel';
import {
  defaultWorkflowSettings,
  WORKFLOW_SETTINGS_CHANGED,
  type WorkflowSettings,
} from '../api/workflowSettings';
import type { ProviderEntry } from '../types/sthira';

// Unmock useWorkflowSettings to test real workflow readiness behavior
vi.unmock('../hooks/useWorkflowSettings');

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({
  invoke,
  Channel: class {
    onmessage = () => {};
  },
}));

vi.mock('../api/ipc', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../api/ipc')>();
  return {
    ...actual,
    onScheduledTopicReady: vi.fn(async () => () => {}),
  };
});

const createTestProviders = (overrides?: {
  providerEnabled?: boolean;
  modelDefault?: boolean;
  modelEnabled?: boolean;
}): ProviderEntry[] => [
  {
    id: 'prov-1',
    display_name: 'Test Provider',
    provider_type: 'ollama',
    base_url: 'http://127.0.0.1:11434',
    has_api_key: false,
    is_enabled: overrides?.providerEnabled ?? true,
    models: [
      {
        id: 'model-1',
        provider_id: 'prov-1',
        model_identifier: 'llama3:latest',
        display_name: 'Llama 3',
        context_window: 8192,
        supports_structured_output: true,
        is_enabled: overrides?.modelEnabled ?? true,
        is_default: overrides?.modelDefault ?? false,
      },
      {
        id: 'model-2',
        provider_id: 'prov-1',
        model_identifier: 'mistral:latest',
        display_name: 'Mistral 7B',
        context_window: 8192,
        supports_structured_output: true,
        is_enabled: true,
        is_default: false,
      },
    ],
  },
];

let testSettings: WorkflowSettings;

beforeEach(() => {
  testSettings = defaultWorkflowSettings();
  invoke.mockReset().mockImplementation(async (command, args) => {
    switch (command) {
      case 'cmd_get_workflow_settings':
        return structuredClone(testSettings);
      case 'cmd_save_workflow_setting':
        testSettings[args.workflow as keyof WorkflowSettings] = args.setting;
        return;
      case 'cmd_get_provider_catalog':
        return createTestProviders({ modelDefault: true });
      case 'cmd_get_workspace_tree':
        return { projects: [], unassociated_pages: [] };
      case 'cmd_get_sidebar_items':
        return { projects: [], notes: [], objects: [] };
      case 'cmd_list_object_types':
      case 'cmd_get_object_type_counts':
        return [];
      case 'cmd_get_workspace_index':
        return { counts: {}, items: [] };
      case 'cmd_list_scheduled_topics':
        return [];
      case 'cmd_list_synthesis_templates':
        return [];
      default:
        return [];
    }
  });
});

describe('Sidebar workflow availability and readiness checks', () => {
  it('disables Explore & Plan when workflows are unconfigured / fresh, blocks navigation, and provides Check settings', async () => {
    // Both research and plan are enabled: false by default
    const navigateSpy = vi.fn();
    const settingsListener = vi.fn();
    window.addEventListener('sthira:open-settings', settingsListener);

    render(
      <LeftSidebar
        currentView="home"
        isCollapsed={false}
        onNavigateHome={vi.fn()}
        onNavigateGraph={vi.fn()}
        onNavigateExplorePlan={navigateSpy}
        onOpenProject={vi.fn()}
        onOpenArtifact={vi.fn()}
        onOpenCanvas={vi.fn()}
        onOpenSearch={vi.fn()}
        onOpenSettings={vi.fn()}
        onOpenAccount={vi.fn()}
        onToggleCollapse={vi.fn()}
        providers={createTestProviders({ modelDefault: true })}
      />
    );

    const navBtn = await screen.findByTestId('nav-explore-plan');
    expect(navBtn).toBeDisabled();
    expect(navBtn).toHaveAttribute('aria-disabled', 'true');
    expect(navBtn).toHaveAttribute('title', 'Enable this workflow and select a model in Settings.');

    // Explanation message
    expect(
      screen.getByText('Enable this workflow and select a model in Settings.')
    ).toBeInTheDocument();

    // Clicking does not navigate
    fireEvent.click(navBtn);
    expect(navigateSpy).not.toHaveBeenCalled();

    // Info button exists: hovering or clicking opens the informative popover
    const setupBtn = screen.getByTestId('sidebar-setup-ai-btn');
    expect(setupBtn).toBeInTheDocument();
    expect(setupBtn).toHaveAttribute(
      'title',
      'Workflow info: Click or hover to view details'
    );
    fireEvent.click(setupBtn);

    const popover = await screen.findByTestId('sidebar-workflow-info-popover');
    expect(popover).toBeInTheDocument();
    expect(screen.getByText('Explore & Plan unavailable')).toBeInTheDocument();
    expect(
      screen.getByText(/This workflow is currently disabled\. To use Research and Plan, enable the workflow in Settings/)
    ).toBeInTheDocument();

    // Clicking "Open Settings" inside the popover navigates to workflow settings
    const openSettingsBtn = screen.getByTestId('sidebar-open-settings-popover-btn');
    fireEvent.click(openSettingsBtn);
    expect(settingsListener).toHaveBeenCalledWith(
      expect.objectContaining({ detail: 'workflows' })
    );

    window.removeEventListener('sthira:open-settings', settingsListener);
  });

  it('routes to research tab when only research is ready', async () => {
    testSettings.research = {
      enabled: true,
      model_id: 'model-1',
      templates_enabled: true,
    };
    testSettings.plan = {
      enabled: false,
      model_id: null,
      templates_enabled: false,
    };

    const navigateSpy = vi.fn();
    render(
      <LeftSidebar
        currentView="home"
        isCollapsed={false}
        onNavigateHome={vi.fn()}
        onNavigateGraph={vi.fn()}
        onNavigateExplorePlan={navigateSpy}
        onOpenProject={vi.fn()}
        onOpenArtifact={vi.fn()}
        onOpenCanvas={vi.fn()}
        onOpenSearch={vi.fn()}
        onOpenSettings={vi.fn()}
        onOpenAccount={vi.fn()}
        onToggleCollapse={vi.fn()}
        providers={createTestProviders({ modelDefault: false })}
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId('nav-explore-plan')).toBeEnabled();
    });

    fireEvent.click(screen.getByTestId('nav-explore-plan'));
    expect(navigateSpy).toHaveBeenCalledWith('research');

    // Granular test aliases
    expect(screen.getByTestId('nav-research')).toBeEnabled();
    expect(screen.getByTestId('nav-plan')).toBeDisabled();
  });

  it('routes to plan tab when only plan is ready', async () => {
    testSettings.research = {
      enabled: false,
      model_id: null,
      templates_enabled: false,
    };
    testSettings.plan = {
      enabled: true,
      model_id: 'model-1',
      templates_enabled: true,
    };

    const navigateSpy = vi.fn();
    render(
      <LeftSidebar
        currentView="home"
        isCollapsed={false}
        onNavigateHome={vi.fn()}
        onNavigateGraph={vi.fn()}
        onNavigateExplorePlan={navigateSpy}
        onOpenProject={vi.fn()}
        onOpenArtifact={vi.fn()}
        onOpenCanvas={vi.fn()}
        onOpenSearch={vi.fn()}
        onOpenSettings={vi.fn()}
        onOpenAccount={vi.fn()}
        onToggleCollapse={vi.fn()}
        providers={createTestProviders({ modelDefault: false })}
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId('nav-explore-plan')).toBeEnabled();
    });

    fireEvent.click(screen.getByTestId('nav-explore-plan'));
    expect(navigateSpy).toHaveBeenCalledWith('plan');

    // Granular test aliases
    expect(screen.getByTestId('nav-research')).toBeDisabled();
    expect(screen.getByTestId('nav-plan')).toBeEnabled();
  });

  it('does NOT substitute first available model when model selection is missing and workspace has no default', async () => {
    // Enable switches are true, but no model_id is chosen, and neither model has is_default: true
    testSettings.research = { enabled: true, model_id: null, templates_enabled: false };
    testSettings.plan = { enabled: true, model_id: null, templates_enabled: false };

    const providersWithoutDefault = createTestProviders({ modelDefault: false });

    render(
      <LeftSidebar
        currentView="home"
        isCollapsed={false}
        onNavigateHome={vi.fn()}
        onNavigateGraph={vi.fn()}
        onNavigateExplorePlan={vi.fn()}
        onOpenProject={vi.fn()}
        onOpenArtifact={vi.fn()}
        onOpenCanvas={vi.fn()}
        onOpenSearch={vi.fn()}
        onOpenSettings={vi.fn()}
        onOpenAccount={vi.fn()}
        onToggleCollapse={vi.fn()}
        providers={providersWithoutDefault}
      />
    );

    const navBtn = await screen.findByTestId('nav-explore-plan');
    expect(navBtn).toBeDisabled();
    expect(screen.getByTestId('sidebar-setup-ai-btn')).toBeInTheDocument();
  });

  it('resolves inherited workspace default model when model_id is null', async () => {
    // Enable switches true, model_id null, but modelDefault is true
    testSettings.research = { enabled: true, model_id: null, templates_enabled: false };
    const providersWithDefault = createTestProviders({ modelDefault: true });

    const navigateSpy = vi.fn();
    render(
      <LeftSidebar
        currentView="home"
        isCollapsed={false}
        onNavigateHome={vi.fn()}
        onNavigateGraph={vi.fn()}
        onNavigateExplorePlan={navigateSpy}
        onOpenProject={vi.fn()}
        onOpenArtifact={vi.fn()}
        onOpenCanvas={vi.fn()}
        onOpenSearch={vi.fn()}
        onOpenSettings={vi.fn()}
        onOpenAccount={vi.fn()}
        onToggleCollapse={vi.fn()}
        providers={providersWithDefault}
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId('nav-explore-plan')).toBeEnabled();
    });
    fireEvent.click(screen.getByTestId('nav-explore-plan'));
    expect(navigateSpy).toHaveBeenCalledWith('research');
  });

  it('disables workflow when model provider is disabled', async () => {
    testSettings.research = { enabled: true, model_id: 'model-1', templates_enabled: false };
    const disabledProvider = createTestProviders({ providerEnabled: false });

    render(
      <LeftSidebar
        currentView="home"
        isCollapsed={false}
        onNavigateHome={vi.fn()}
        onNavigateGraph={vi.fn()}
        onNavigateExplorePlan={vi.fn()}
        onOpenProject={vi.fn()}
        onOpenArtifact={vi.fn()}
        onOpenCanvas={vi.fn()}
        onOpenSearch={vi.fn()}
        onOpenSettings={vi.fn()}
        onOpenAccount={vi.fn()}
        onToggleCollapse={vi.fn()}
        providers={disabledProvider}
      />
    );

    const navBtn = await screen.findByTestId('nav-explore-plan');
    expect(navBtn).toBeDisabled();
  });

  it('updates availability immediately upon settings change event without restart', async () => {
    const providers = createTestProviders({ modelDefault: true });

    render(
      <LeftSidebar
        currentView="home"
        isCollapsed={false}
        onNavigateHome={vi.fn()}
        onNavigateGraph={vi.fn()}
        onNavigateExplorePlan={vi.fn()}
        onOpenProject={vi.fn()}
        onOpenArtifact={vi.fn()}
        onOpenCanvas={vi.fn()}
        onOpenSearch={vi.fn()}
        onOpenSettings={vi.fn()}
        onOpenAccount={vi.fn()}
        onToggleCollapse={vi.fn()}
        providers={providers}
      />
    );

    const navBtn = await screen.findByTestId('nav-explore-plan');
    expect(navBtn).toBeDisabled();

    // Now dispatch settings change event enabling research
    const updatedSetting = { enabled: true, model_id: 'model-1', templates_enabled: true };
    act(() => {
      window.dispatchEvent(
        new CustomEvent(WORKFLOW_SETTINGS_CHANGED, {
          detail: { workflow: 'research', setting: updatedSetting },
        })
      );
    });

    await waitFor(() => {
      expect(screen.getByTestId('nav-explore-plan')).toBeEnabled();
    });
  });

  it('handles collapsed sidebar mode with accessible info button', async () => {
    const settingsListener = vi.fn();
    window.addEventListener('sthira:open-settings', settingsListener);

    render(
      <LeftSidebar
        currentView="home"
        isCollapsed={true}
        onNavigateHome={vi.fn()}
        onNavigateGraph={vi.fn()}
        onNavigateExplorePlan={vi.fn()}
        onOpenProject={vi.fn()}
        onOpenArtifact={vi.fn()}
        onOpenCanvas={vi.fn()}
        onOpenSearch={vi.fn()}
        onOpenSettings={vi.fn()}
        onOpenAccount={vi.fn()}
        onToggleCollapse={vi.fn()}
        providers={createTestProviders({ modelDefault: false })}
      />
    );

    const collapsedBtn = await screen.findByTestId('nav-explore-plan-collapsed');
    expect(collapsedBtn).toBeDisabled();
    expect(collapsedBtn).toHaveAttribute(
      'title',
      'Enable this workflow and select a model in Settings.'
    );

    const collapsedSetupBtn = screen.getByTestId('sidebar-setup-ai-collapsed-btn');
    expect(collapsedSetupBtn).toBeInTheDocument();
    expect(collapsedSetupBtn).toHaveAttribute(
      'title',
      'Workflow info: Click or hover to view details'
    );
    fireEvent.click(collapsedSetupBtn);

    const collapsedPopover = await screen.findByTestId('sidebar-workflow-info-popover-collapsed');
    expect(collapsedPopover).toBeInTheDocument();
    expect(screen.getByText('Explore & Plan unavailable')).toBeInTheDocument();

    const openSettingsCollapsedBtn = screen.getByTestId('sidebar-open-settings-popover-collapsed-btn');
    fireEvent.click(openSettingsCollapsedBtn);
    expect(settingsListener).toHaveBeenCalledWith(
      expect.objectContaining({ detail: 'workflows' })
    );

    window.removeEventListener('sthira:open-settings', settingsListener);
  });

  it('renders top navigation buttons in the required order: Home, All Files, Explore & Plan, Knowledge Graph, Canvas', async () => {
    render(
      <LeftSidebar
        currentView="home"
        isCollapsed={false}
        onNavigateHome={vi.fn()}
        onNavigateGraph={vi.fn()}
        onNavigateExplorePlan={vi.fn()}
        onOpenProject={vi.fn()}
        onOpenArtifact={vi.fn()}
        onOpenCanvas={vi.fn()}
        onOpenSearch={vi.fn()}
        onOpenSettings={vi.fn()}
        onOpenAccount={vi.fn()}
        onToggleCollapse={vi.fn()}
        providers={createTestProviders({ modelDefault: true })}
      />
    );

    const navButtons = [
      await screen.findByTestId('nav-home'),
      screen.getByTestId('nav-workspace-index'),
      screen.getByTestId('nav-explore-plan'),
      screen.getByTestId('nav-knowledge-graph'),
      screen.getByTestId('nav-canvas'),
    ];

    // Assert DOM ordering: each item follows the previous item
    for (let i = 0; i < navButtons.length - 1; i++) {
      expect(
        navButtons[i].compareDocumentPosition(navButtons[i + 1]) &
          Node.DOCUMENT_POSITION_FOLLOWING
      ).toBeTruthy();
    }
  });
});

describe('ExplorePlanView workflow readiness and generation control gating', () => {
  it('disables unavailable tab and generation controls in ExplorePlanView', async () => {
    // Research enabled, Plan disabled
    testSettings.research = { enabled: true, model_id: 'model-1', templates_enabled: true };
    testSettings.plan = { enabled: false, model_id: null, templates_enabled: false };

    render(
      <ExplorePlanView
        initialTab="research"
        onOpenArtifact={vi.fn()}
        onStartResearch={vi.fn()}
        onStartPlan={vi.fn()}
        onBack={vi.fn()}
      />
    );

    // Research tab is enabled
    const researchTab = await screen.findByTestId('tab-research');
    expect(researchTab).toBeEnabled();

    // Plan tab is disabled
    const planTab = screen.getByTestId('tab-plan');
    expect(planTab).toBeDisabled();
    expect(planTab).toHaveAttribute(
      'title',
      'Enable this workflow and select a model in Settings.'
    );

    // Inside research tab, input and submit are active
    const researchInput = screen.getByTestId('input-research-query');
    expect(researchInput).toBeEnabled();
  });

  it('switches to available tab when initialTab is set to an unavailable workflow', async () => {
    // Only Plan is ready
    testSettings.research = { enabled: false, model_id: null, templates_enabled: false };
    testSettings.plan = { enabled: true, model_id: 'model-1', templates_enabled: true };

    render(
      <ExplorePlanView
        initialTab="research" // requesting unavailable tab
        onOpenArtifact={vi.fn()}
        onStartResearch={vi.fn()}
        onStartPlan={vi.fn()}
        onBack={vi.fn()}
      />
    );

    // Should automatically switch to Plan tab
    await waitFor(() => {
      expect(screen.getByTestId('tab-content-plan')).toBeInTheDocument();
    });

    const planTab = screen.getByTestId('tab-plan');
    expect(planTab).toBeEnabled();
    const researchTab = screen.getByTestId('tab-research');
    expect(researchTab).toBeDisabled();
  });

  it('keeps saved documents and recent list visible when workflow is unavailable', async () => {
    testSettings.research = { enabled: false, model_id: null, templates_enabled: false };
    testSettings.plan = { enabled: false, model_id: null, templates_enabled: false };

    render(
      <ExplorePlanView
        initialTab="research"
        onOpenArtifact={vi.fn()}
        onStartResearch={vi.fn()}
        onStartPlan={vi.fn()}
        onBack={vi.fn()}
      />
    );

    // Unavailable banner is rendered
    expect(await screen.findByTestId('research-unavailable-banner')).toBeInTheDocument();
    expect(screen.getByTestId('btn-setup-research-ai')).toBeInTheDocument();

    // Generation controls are disabled
    expect(screen.getByTestId('input-research-query')).toBeDisabled();
    expect(screen.getByTestId('btn-start-research')).toBeDisabled();

    // Recent research section is still rendered
    expect(screen.getByTestId('recent-research-section')).toBeInTheDocument();
  });
});

describe('WorkflowSettingsPanel information and guidance', () => {
  it('renders the explanatory guide banner explaining why to toggle and the 3 steps', async () => {
    const navigateTabSpy = vi.fn();
    await act(async () => {
      render(
        <WorkflowSettingsPanel
          providers={createTestProviders({ modelDefault: true })}
          onNavigateTab={navigateTabSpy}
        />
      );
    });

    // Guide banner explaining why to toggle
    expect(screen.getByTestId('workflow-guide-banner')).toBeInTheDocument();
    expect(screen.getByText('Why Toggle Workflows & How They Work')).toBeInTheDocument();
    expect(screen.getByText(/Local-first privacy:/)).toBeInTheDocument();
    expect(screen.getByText(/Step 1/)).toBeInTheDocument();
    expect(screen.getByText(/Step 2/)).toBeInTheDocument();
    expect(screen.getByText(/Step 3/)).toBeInTheDocument();

    // Fast-jump to AI & Models tab
    const jumpBtn = screen.getByTestId('btn-go-to-models-guide');
    fireEvent.click(jumpBtn);
    expect(navigateTabSpy).toHaveBeenCalledWith('models');

    // Cards display purpose and disabled status
    expect(screen.getByText(/Unlocks Research under Explore & Plan in the sidebar/)).toBeInTheDocument();
    expect(screen.getAllByText(/No model calls/).length).toBeGreaterThan(0);
  });
});
