import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { UniversalInputBar } from '../components/UniversalInputBar';
import { App } from '../App';

// Mock Tauri invoke
const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
  // `streamAdaptiveRun` constructs a Tauri Channel to receive progressive chunks.
  Channel: class {
    onmessage: ((message: unknown) => void) | null = null;
  },
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

describe('Universal Input Bar Actions: Capture, Research, Plan, Act', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('Capture mode: typing note and pressing Enter creates page and calls onOpenArtifact', async () => {
    const user = userEvent.setup();
    const onOpenArtifact = vi.fn();
    const onRefresh = vi.fn();

    mockInvoke.mockImplementation(async (cmd: string, args: any) => {
      if (cmd === 'cmd_get_provider_catalog') {
        return [];
      }
      if (cmd === 'cmd_create_page') {
        return {
          id: 'captured-art-123',
          artifact_type: 'note',
          authorship: 'user',
          title: args.title || 'My First Quick Capture',
          content_md: `# ${args.title}\n\n`,
          version: 1,
          is_unfinished_draft: false,
          created_at: Date.now(),
          updated_at: Date.now(),
        };
      }
      if (cmd === 'cmd_save_artifact') {
        return {
          id: args.artifactId || 'captured-art-123',
          title: args.title,
          content_md: args.contentMd,
          version: 1,
        };
      }
      return {};
    });

    render(
      <UniversalInputBar
        onOpenArtifact={onOpenArtifact}
        onRefresh={onRefresh}
      />
    );

    const input = screen.getByPlaceholderText(/Ask, research, capture or plan/i);
    await user.type(input, 'My First Quick Capture');
    await user.keyboard('{Enter}');

    await waitFor(() => {
      expect(onOpenArtifact).toHaveBeenCalledWith('captured-art-123', null);
    });

    expect(onRefresh).toHaveBeenCalled();
  });

  it('Research mode: typing inquiry and pressing Enter creates research note and opens note (no project created)', async () => {
    const user = userEvent.setup();
    const onOpenArtifact = vi.fn();
    const onRefresh = vi.fn();

    mockInvoke.mockImplementation(async (cmd: string, args: any) => {
      if (cmd === 'cmd_get_provider_catalog') {
        return [];
      }
      if (cmd === 'cmd_create_page') {
        return {
          id: 'art-research-99',
          artifact_type: 'note',
          authorship: 'user',
          title: args.title,
          content_md: `# ${args.title}\n\n`,
          version: 1,
          is_unfinished_draft: false,
          created_at: Date.now(),
          updated_at: Date.now(),
        };
      }
      if (cmd === 'cmd_save_artifact') {
        return {
          id: args.artifactId || 'art-research-99',
          title: args.title,
          content_md: args.contentMd,
          version: 1,
        };
      }
      return {};
    });

    render(
      <UniversalInputBar
        onOpenArtifact={onOpenArtifact}
        onRefresh={onRefresh}
      />
    );

    // Switch to Research mode
    const researchBtn = screen.getByTestId('mode-btn-research');
    await user.click(researchBtn);

    const input = screen.getByPlaceholderText(/Ask, research, capture or plan/i);
    await user.type(input, 'How does local KV cache quantization work?');
    await user.keyboard('{Enter}');

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        'cmd_create_page',
        expect.objectContaining({
          projectId: null,
          title: 'How does local KV cache quantization work?',
        })
      );
      expect(mockInvoke).toHaveBeenCalledWith(
        'cmd_save_artifact',
        expect.objectContaining({
          artifactId: 'art-research-99',
          title: 'How does local KV cache quantization work?',
          projectId: null,
        })
      );
      expect(onOpenArtifact).toHaveBeenCalledWith('art-research-99', null);
    });

    // Verify project creation was NOT called
    expect(mockInvoke).not.toHaveBeenCalledWith('cmd_synthesize_and_create_research', expect.anything());
    expect(mockInvoke).not.toHaveBeenCalledWith('cmd_create_project', expect.anything());
  });

  it('Plan mode: typing goal and pressing Enter creates plan note and opens note (no project created)', async () => {
    const user = userEvent.setup();
    const onOpenArtifact = vi.fn();
    const onRefresh = vi.fn();

    mockInvoke.mockImplementation(async (cmd: string, args: any) => {
      if (cmd === 'cmd_get_provider_catalog') {
        return [];
      }
      if (cmd === 'cmd_create_page') {
        return {
          id: 'art-plan-88',
          artifact_type: 'note',
          authorship: 'user',
          title: args.title,
          content_md: `# ${args.title}\n\n`,
          version: 1,
          is_unfinished_draft: false,
          created_at: Date.now(),
          updated_at: Date.now(),
        };
      }
      if (cmd === 'cmd_save_artifact') {
        return {
          id: args.artifactId || 'art-plan-88',
          title: args.title,
          content_md: args.contentMd,
          version: 1,
        };
      }
      return {};
    });

    render(
      <UniversalInputBar
        onOpenArtifact={onOpenArtifact}
        onRefresh={onRefresh}
      />
    );

    const planBtn = screen.getByTestId('mode-btn-plan');
    await user.click(planBtn);

    const input = screen.getByPlaceholderText(/Ask, research, capture or plan/i);
    await user.type(input, 'Design distributed cache system');
    await user.keyboard('{Enter}');

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        'cmd_create_page',
        expect.objectContaining({
          projectId: null,
          title: 'Design distributed cache system',
        })
      );
      expect(mockInvoke).toHaveBeenCalledWith(
        'cmd_save_artifact',
        expect.objectContaining({
          artifactId: 'art-plan-88',
          title: 'Design distributed cache system',
          projectId: null,
        })
      );
      expect(onOpenArtifact).toHaveBeenCalledWith('art-plan-88', null);
    });

    // Verify project creation was NOT called
    expect(mockInvoke).not.toHaveBeenCalledWith('cmd_create_guided_project', expect.anything());
    expect(mockInvoke).not.toHaveBeenCalledWith('cmd_create_project', expect.anything());
  });

  it('Act mode: typing directive and pressing Enter creates action note and opens note (no project created)', async () => {
    const user = userEvent.setup();
    const onOpenArtifact = vi.fn();
    const onRefresh = vi.fn();

    mockInvoke.mockImplementation(async (cmd: string, args: any) => {
      if (cmd === 'cmd_get_provider_catalog') {
        return [];
      }
      if (cmd === 'cmd_create_page') {
        return {
          id: 'art-act-77',
          artifact_type: 'note',
          authorship: 'user',
          title: args.title,
          content_md: `# ${args.title}\n\n`,
          version: 1,
          is_unfinished_draft: false,
          created_at: Date.now(),
          updated_at: Date.now(),
        };
      }
      if (cmd === 'cmd_save_artifact') {
        return {
          id: args.artifactId || 'art-act-77',
          title: args.title,
          content_md: args.contentMd,
          version: 1,
        };
      }
      return {};
    });

    render(
      <UniversalInputBar
        onOpenArtifact={onOpenArtifact}
        onRefresh={onRefresh}
      />
    );

    const actBtn = screen.getByTestId('mode-btn-act');
    await user.click(actBtn);

    const input = screen.getByPlaceholderText(/Ask, research, capture or plan/i);
    await user.type(input, 'Refactor database migrations');
    await user.keyboard('{Enter}');

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        'cmd_create_page',
        expect.objectContaining({
          projectId: null,
          title: 'Refactor database migrations',
        })
      );
      expect(mockInvoke).toHaveBeenCalledWith(
        'cmd_save_artifact',
        expect.objectContaining({
          artifactId: 'art-act-77',
          title: 'Refactor database migrations',
          projectId: null,
        })
      );
      expect(onOpenArtifact).toHaveBeenCalledWith('art-act-77', null);
    });

    // Verify project creation was NOT called
    expect(mockInvoke).not.toHaveBeenCalledWith('cmd_create_guided_project', expect.anything());
    expect(mockInvoke).not.toHaveBeenCalledWith('cmd_create_project', expect.anything());
  });

  it('End-to-end in App: capturing note transitions view to FocusView as a standalone note without project', async () => {
    const user = userEvent.setup();

    mockInvoke.mockImplementation(async (cmd: string, args: any) => {
      if (cmd === 'cmd_get_home_state') {
        return {
          projects: [],
          recent_items: [],
          loose_captures: [],
        };
      }
      if (cmd === 'cmd_list_object_types') {
        return [];
      }
      if (cmd === 'cmd_get_provider_catalog') {
        return [];
      }
      if (cmd === 'cmd_get_workspace_tree') {
        return {
          projects: [],
          unassociated_pages: [],
        };
      }
      if (cmd === 'cmd_create_page') {
        return {
          id: 'art-cap-1',
          artifact_type: 'note',
          authorship: 'user',
          title: args.title || 'Quick thought on architecture',
          content_md: `# ${args.title}\n\n`,
          version: 1,
          is_unfinished_draft: false,
          created_at: Date.now(),
          updated_at: Date.now(),
        };
      }
      if (cmd === 'cmd_save_artifact') {
        return {
          id: args.artifactId || 'art-cap-1',
          title: args.title || 'Quick thought on architecture',
          content_md: args.contentMd,
          version: 1,
        };
      }
      if (cmd === 'cmd_get_artifact_context') {
        return {
          artifact: {
            id: 'art-cap-1',
            artifact_type: 'note',
            authorship: 'user',
            title: 'Quick thought on architecture',
            content_md: '# Quick thought on architecture\n\nQuick thought on architecture',
            source_url: null,
            content_hash: 'hash-cap-1',
            version: 1,
            is_unfinished_draft: false,
            created_at: Date.now(),
            updated_at: Date.now(),
          },
          epistemic_nodes: [],
          pending_proposals: [],
          project_id: null,
          is_unfinished_draft: false,
        };
      }
      return {};
    });

    render(<App />);

    // Wait for home view to render
    await waitFor(() => {
      expect(screen.getByText(/Make room to think/i)).toBeInTheDocument();
    });

    const input = screen.getByPlaceholderText(/Ask, research, capture or plan/i);
    await user.type(input, 'Quick thought on architecture');
    await user.keyboard('{Enter}');

    // Verify FocusView opens up and displays the note title without any project attachment
    await waitFor(() => {
      const titleInput = screen.getByTestId('input-artifact-title') as HTMLInputElement;
      expect(titleInput.value).toBe('Quick thought on architecture');
    });

    const editor = screen.getByTestId('textarea-editor') as HTMLTextAreaElement;
    expect(editor.value).toContain('Quick thought on architecture');

    // Breadcrumb should display "Notes", NOT "Project Workspace"
    expect(screen.getByText('Notes')).toBeInTheDocument();

    // Verify project creation was NEVER invoked
    expect(mockInvoke).not.toHaveBeenCalledWith('cmd_create_project', expect.anything());
    expect(mockInvoke).not.toHaveBeenCalledWith('cmd_create_guided_project', expect.anything());
    expect(mockInvoke).not.toHaveBeenCalledWith('cmd_synthesize_and_create_research', expect.anything());
  });
});
