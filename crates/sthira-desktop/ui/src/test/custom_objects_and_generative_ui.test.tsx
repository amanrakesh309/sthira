import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { FocusView } from '../components/FocusView';
import {
  AdaptiveResponseRenderer,
  extractMarkdownPhases,
  extractMarkdownComparisonRows,
  extractMarkdownDecisions,
} from '../components/AdaptiveResponseRenderer';
import { FormattedMarkdownView } from '../components/FormattedMarkdownView';
import type { ObjectType } from '../types/sthira';

// Mock Tauri invoke
const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

describe('Custom Objects in Slash Menu & Guaranteed Generative UI', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // -------------------------------------------------------------------------
  // Test 1: Create custom object type "Experiment" -> Open FocusView -> Type /
  // -> Assert /experiment appears in Slash Command menu -> Select it ->
  // Submit properties -> Assert cmd_create_typed_object / cmd_save_object_instance
  // is called and embedded object card renders.
  // -------------------------------------------------------------------------
  it('Test 1: Custom Object /experiment appears in slash menu, creates instance, and renders interactive embedded card', async () => {
    const user = userEvent.setup();

    const customExperimentType: ObjectType = {
      id: 'experiment',
      display_name: 'Experiment',
      icon: '🧪',
      color: '#8B5CF6',
      schema: [
        {
          key: 'hypothesis',
          label: 'Hypothesis',
          property_type: 'text',
          required: true,
        },
        {
          key: 'status',
          label: 'Status',
          property_type: 'select',
          options: ['proposed', 'running', 'concluded'],
          required: true,
        },
      ],
      is_system: false,
      created_at: 1000,
    };

    mockInvoke.mockImplementation(async (cmd: string, args: any) => {
      if (cmd === 'cmd_get_artifact_context') {
        return {
          artifact: {
            id: 'art-doc-1',
            artifact_type: 'note',
            authorship: 'user',
            title: 'Lab Notebook',
            content_md: '# Lab Notebook\n\nInitial experiments notes.\n',
            source_url: null,
            content_hash: 'hash-doc-1',
            version: 1,
            file_path: null,
            last_synced_hash: null,
            is_unfinished_draft: false,
            deleted_at: null,
            created_at: 1000,
            updated_at: 1000,
          },
          epistemic_nodes: [],
          pending_proposals: [],
          project_id: 'proj-lab',
          is_unfinished_draft: false,
        };
      }
      if (cmd === 'cmd_get_resume_brief') {
        return {
          project_id: 'proj-lab',
          level: 1,
          text: 'Lab summary.',
          created_at: 1000,
        };
      }
      if (cmd === 'cmd_get_project_history') {
        return {
          sessions: [],
          stats: { total_sessions: 0, total_artifacts: 1, total_epistemic_nodes: 0, first_active_at: 1000, last_active_at: 1000 },
          pending_proposals_count: 0,
        };
      }
      if (cmd === 'cmd_get_provider_catalog') {
        return [];
      }
      if (cmd === 'cmd_list_object_types') {
        return [customExperimentType];
      }
      if (cmd === 'cmd_get_type_templates') {
        return [
          {
            id: 'experiment-default',
            object_type_id: 'experiment',
            name: 'Experiment Log',
            icon: '🧪',
            is_default: true,
            properties_preset: { status: 'proposed' },
            body_template_md: '## Hypothesis\n\n## Method',
            created_at: 1000,
          },
        ];
      }
      if (cmd === 'cmd_instantiate_object_from_template') {
        return 'art-exp-999';
      }
      if (cmd === 'cmd_get_object_instance') {
        return null;
      }
      if (cmd === 'cmd_create_typed_object') {
        return {
          artifact_id: 'art-exp-999',
          title: args.title,
          content_md: `# ${args.title}\n\n`,
          object_type_id: args.objectTypeId,
          properties: args.properties,
          project_id: args.projectId,
          updated_at: 2000,
        };
      }
      if (cmd === 'cmd_save_object_instance') {
        return null;
      }
      if (cmd === 'cmd_save_artifact') {
        return {
          id: args.artifactId,
          artifact_type: 'note',
          authorship: 'user',
          title: 'Lab Notebook',
          content_md: args.contentMd,
          source_url: null,
          content_hash: 'hash-saved',
          version: 2,
          file_path: null,
          last_synced_hash: null,
          is_unfinished_draft: false,
          deleted_at: null,
          created_at: 1000,
          updated_at: 2000,
        };
      }
      return null;
    });

    render(
      <FocusView
        projectId="proj-lab"
        artifactId="art-doc-1"
        onBack={vi.fn()}
        onOpenCanvas={vi.fn()}
      />
    );

    // 1. Wait for document and object types to load
    await waitFor(() => {
      expect(screen.getByTestId('textarea-editor')).toBeInTheDocument();
    });

    const textarea = screen.getByTestId('textarea-editor') as HTMLTextAreaElement;

    // 2. Type '/' to trigger the slash command popover menu
    fireEvent.change(textarea, { target: { value: '# Lab Notebook\n\n/' } });
    (textarea as any).selectionStart = textarea.value.length;
    fireEvent.select(textarea);

    // 3. Assert /experiment appears under Custom & Typed Objects
    await waitFor(() => {
      expect(screen.getByTestId('slash-cmd-experiment')).toBeInTheDocument();
      expect(screen.getByText('Custom & Typed Objects')).toBeInTheDocument();
      expect(screen.getByText('Experiment')).toBeInTheDocument();
    });

    // 4. Click the /experiment slash command
    const experimentCmd = screen.getByTestId('slash-cmd-experiment');
    await user.click(experimentCmd);

    // 5. Assert Quick Object Creator Card mounts in the DOM
    await waitFor(() => {
      expect(screen.getByTestId('quick-object-creator-card')).toBeInTheDocument();
      expect(screen.getByText('New Experiment')).toBeInTheDocument();
    });

    // 6. Enter Title in Quick Object Creator Card
    const titleInput = screen.getByTestId('input-object-title');
    await user.type(titleInput, 'Quantum Latency Benchmark');

    // 7. Click [Create & Insert]
    const createBtn = screen.getByTestId('btn-create-insert-object');
    await user.click(createBtn);

    // 8. Verify cmd_instantiate_object_from_template was invoked with correct payload
    //    (the template-hydrated creation path replaces the legacy bare-create command)
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        'cmd_instantiate_object_from_template',
        expect.objectContaining({
          typeId: 'experiment',
          templateId: 'experiment-default',
          title: 'Quantum Latency Benchmark',
          projectId: 'proj-lab',
        })
      );
    });

    // 9. Switch to Rich Preview mode
    const previewBtn = screen.getByTestId('btn-toggle-preview-mode');
    await user.click(previewBtn);

    // 10. Assert the embedded object card renders in Rich Preview
    await waitFor(() => {
      expect(screen.getByTestId('embedded-object-art-exp-999')).toBeInTheDocument();
      expect(screen.getByText('Quantum Latency Benchmark')).toBeInTheDocument();
    });

    // 11. Interact with status property inside embedded object card
    const selectElem = screen.getByRole('combobox');
    expect(selectElem).toBeInTheDocument();
    await user.selectOptions(selectElem, 'concluded');

    // 12. Assert cmd_save_object_instance was called with updated property
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        'cmd_save_object_instance',
        expect.objectContaining({
          instance: expect.objectContaining({
            artifact_id: 'art-exp-999',
            object_type_id: 'experiment',
            properties: expect.objectContaining({
              status: 'concluded',
            }),
          }),
        })
      );
    });
  });

  // -------------------------------------------------------------------------
  // Test 2: Guaranteed Generative UI in Plan and Research Modes:
  // Assert <MilestonePlanDagWidget/> and <EvidenceComparisonMatrixWidget/>
  // visibly mount in the DOM across both explicit and auto-hydrated markdown.
  // -------------------------------------------------------------------------
  it('Test 2: Plan and Research Generative UI widgets visibly mount in the DOM', async () => {
    // A. Plan Mode with standard markdown (auto-hydrated into MilestonePlanDagWidget)
    const planMarkdown = `# Distributed System Implementation Plan

## Phase 1: Storage Layer Architecture
- [ ] Formalize SQLite schema migrations
- [ ] Implement append-only changeset log

## Phase 2: Progressive Streaming Engine
- [ ] Fail-safe token stream buffer
- [ ] IPC progressive chunk dispatch

## Decisions
Which database engine should be used?
- SQLite WAL (recommended)
- Remote Postgres
`;

    const extracted = extractMarkdownPhases(planMarkdown);
    expect(extracted.length).toBe(2);

    const { unmount } = render(<AdaptiveResponseRenderer rawText={planMarkdown} />);

    // Assert MilestonePlanDagWidget mounted and visible
    expect(screen.getByText(/Progressive Plan DAG/)).toBeInTheDocument();
    expect(screen.getByText(/Storage Layer Architecture/)).toBeInTheDocument();
    expect(screen.getByText(/Progressive Streaming Engine/)).toBeInTheDocument();
    expect(screen.getByText('Which database engine should be used?')).toBeInTheDocument();

    unmount();

    // B. Research Mode with Markdown Table (auto-hydrated into EvidenceComparisonMatrixWidget)
    const researchMarkdown = `# Comparative Technical Evaluation

| Candidate Architecture | Latency | Memory Footprint | Verdict |
| :--- | :--- | :--- | :--- |
| Local SQLite Engine | < 1ms | 12 MB | Immediate zero-jitter rendering |
| Cloud REST Engine | 180ms | 85 MB | Significant round-trip latency |

> **Decision:** Use Local SQLite Engine for sub-millisecond local-first continuity.
`;

    const extractedRows = extractMarkdownComparisonRows(researchMarkdown);
    expect(extractedRows.length).toBe(2);

    render(<AdaptiveResponseRenderer rawText={researchMarkdown} />);

    // Assert EvidenceComparisonMatrixWidget mounted and visible
    await waitFor(() => {
      expect(screen.getByText(/Evidence Comparison Matrix/)).toBeInTheDocument();
      expect(screen.getByText('Local SQLite Engine')).toBeInTheDocument();
      expect(screen.getByText('Cloud REST Engine')).toBeInTheDocument();
      expect(screen.getByText('Immediate zero-jitter rendering')).toBeInTheDocument();
    });
  });

  // -------------------------------------------------------------------------
  // Test 3: Real-world domain-adaptive plan (e.g. Weight Loss) auto-hydrates
  // into PlanPhase DAG, Evidence Matrix, and Decision Selector, while
  // FormattedMarkdownView normalizes LaTeX math symbols and orphaned bullets.
  // -------------------------------------------------------------------------
  it('Test 3: Real-world domain plan auto-hydrates into rich widgets and FormattedMarkdownView normalizes math & bullets', async () => {
    const rawWeightLossMarkdown = `As an expert systems architect, I will treat your weight loss goal as a **Biological Optimization Project**. To move from 81kg to a leaner state while maintaining muscle mass (via 80g of protein) on a vegetarian diet, we need a phased deployment plan.
System Analysis (The Baseline)
•
**Current State:** 81kg | 175cm (5'9")
•
**Target Metric:** Weight loss (Caloric Deficit)
•
**Constraint A:** Vegetarian (No meat/fish)
•
**Constraint B:** $\\ge 80g$ Protein/day (to prevent muscle atrophy)
---
Phase 1: Caloric Budgeting & Macro Schema
Before selecting foods, we must define the "Energy Budget." For your height and weight, your Maintenance Calories (TDEE) are roughly 2,200–2,400 kcal (assuming sedentary/light activity). To lose weight safely, we will implement a **500 kcal deficit**.
**Architectural Invariant:** The "Protein Floor." No matter the caloric fluctuation, protein must remain $\\ge 80g$. This ensures you lose fat, not muscle.
---
Phase 2: Protein Source Engineering
The primary challenge in vegetarian diets is the "Protein-to-Calorie Ratio." We need "high-density" proteins that don't blow the calorie budget.
**The "Protein Stack" for your plan:**
1. **Greek Yogurt/Paneer (Low Fat):** High bioavailability.
2. **Tofu/Tempeh:** Complete plant proteins.
3. **Lentils/Chickpeas:** Fiber + Protein (Careful with carb count).
4. **Whey/Pea Protein:** Efficient "top-off" to hit 80g without excess calories.
---
Phase 3: The Daily Deployment (Meal Plan)
We will distribute the 1,800 kcal and 80g protein across four "execution windows."
#### The Proposed Blueprint:
Meal\tComponent\tEst. Calories\tEst. Protein
**Breakfast**\tGreek Yogurt (200g) + Berries + 10g Almonds\t300 kcal\t22g
**Lunch**\t100g Tofu/Paneer + 1 cup Sautéed Spinach/Broccoli + 1/2 cup Brown Rice\t500 kcal\t25g
**Snack**\t1 Apple + 1 Scoop Whey/Pea Protein (with water)\t250 kcal\t25g
**Dinner**\t1 cup Lentil Soup (Dal) + Large Mixed Green Salad (Lemon dressing)\t450 kcal\t15g
**Total**\t\t**~1,500 - 1,700 kcal**\t**~87g**
---
Phase 4: Monitoring & Iteration (The Feedback Loop)
Weight loss is not linear. We must implement a "Telemetry" system to adjust the plan based on actual biological response.
**Trade-offs & Decisions:**
•
**The Carbohydrate Trade-off:** To keep protein high and calories low, we must limit "empty" carbs (white bread, sugar). We trade these for "complex" carbs (quinoa, oats) to maintain insulin stability.
Final System Summary
1. **Calorie Target:** $\\approx 1,700$ kcal.
2. **Protein Target:** $80g - 90g$ (Prioritize Tofu, Greek Yogurt, and Protein Powder).
3. **Water Invariant:** Minimum 3L per day to assist kidney function during high protein intake.
4. **Activity Modifier:** If you add weight training, increase calories by 200 (via complex carbs) to support recovery.`;

    // 1. Verify auto-hydration functions extract phases, table rows, and decisions
    const phases = extractMarkdownPhases(rawWeightLossMarkdown);
    expect(phases.length).toBeGreaterThanOrEqual(4);
    expect(phases[0].title).toContain('Caloric Budgeting & Macro Schema');
    expect(phases[1].title).toContain('Protein Source Engineering');
    expect(phases[1].milestones.length).toBeGreaterThanOrEqual(4);
    expect(phases[1].milestones.some((m) => m.includes('Greek Yogurt') || m.includes('Tofu'))).toBe(true);

    const comparisonRows = extractMarkdownComparisonRows(rawWeightLossMarkdown);
    expect(comparisonRows.length).toBeGreaterThanOrEqual(4);
    expect(comparisonRows.some((r) => r.criteria.includes('Breakfast'))).toBe(true);
    expect(comparisonRows.some((r) => r.criteria.includes('Lunch'))).toBe(true);

    const decisions = extractMarkdownDecisions(rawWeightLossMarkdown);
    expect(decisions.length).toBeGreaterThanOrEqual(1);

    // 2. Verify AdaptiveResponseRenderer renders all 3 widgets from the raw text
    const { unmount } = render(<AdaptiveResponseRenderer rawText={rawWeightLossMarkdown} />);

    await waitFor(() => {
      expect(screen.getByText(/Progressive Plan DAG/)).toBeInTheDocument();
      expect(screen.getByText(/Evidence Comparison Matrix/)).toBeInTheDocument();
      expect(screen.getByText(/Interactive Decisions/)).toBeInTheDocument();
    });

    unmount();

    // 3. Verify FormattedMarkdownView normalizes LaTeX math symbols and orphaned bullets
    render(<FormattedMarkdownView content={rawWeightLossMarkdown} />);
    // Math symbols normalized: $\ge 80g$ -> ≥ 80g, $\approx 1,700$ -> ≈ 1,700
    expect(screen.getAllByText(/≥ 80g/).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/≈ 1,700/).length).toBeGreaterThan(0);
  });
});
