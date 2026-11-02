import React from 'react';

export interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  icon?: React.ReactNode;
  clearable?: boolean;
  onClear?: () => void;
}

export const Input = React.forwardRef<HTMLInputElement, InputProps>(
  ({ icon, clearable, onClear, className = '', ...props }, ref) => {
    return (
      <div className="relative flex items-center w-full">
        {icon && (
          <div className="absolute left-3 flex items-center pointer-events-none text-[var(--sthira-text-muted)]">
            {icon}
          </div>
        )}
        <input
          ref={ref}
          className={`w-full h-10 min-h-[40px] rounded-[var(--sthira-radius-sm)] bg-[var(--sthira-surface)] border border-[var(--sthira-border)] text-[var(--sthira-text)] placeholder-[var(--sthira-text-subtle)] px-3 text-sm transition-all focus:outline-none focus:ring-2 focus:ring-[var(--sthira-focus)] focus:border-transparent disabled:opacity-50 disabled:bg-[var(--sthira-surface-muted)] ${
            icon ? 'pl-9' : ''
          } ${clearable ? 'pr-9' : ''} ${className}`}
          {...props}
        />
        {clearable && props.value && (
          <button
            type="button"
            onClick={onClear}
            className="absolute right-3 text-[var(--sthira-text-muted)] hover:text-[var(--sthira-text)] p-0.5 rounded-full"
            tabIndex={-1}
          >
            ✕
          </button>
        )}
      </div>
    );
  }
);

Input.displayName = 'Input';
