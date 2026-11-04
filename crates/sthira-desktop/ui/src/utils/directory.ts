/** A display-only excerpt. The underlying Markdown and saved title are never changed. */
export function directoryExcerpt(markdown: string | null | undefined, title = ''): string {
  if (!markdown) return '';
  const withoutTitle = markdown.replace(/^\s*#\s+(.+?)(?:\r?\n|$)/, (match, heading: string) =>
    heading.trim() === title.trim() ? '' : match);
  return withoutTitle
    .replace(/```[^\n]*\n[\s\S]*?(?:```|$)/g, '')
    .replace(/!\[([^\]]*)\]\([^)]*\)/g, '$1')
    .replace(/\[([^\]]*)\]\([^)]*\)/g, '$1')
    .replace(/^\s*(?:#{1,6}\s+|>\s*|[-*+]\s+(?:\[[ xX]\]\s*)?|\d+[.)]\s+)/gm, '')
    .replace(/<[^>]*>/g, '')
    .replace(/([*_~`])\1*/g, '')
    .replace(/\s+/g, ' ').trim();
}

export function directoryDate(timestamp: number): string {
  if (!timestamp) return '—';
  const date = new Date(timestamp > 1e11 ? timestamp : timestamp * 1000);
  return date.toLocaleDateString(undefined, { month: 'short', day: 'numeric',
    year: date.getFullYear() === new Date().getFullYear() ? undefined : 'numeric' });
}
