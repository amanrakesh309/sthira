import { parsePageBlock, type PageBlock } from './pageBlocks';

/** Read compatibility for retired UI fences. Opening a page never writes its source. */
function blockMarkdown(block: PageBlock): string {
  let content: string;
  const known = new Set(['version', 'kind', 'widget_type']);
  if (block.kind === 'plan') {
    ['title', 'phase_number', 'milestones', 'prerequisites', 'completed'].forEach(key => known.add(key));
    content = `### ${block.title}\n\n${block.milestones.map((task, index) => `- [${block.completed.includes(index) ? 'x' : ' '}] ${task}`).join('\n')}`;
    if (block.prerequisites.length) content += `\n\n**Prerequisites**\n${block.prerequisites.map(item => `- ${item}`).join('\n')}`;
    if (block.data.phase_number !== undefined) content += `\n\nPhase: ${String(block.data.phase_number)}`;
  } else if (block.kind === 'comparison') {
    ['criteria', 'aspect_a', 'aspect_b', 'verdict'].forEach(key => known.add(key));
    content = `### ${block.criteria}\n\n**Option A:** ${block.aspect_a}\n\n**Option B:** ${block.aspect_b}\n\n**Assessment:** ${block.verdict}`;
  } else {
    ['question', 'options', 'recommended', 'selected'].forEach(key => known.add(key));
    content = `### ${block.question}\n\n${block.options.map(option => `- ${option}`).join('\n')}`;
    if (block.recommended) content += `\n\n**Suggested:** ${block.recommended}`;
    if (block.selected) content += `\n\n**Selected:** ${block.selected}`;
  }
  // Preserve other saved data as ordinary text, rather than hiding it in a card.
  for (const [key, value] of Object.entries(block.data)) {
    if (known.has(key)) continue;
    content += `\n\n**${key.replace(/_/g, ' ')}:** ${typeof value === 'string' ? value : JSON.stringify(value)}`;
  }
  return content;
}

export function plainPageMarkdown(source: string): string {
  const lines = source.split('\n');
  const output: string[] = [];
  for (let index = 0; index < lines.length; index++) {
    const opening = lines[index].match(/^ {0,3}(`{3,}|~{3,})([^\r]*)$/);
    const xml = lines[index].match(/^<widget:(plan|comparison|decision)>\s*$/);
    if (!opening && !xml) { output.push(lines[index]); continue; }
    let end = index + 1;
    while (end < lines.length) {
      if (opening) {
        const close = lines[end].match(/^ {0,3}(`{3,}|~{3,})\s*$/);
        if (close && close[1][0] === opening[1][0] && close[1].length >= opening[1].length) break;
      } else if (lines[end].trim() === `</widget:${xml![1]}>`) break;
      end++;
    }
    if (end === lines.length) { output.push(...lines.slice(index)); break; }
    const language = opening ? opening[2].trim() : `widget:${xml![1]}`;
    const block = parsePageBlock(language, lines.slice(index + 1, end).join('\n'));
    output.push(block ? blockMarkdown(block) : lines.slice(index, end + 1).join('\n'));
    index = end;
  }
  return output.join('\n');
}
