import type { CSSProperties } from 'react';

/** Registered colors are user data; neutral token surfaces keep their text legible in both themes. */
export function ObjectIdentity({ name, icon, color, large = false }: {
  name: string; icon?: string | null; color?: string | null; large?: boolean;
}) {
  return <span className={large ? 'object-identity object-identity-large' : 'object-identity'}
    style={{ '--object-color': color || 'var(--sthira-primary)' } as CSSProperties}>
    <span className="object-identity-icon" aria-hidden="true">{icon || '◇'}</span>
    <span>{name}</span>
  </span>;
}
