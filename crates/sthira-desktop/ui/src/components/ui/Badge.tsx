import React from 'react';

export type BadgeVariant =
  | 'default'
  | 'sage'
  | 'amber'
  | 'slate'
  | 'purple'
  | 'danger'
  | 'outline';

export interface BadgeProps extends React.HTMLAttributes<HTMLSpanElement> {
  variant?: BadgeVariant;
  icon?: React.ReactNode;
}

export const Badge: React.FC<BadgeProps> = ({
  children,
  variant = 'default',
  icon,
  className = '',
  ...props
}) => {
  const baseClasses =
    'inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-xs font-medium tracking-wide transition-colors';

  const variantClasses: Record<BadgeVariant, string> = {
    default:
      'bg-[var(--sthira-surface-muted)] text-[var(--sthira-text-muted)] border border-[var(--sthira-border)]',
    sage:
      'bg-[var(--sthira-primary-tint)] text-[var(--sthira-primary)] border border-[#CCDCCC] dark:border-[#2B472A]',
    amber:
      'bg-[var(--sthira-warning-bg)] text-[var(--sthira-warning-text)] border border-[var(--sthira-border)]',
    slate:
      'bg-[#E4EBF2] dark:bg-[#172738] text-[#2B4C6F] dark:text-[#7BB6F0] border border-[#CDE0EE] dark:border-[#263E57]',
    purple:
      'bg-[#F3EFF8] dark:bg-[#251D2F] text-[#694291] dark:text-[#C5A7E8] border border-[#E4D7F2] dark:border-[#3D2E4E]',
    danger:
      'bg-[#FDF0ED] dark:bg-[#2D1B1A] text-[var(--sthira-danger)] border border-[#F8D2CC] dark:border-[#522927]',
    outline:
      'bg-transparent text-[var(--sthira-text-muted)] border border-[var(--sthira-border-strong)]',
  };

  return (
    <span className={`${baseClasses} ${variantClasses[variant]} ${className}`} {...props}>
      {icon && <span className="shrink-0 text-[10px]">{icon}</span>}
      <span>{children}</span>
    </span>
  );
};
