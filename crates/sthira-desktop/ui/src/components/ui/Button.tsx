import React from 'react';

export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'destructive' | 'outline';
export type ButtonSize = 'sm' | 'md' | 'lg' | 'icon';

export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  icon?: React.ReactNode;
  iconPosition?: 'left' | 'right';
  loading?: boolean;
}

export const Button: React.FC<ButtonProps> = ({
  children,
  variant = 'secondary',
  size = 'md',
  icon,
  iconPosition = 'left',
  loading = false,
  className = '',
  disabled,
  ...props
}) => {
  const baseClasses =
    'inline-flex items-center justify-center font-medium transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--sthira-focus)] disabled:opacity-50 disabled:pointer-events-none select-none';

  const sizeClasses: Record<ButtonSize, string> = {
    sm: 'text-xs h-8 px-3 rounded-[var(--sthira-radius-sm)] gap-1.5',
    md: 'text-sm h-10 px-4 rounded-[var(--sthira-radius-sm)] gap-2 min-h-[40px]',
    lg: 'text-base h-11 px-5 rounded-[var(--sthira-radius-md)] gap-2.5 min-h-[44px]',
    icon: 'h-9 w-9 p-0 rounded-[var(--sthira-radius-sm)] justify-center',
  };

  const variantClasses: Record<ButtonVariant, string> = {
    primary:
      'bg-[var(--sthira-primary)] text-[var(--sthira-on-primary)] hover:bg-[var(--sthira-primary-hover)] active:bg-[var(--sthira-primary-hover)] shadow-xs border border-transparent',
    secondary:
      'bg-[var(--sthira-surface)] text-[var(--sthira-text)] hover:bg-[var(--sthira-surface-muted)] border border-[var(--sthira-border)] active:bg-[var(--sthira-surface-muted)] shadow-2xs',
    ghost:
      'bg-transparent text-[var(--sthira-text-muted)] hover:text-[var(--sthira-text)] hover:bg-[var(--sthira-surface-muted)] border border-transparent',
    outline:
      'bg-transparent text-[var(--sthira-text)] hover:bg-[var(--sthira-surface-muted)] border border-[var(--sthira-border-strong)]',
    destructive:
      'bg-[var(--sthira-danger)] text-white hover:opacity-90 active:opacity-100 border border-transparent',
  };

  return (
    <button
      className={`${baseClasses} ${sizeClasses[size]} ${variantClasses[variant]} ${className}`}
      disabled={disabled || loading}
      {...props}
    >
      {loading ? (
        <span className="w-4 h-4 border-2 border-current border-t-transparent rounded-full animate-spin" />
      ) : (
        <>
          {icon && iconPosition === 'left' && <span className="shrink-0">{icon}</span>}
          {children && <span>{children}</span>}
          {icon && iconPosition === 'right' && <span className="shrink-0">{icon}</span>}
        </>
      )}
    </button>
  );
};
