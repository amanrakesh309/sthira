import { useEffect, useState } from 'react';
import { defaultWorkflowSettings, getWorkflowSettings, WORKFLOW_SETTINGS_CHANGED, type WorkflowId, type WorkflowSetting } from '../api/workflowSettings';

export function useWorkflowSettings() {
  const [settings, setSettings] = useState(defaultWorkflowSettings);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [reload, setReload] = useState(0);
  useEffect(() => {
    let active = true;
    const updates: Partial<Record<WorkflowId, WorkflowSetting>> = {};
    const changed = (event: Event) => {
      const { workflow, setting } = (event as CustomEvent<{ workflow: WorkflowId; setting: WorkflowSetting }>).detail;
      updates[workflow] = setting;
      setSettings(previous => ({ ...previous, [workflow]: setting }));
    };
    window.addEventListener(WORKFLOW_SETTINGS_CHANGED, changed);
    setLoading(true); setError(null);
    getWorkflowSettings().then(value => { if (active) setSettings({ ...value, ...updates }); })
      .catch(reason => { if (active) setError(String(reason)); })
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; window.removeEventListener(WORKFLOW_SETTINGS_CHANGED, changed); };
  }, [reload]);
  return { settings, loading, error, retry: () => setReload(value => value + 1) };
}
