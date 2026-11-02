import { describe, expect, it, vi } from 'vitest';
import { getProjectHistory } from '../api/ipc';
import type { ProjectHistoryDto } from '../types/sthira';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke, Channel: class {} }));

describe('Rust project history IPC mapping', () => {
  it('preserves narrative/provenance/timestamps and only links evidence known to be artifact IDs', async () => {
    const raw: ProjectHistoryDto = {
      segments: [
        { attribution: 'user_recorded', text: 'The user recorded a purpose.', evidence_ids: ['purpose-id'] },
        { attribution: 'source_backed', text: 'A source supports a finding.', evidence_ids: ['node-id'] },
        { attribution: 'ai_interpretation', text: 'An inferred shift.', evidence_ids: ['proposal-id'] },
      ],
      milestones: [
        { id: 'purpose-id', timestamp_ms: 1000, milestone_type: 'initial_goal', attribution: 'user_recorded', title: 'Initial goal', description: 'The recorded goal.', evidence_ids: ['purpose-id'] },
        { id: 'node-id', timestamp_ms: 2000, milestone_type: 'source_finding', attribution: 'source_backed', title: 'Source finding', description: 'A verified quote.', evidence_ids: ['node-id'] },
        { id: 'event-id', timestamp_ms: 3000, milestone_type: 'substantive_edit', attribution: 'user_recorded', title: 'Note updated', description: 'A substantive revision.', evidence_ids: ['artifact-id'] },
      ],
    };
    invoke.mockResolvedValueOnce(raw);
    const result = await getProjectHistory('project-a');
    expect(invoke).toHaveBeenLastCalledWith('cmd_get_project_history', { projectId: 'project-a' });
    expect(result.narrative_segments.map((segment) => segment.content)).toEqual(raw.segments.map((segment) => segment.text));
    expect(result.narrative_segments.map((segment) => segment.origin)).toEqual(['user_recorded', 'source_backed', 'ai_inferred']);
    expect(result.narrative_segments.map((segment) => segment.is_ai_interpretation)).toEqual([false, false, true]);
    expect(result.milestones.map((milestone) => milestone.attribution)).toEqual(['you_recorded', 'supported_by_source', 'you_recorded']);
    expect(result.milestones.map((milestone) => milestone.occurred_at)).toEqual([1000, 2000, 3000]);
    expect(result.milestones.map((milestone) => milestone.artifact_id)).toEqual([null, null, 'artifact-id']);
    expect(result.milestones[1].evidence_ids).toEqual(['node-id']);
    expect(result.milestones[1].description).toBe('A verified quote.');
  });

  it('retains compatibility with existing view-model responses', async () => {
    const legacy = { project_id: 'project-a', narrative_segments: [{ content: 'Existing recorded text.', is_ai_interpretation: false }], milestones: [] };
    invoke.mockResolvedValueOnce(legacy);
    expect(await getProjectHistory('project-a')).toEqual(legacy);
  });
});
