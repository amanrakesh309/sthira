import React from 'react';

export interface CardProps extends React.HTMLAttributes<HTMLDivElement> {
  variant?: 'surface' | 'raised' | 'canvas' | 'outline';
  interactive?: boolean;
}

export const Card: React.FC<CardProps> = ({
  children,
  variant = 'surface',
  interactive = false,
  className = '',
  ...props
}) => {
  const baseClasses =
    'rounded-[var(--sthira-radius-lg)] border transition-all duration-150';

  const variantClasses = {
    surface:
      'bg-[var(--sthira-surface)] border-[var(--sthira-border)] shadow-2xs',
    raised:
      'bg-[var(--sthira-surface-raised)] border-[var(--sthira-border)] shadow-2xs',
    canvas:
      'bg-[var(--sthira-canvas)] border-[var(--sthira-border)]',
    outline:
      'bg-transparent border-[var(--sthira-border)]',
  };

  const interactiveClasses = interactive
    ? 'hover:border-[var(--sthira-border-strong)] hover:shadow-xs cursor-pointer'
    : '';

  return (
    <div
      className={`${baseClasses} ${variantClasses[variant]} ${interactiveClasses} ${className}`}
      {...props}
    >
      {children}
    </div>
  );
};
