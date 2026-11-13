import { useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { useDialogFocus } from '../hooks/useDialogFocus';

interface Props {
  kind: 'page' | 'project';
  onCreate: (name: string, purpose: string) => Promise<void>;
  onClose: () => void;
}

export function WorkspaceCreationDialog({ kind, onCreate, onClose }: Props) {
  const [name, setName] = useState('');
  const [purpose, setPurpose] = useState('');
  const [pending, setPending] = useState(false);
  const [error, setError] = useState('');
  const submitting = useRef(false);
  const panel = useRef<HTMLDivElement>(null);
  const input = useRef<HTMLInputElement>(null);
  const close = () => { if (!submitting.current) onClose(); };
  useDialogFocus(true, panel, close, { initialFocus: input });

  return createPortal(
    <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/40 p-6" onMouseDown={event => { if (event.target === event.currentTarget) close(); }}>
      <div ref={panel} role="dialog" aria-modal="true" aria-labelledby="workspace-creation-title" className="w-full max-w-md rounded-2xl border border-sthira-border bg-sthira-surface p-6 text-sthira-text shadow-xl">
        <h2 id="workspace-creation-title" className="mb-4 text-lg font-semibold">New {kind === 'page' ? 'Page' : 'Project'}</h2>
        <form className="space-y-4" onSubmit={async event => {
          event.preventDefault();
          if (!name.trim() || submitting.current) return;
          submitting.current = true;
          setPending(true);
          setError('');
          try { await onCreate(name.trim(), purpose.trim()); }
          catch (failure) { setError(failure instanceof Error ? failure.message : `Couldn’t create this ${kind}. Try again.`); }
          finally { submitting.current = false; setPending(false); }
        }}>
          <label className="block text-sm">{kind === 'page' ? 'Title' : 'Project name'}
            <input ref={input} required disabled={pending} value={name} onChange={event => setName(event.target.value)} className="mt-2 block min-h-11 w-full rounded-lg border border-sthira-border bg-sthira-canvas px-3 text-sthira-text" />
          </label>
          {kind === 'project' && <label className="block text-sm">Purpose (optional)
            <textarea disabled={pending} value={purpose} onChange={event => setPurpose(event.target.value)} className="mt-2 block w-full rounded-lg border border-sthira-border bg-sthira-canvas p-3 text-sthira-text" />
          </label>}
          {error && <p role="alert" className="text-sm text-sthira-text">{error}</p>}
          <div className="flex justify-end gap-2">
            <button type="button" disabled={pending} onClick={close} className="min-h-11 rounded-lg px-4 text-sm hover:bg-sthira-surface-muted">Cancel</button>
            <button type="submit" disabled={pending || !name.trim()} className="min-h-11 rounded-lg bg-sthira-primary px-4 text-sm font-semibold text-sthira-on-primary disabled:opacity-50">{pending ? 'Creating…' : `Create ${kind}`}</button>
          </div>
        </form>
      </div>
    </div>, document.body
  );
}
