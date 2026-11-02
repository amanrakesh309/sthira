import React from 'react';

export interface EmptyStateProps {
  icon?: React.ReactNode;
  title: string;
  description?: string;
  action?: React.ReactNode;
  className?: string;
}

export const EmptyState: React.FC<EmptyStateProps> = ({
  icon,
  title,
  description,
  action,
  className = '',
}) => {
  return (
    <div
      className={`flex flex-col items-center justify-center p-8 text-center rounded-[var(--sthira-radius-lg)] border border-dashed border-[var(--sthira-border-strong)] bg-[var(--sthira-surface-raised)]/50 ${className}`}
    >
      {icon && (
        <div className="w-12 h-12 mb-3 rounded-full bg-[var(--sthira-surface-muted)] text-[var(--sthira-text-muted)] flex items-center justify-center text-xl">
          {icon}
        </div>
      )}
      <h3 className="text-base font-semibold text-[var(--sthira-text)] mb-1">{title}</h3>
      {description && (
        <p className="text-sm text-[var(--sthira-text-muted)] max-w-sm mb-4 leading-relaxed">
          {description}
        </p>
      )}
      {action && <div className="mt-1">{action}</div>}
    </div>
  );
};
