export type PageBlockKind = 'plan' | 'comparison' | 'decision';
type Fields = Record<string, unknown>;
export type PageBlock =
  | { kind: 'plan'; data: Fields; title: string; milestones: string[]; prerequisites: string[]; completed: number[] }
  | { kind: 'comparison'; data: Fields; criteria: string; aspect_a: string; aspect_b: string; verdict: string }
  | { kind: 'decision'; data: Fields; question: string; options: string[]; recommended?: string; selected?: string };

const text = (value: unknown): value is string => typeof value === 'string' && value.trim().length > 0 && value.length <= 8000;
const strings = (value: unknown, max: number): value is string[] => Array.isArray(value) && value.length <= max && value.every(text);
export const pageBlockKind = (language: string): PageBlockKind | null => {
  const kind = language.replace(/^widget:/, '');
  return language.startsWith('widget:') && ['plan', 'comparison', 'decision'].includes(kind) ? kind as PageBlockKind : null;
};

/** Model output is data only. Unknown versions/shapes retain their original code fence. */
export function parsePageBlock(language: string, raw: string): PageBlock | null {
  const kind = pageBlockKind(language);
  if (!kind || raw.length > 64000) return null;
  try {
    const data: Fields = JSON.parse(raw);
    if (!data || typeof data !== 'object' || Array.isArray(data) || (data.version !== undefined && data.version !== 1)) return null;
    if (kind === 'plan' && text(data.title) && strings(data.milestones, 40) && data.milestones.length > 0 && (data.prerequisites === undefined || strings(data.prerequisites, 20))) {
      const completed = data.completed ?? [];
      if (!Array.isArray(completed) || !completed.every(i => Number.isInteger(i) && i >= 0 && i < (data.milestones as string[]).length)) return null;
      return { kind, data, title: data.title, milestones: data.milestones, prerequisites: (data.prerequisites as string[]) ?? [], completed };
    }
    if (kind === 'comparison' && text(data.criteria) && text(data.aspect_a) && text(data.aspect_b) && text(data.verdict)) {
      return { kind, data, criteria: data.criteria, aspect_a: data.aspect_a, aspect_b: data.aspect_b, verdict: data.verdict };
    }
    if (kind === 'decision' && text(data.question) && strings(data.options, 12) && data.options.length >= 2 && new Set(data.options).size === data.options.length) {
      if (data.recommended !== undefined && !data.options.includes(data.recommended as string)) return null;
      if (data.selected !== undefined && !data.options.includes(data.selected as string)) return null;
      return { kind, data, question: data.question, options: data.options, recommended: data.recommended as string | undefined, selected: data.selected as string | undefined };
    }
  } catch { /* Malformed or incomplete output is displayed literally. */ }
  return null;
}
