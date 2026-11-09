import { Check, GitBranch, ListChecks, Scale } from 'lucide-react';
import ReactMarkdown, { defaultUrlTransform } from 'react-markdown';
import { parseEntityHref } from '../api/connections';
import type { EntityRef } from '../types/sthira';
import type { PageBlock as Block } from '../editor/pageBlocks';
import '../styles/page-blocks.css';

interface Props { block: Block; onChange?: (data: Record<string, unknown>) => void; onOpenEntity?: (entity: EntityRef) => void }

/** Shared by the editor and read-only previews. Actions only update this page's data. */
export function PageBlock({ block, onChange, onOpenEntity }: Props) {
  const update = (patch: Record<string, unknown>) => onChange?.({ ...block.data, ...patch });
  const text = (value: string) => <ReactMarkdown skipHtml urlTransform={url => parseEntityHref(url) ? url : defaultUrlTransform(url)} components={{
    p: ({ children }) => <span>{children}</span>,
    a: ({ href, children }) => {
      const entity = parseEntityHref(href);
      return entity && onOpenEntity ? <button type="button" className="connection-inline-link" onClick={() => onOpenEntity(entity)}>{children}</button> : <a href={href} target="_blank" rel="noopener noreferrer">{children}</a>;
    },
    img: ({ alt }) => <span>{alt}</span>,
  }}>{value}</ReactMarkdown>;
  const Icon = block.kind === 'plan' ? ListChecks : block.kind === 'comparison' ? Scale : GitBranch;
  return <section className="page-block" data-page-block={block.kind} aria-label={`${block.kind} block`}>
    <div className="page-block-label"><Icon size={16} aria-hidden="true" /><span>{block.kind === 'plan' ? 'Plan' : block.kind === 'comparison' ? 'Comparison' : 'Choice'}</span></div>
    {block.kind === 'plan' && <>
      <h3>{block.title}</h3>
      {block.prerequisites.length > 0 && <p className="page-block-context">Depends on: {block.prerequisites.join(' · ')}</p>}
      <ul className="page-block-tasks">{block.milestones.map((milestone, index) => <li key={index}>
        <label><input type="checkbox" checked={block.completed.includes(index)} disabled={!onChange} onChange={event => update({ completed: event.target.checked ? [...block.completed, index] : block.completed.filter(i => i !== index) })} /><span>{milestone}</span></label>
      </li>)}</ul>
      <p className="page-block-context">{block.completed.length} of {block.milestones.length} complete</p>
    </>}
    {block.kind === 'comparison' && <>
      <h3>{block.criteria}</h3>
      <div className="page-block-comparison"><div><span>Option A</span><div className="page-block-evidence">{text(block.aspect_a)}</div></div><div><span>Option B</span><div className="page-block-evidence">{text(block.aspect_b)}</div></div></div>
      <div className="page-block-verdict"><strong>Assessment</strong>{text(block.verdict)}</div>
    </>}
    {block.kind === 'decision' && <>
      <h3>{block.question}</h3>
      {onChange ? <div className="page-block-options" role="group" aria-label={block.question}>{block.options.map(option => <button key={option} type="button" aria-pressed={block.selected === option} onClick={() => update({ selected: option })}>
        {block.selected === option && <Check size={16} aria-hidden="true" />}<span>{option}</span>{block.recommended === option && <small>Suggested</small>}
      </button>)}</div> : <ul className="page-block-options-readonly">{block.options.map(option => <li key={option}>{option}{block.selected === option ? ' · Selected' : block.recommended === option ? ' · Suggested' : ''}</li>)}</ul>}
      <p className="page-block-context">{block.selected ? `Selected: ${block.selected}. Saved in this page; no separate decision is recorded.` : onChange ? 'Choose an option to keep it in this page. Suggestions need your review.' : 'No option selected. Suggestions need your review.'}</p>
      {onChange && block.selected && <button type="button" className="page-block-clear" onClick={() => { const data = { ...block.data }; delete data.selected; onChange(data); }}>Clear choice</button>}
    </>}
  </section>;
}
