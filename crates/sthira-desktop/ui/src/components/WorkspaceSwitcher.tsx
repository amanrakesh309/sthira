import React, { useEffect, useId, useRef, useState } from 'react';
import {
  Check,
  ChevronDown,
  HardDrive,
  Plus,
  UserPlus,
  Users,
  AlertCircle,
} from 'lucide-react';
import {
  WORKSPACE_ROLE_LABELS,
  type OwnershipTransferView,
  type WorkspaceSummary,
} from '../api/workspaces';
import type { GoogleSession } from './AccountModal';

interface WorkspaceSwitcherProps {
  activeWorkspace: WorkspaceSummary | null;
  workspaces: WorkspaceSummary[];
  incomingTransfers: OwnershipTransferView[];
  session: GoogleSession | null;
  loading?: boolean;
  onSelectPersonal: () => void;
  onSelectWorkspace: (workspace: WorkspaceSummary) => void;
  onOpenCreate: () => void;
  onOpenJoin: () => void;
  onOpenAccount: () => void;
  onRefreshWorkspaces?: () => void;
}

export const WorkspaceSwitcher: React.FC<WorkspaceSwitcherProps> = ({
  activeWorkspace,
  workspaces,
  incomingTransfers,
  session,
  loading = false,
  onSelectPersonal,
  onSelectWorkspace,
  onOpenCreate,
  onOpenJoin,
  onOpenAccount,
  onRefreshWorkspaces,
}) => {
  const [open, setOpen] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const menuId = useId();

  useEffect(() => {
    if (!open) return;
    const handleOutside = (event: MouseEvent) => {
      if (
        menuRef.current?.contains(event.target as Node) ||
        triggerRef.current?.contains(event.target as Node)
      ) {
        return;
      }
      setOpen(false);
    };
    const handleKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault();
        event.stopPropagation();
        setOpen(false);
        triggerRef.current?.focus();
        return;
      }
      const items = Array.from(
        menuRef.current?.querySelectorAll<HTMLButtonElement>('[role="menuitem"]:not(:disabled)') || [],
      );
      if (items.length === 0) return;
      const idx = items.indexOf(document.activeElement as HTMLButtonElement);
      if (event.key === 'ArrowDown') {
        event.preventDefault();
        items[(idx + 1) % items.length]?.focus();
      } else if (event.key === 'ArrowUp') {
        event.preventDefault();
        const previous = idx <= 0 ? items.length - 1 : idx - 1;
        items[previous]?.focus();
      } else if (event.key === 'Home') {
        event.preventDefault();
        items[0]?.focus();
      } else if (event.key === 'End') {
        event.preventDefault();
        items[items.length - 1]?.focus();
      }
    };
    document.addEventListener('mousedown', handleOutside);
    document.addEventListener('keydown', handleKey, true);
    return () => {
      document.removeEventListener('mousedown', handleOutside);
      document.removeEventListener('keydown', handleKey, true);
    };
  }, [open]);

  const pendingTransferCount = incomingTransfers.length;

  return (
    <div className="relative px-3 pb-2">
      <button
        ref={triggerRef}
        type="button"
        data-testid="workspace-switcher-trigger"
        aria-label={
          activeWorkspace
            ? `Workspace switcher: ${activeWorkspace.name} (${WORKSPACE_ROLE_LABELS[activeWorkspace.myRole]})`
            : 'Workspace switcher: Personal'
        }
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? menuId : undefined}
        onClick={() => {
          const next = !open;
          setOpen(next);
          if (next) onRefreshWorkspaces?.();
        }}
        className="w-full min-h-[40px] flex items-center justify-between gap-2 px-3 py-2 rounded-xl border border-sthira-border bg-sthira-surface hover:bg-sthira-canvas text-xs text-sthira-text transition-colors cursor-pointer"
      >
        <span className="flex items-center gap-2 min-w-0">
          {activeWorkspace ? (
            <Users className="w-3.5 h-3.5 shrink-0 text-sthira-primary" />
          ) : (
            <HardDrive className="w-3.5 h-3.5 shrink-0 text-sthira-text-muted" />
          )}
          <span className="truncate font-medium">
            {activeWorkspace ? activeWorkspace.name : 'Personal'}
          </span>
          {activeWorkspace && (
            <span className="shrink-0 rounded px-1.5 py-0.5 text-[10px] font-medium border border-sthira-border text-sthira-text-muted">
              {WORKSPACE_ROLE_LABELS[activeWorkspace.myRole]}
            </span>
          )}
        </span>
        <span className="flex items-center gap-1 shrink-0">
          {pendingTransferCount > 0 && (
            <span
              className="rounded-full bg-sthira-primary px-1.5 py-0.5 text-[10px] font-semibold text-sthira-on-primary"
              title="Pending ownership transfer"
            >
              {pendingTransferCount}
            </span>
          )}
          <ChevronDown className="w-3.5 h-3.5 text-sthira-text-muted" />
        </span>
      </button>

      {open && (
        <div
          ref={menuRef}
          id={menuId}
          role="menu"
          aria-label="Switch workspace"
          data-testid="workspace-switcher-menu"
          className="sthira-popover absolute left-3 right-3 top-full mt-1 z-50 rounded-xl border border-sthira-border bg-sthira-surface p-1.5 shadow-xl space-y-1 text-xs"
        >
          <button
            type="button"
            role="menuitem"
            aria-label="Switch to Personal"
            onClick={() => {
              setOpen(false);
              onSelectPersonal();
            }}
            className="w-full min-h-[38px] flex items-center justify-between gap-2 px-2.5 py-1.5 rounded-lg hover:bg-sthira-surface-muted text-left text-sthira-text cursor-pointer"
          >
            <span className="flex items-center gap-2 min-w-0">
              <HardDrive className="w-3.5 h-3.5 shrink-0 text-sthira-text-muted" />
              <span className="min-w-0">
                <span className="block font-medium truncate">Personal</span>
                <span className="block text-[10px] text-sthira-text-muted">Local vault · offline</span>
              </span>
            </span>
            {!activeWorkspace && <Check className="w-3.5 h-3.5 shrink-0 text-sthira-primary" />}
          </button>

          {workspaces.length > 0 && (
            <div className="pt-1 border-t border-sthira-border space-y-0.5" aria-label="Team workspaces">
              <p className="px-2.5 py-1 text-[10px] font-semibold uppercase tracking-wider text-sthira-text-subtle">
                Team workspaces
              </p>
              {workspaces.map((ws) => {
                const isSelected = activeWorkspace?.workspaceId === ws.workspaceId;
                const hasIncoming = incomingTransfers.some((t) => t.workspaceId === ws.workspaceId);
                return (
                  <button
                    key={ws.workspaceId}
                    type="button"
                    role="menuitem"
                    aria-label={`Switch to ${ws.name} (${WORKSPACE_ROLE_LABELS[ws.myRole]})`}
                    onClick={() => {
                      setOpen(false);
                      onSelectWorkspace(ws);
                    }}
                    className="w-full min-h-[38px] flex items-center justify-between gap-2 px-2.5 py-1.5 rounded-lg hover:bg-sthira-surface-muted text-left text-sthira-text cursor-pointer"
                  >
                    <span className="flex items-center gap-2 min-w-0">
                      <Users className="w-3.5 h-3.5 shrink-0 text-sthira-primary" />
                      <span className="min-w-0">
                        <span className="block font-medium truncate">{ws.name}</span>
                        <span className="block text-[10px] text-sthira-text-muted">
                          {WORKSPACE_ROLE_LABELS[ws.myRole]}
                          {hasIncoming ? ' · Ownership pending' : ''}
                        </span>
                      </span>
                    </span>
                    {isSelected && <Check className="w-3.5 h-3.5 shrink-0 text-sthira-primary" />}
                  </button>
                );
              })}
            </div>
          )}

          {loading && (
            <p className="px-2.5 py-1 text-[11px] text-sthira-text-muted" role="status">
              Checking workspaces…
            </p>
          )}

          {session?.status === 'unverified' && (
            <div className="mx-1 rounded-lg border border-sthira-border bg-sthira-surface-muted px-2.5 py-1.5 text-[11px] text-sthira-text-muted flex items-start gap-1.5">
              <AlertCircle className="w-3.5 h-3.5 shrink-0 mt-0.5" />
              <div className="space-y-1">
                <span>Session verification paused; Personal stays available offline.</span>
                <button
                  type="button"
                  role="menuitem"
                  onClick={() => {
                    setOpen(false);
                    onOpenAccount();
                  }}
                  className="underline font-medium text-sthira-text cursor-pointer block"
                >
                  Open Account to retry
                </button>
              </div>
            </div>
          )}

          <div className="pt-1 border-t border-sthira-border space-y-0.5">
            <button
              type="button"
              role="menuitem"
              onClick={() => {
                setOpen(false);
                onOpenCreate();
              }}
              className="w-full min-h-[38px] flex items-center gap-2 px-2.5 py-1.5 rounded-lg hover:bg-sthira-surface-muted text-left text-sthira-text font-medium cursor-pointer"
            >
              <Plus className="w-3.5 h-3.5 shrink-0 text-sthira-primary" />
              <span>Create workspace</span>
            </button>
            <button
              type="button"
              role="menuitem"
              onClick={() => {
                setOpen(false);
                onOpenJoin();
              }}
              className="w-full min-h-[38px] flex items-center gap-2 px-2.5 py-1.5 rounded-lg hover:bg-sthira-surface-muted text-left text-sthira-text font-medium cursor-pointer"
            >
              <UserPlus className="w-3.5 h-3.5 shrink-0 text-sthira-primary" />
              <span>Join workspace</span>
            </button>
          </div>
        </div>
      )}
    </div>
  );
};
