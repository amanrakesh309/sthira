import type { ProviderEntry, ProviderModelEntry } from '../types/sthira';

/** Retained for future evaluation work; not displayed as task recommendations. */
export function modelGuidance(model: ProviderModelEntry) {
  const name = `${model.model_identifier} ${model.display_name}`.toLowerCase();
  if (/embed|bge-|whisper|rerank/.test(name)) {
    return { label: 'Specialist model', reason: 'This looks like an embedding, ranking, or audio model. It may not support Sthira’s text workflows.' };
  }
  if (/reasoner|reasoning|thinking|deepseek-r1|qwq|(?:^|[/\s])o[134](?:[-:\s]|$)/.test(name)) {
    return { label: 'Try for planning', reason: 'Its name indicates a reasoning model, which may suit breaking a goal into steps. This is a suggestion based on its name, not a measured quality or speed rating.' };
  }
  if (/sonar|perplexity/.test(name) && model.web_search_capability === 'native') {
    return { label: 'Try for research', reason: 'This model family focuses on search, and its catalog is configured for native grounding. Check its sources; this hint does not verify search availability or accuracy.' };
  }
  if (/gemma|llama|mistral|qwen|claude|gpt|gemini|deepseek|phi[\d-]/.test(name)) {
    return { label: 'Try for drafting', reason: 'This looks like a general text model family. Try it for writing and summarizing before choosing it for your workflows. Model variants, hardware, and runtime settings affect results.' };
  }
  return { label: 'Explore this model', reason: 'We do not have enough information to suggest a task for this model. Try a small example and review the result before choosing a workflow default.' };
}

/** Explain capacity without treating context size as a model quality score. */
export function modelContextGuidance(model: ProviderModelEntry): string {
  if (model.context_window <= 0) {
    return 'The provider has not reported a context limit. Check the setting in your AI app before working with long documents.';
  }
  if (model.context_window < 16384) {
    return 'For this catalog context limit, keep requests focused and work with shorter excerpts. Larger documents may need to be split into sections.';
  }
  if (model.context_window < 65536) {
    return 'This catalog context limit offers room for document excerpts and follow-up questions. Keep the source material focused, since instructions, conversation history, and the reply also use context.';
  }
  return 'This larger catalog context limit may accommodate longer documents or several source excerpts together. The effective limit depends on the configuration in your AI app; instructions, conversation history, and the reply also use context.';
}

export function connectionLocation(provider: ProviderEntry, model: ProviderModelEntry) {
  if (/(?:[-:]cloud)(?:$|[-:])/.test(model.model_identifier.toLowerCase())) return 'Cloud model';
  try {
    const host = new URL(provider.base_url).hostname;
    if (['localhost', '127.0.0.1', '[::1]'].includes(host)) return 'Via local app';
  } catch { /* An older invalid URL must remain editable. */ }
  return 'Remote connection';
}

export function validConnectionUrl(value: string) {
  try {
    const url = new URL(value);
    return ['http:', 'https:'].includes(url.protocol) && !url.username && !url.password && !url.search && !url.hash;
  } catch { return false; }
}
