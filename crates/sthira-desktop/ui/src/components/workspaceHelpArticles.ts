import helpGuide from '../../../../../docs/features/workspace-help.md?raw';
import memoryGuide from '../../../../../docs/features/shared-memory.md?raw';
import migrationGuide from '../../../../../docs/features/migration.md?raw';
import proposalGuide from '../../../../../docs/features/agent-workflow-proposals.md?raw';

export type HelpDestination = 'models' | 'workflows' | 'search' | 'agents' | 'vault';

export function helpSection(document: string, heading: string) {
  const lines = document.split(/\r?\n/);
  const start = lines.indexOf(`## ${heading}`);
  if (start < 0) return '';
  const end = lines.findIndex((line, index) => index > start && /^#{1,2} /.test(line));
  return lines.slice(start + 1, end < 0 ? undefined : end).join('\n').trim();
}

function article(title: string, guide = helpGuide, location?: { destination: HelpDestination; label: string }, sections = [title]) {
  return { title, location, markdown: sections.map(name => helpSection(guide, name)).filter(Boolean).join('\n\n') };
}

export const helpArticles = {
  basics: article('Workspace basics'),
  pages: article('Pages and tabs'),
  connections: article('References and backlinks'),
  memory: article('Using Memory', memoryGuide, undefined, ['Using Memory', 'Sources and lifecycle']),
  agent: article('Agent, document chat, and tasks'),
  review: article('Review and recovery', proposalGuide),
  research: article('Research and Plan'),
  resume: article('Resume a project'),
  models: article('Set up models and search', helpGuide, { destination: 'models', label: 'Open AI & Models' }),
  import: article('Supported import formats', migrationGuide, { destination: 'vault', label: 'Open Workspace & Data' }, ['Source preparation', 'Receipts, conflicts and recovery']),
  backup: article('Back up and recover your vault'),
  troubleshooting: article('Troubleshoot setup and failed runs', helpGuide, { destination: 'workflows', label: 'Open Workflows' }),
  privacy: article('Privacy, scope, and sharing'),
  glossary: article('Glossary'),
  shortcuts: article('Keyboard shortcuts'),
  cli: article('External clients', helpGuide, { destination: 'agents', label: 'Open Agent connections' }),
  workspaces: article('Shared workspaces and roles'),
};
export type HelpArticleId = keyof typeof helpArticles;
export const helpArticleIds = Object.keys(helpArticles) as HelpArticleId[];

export function isHelpArticleId(value: string): value is HelpArticleId {
  return Object.prototype.hasOwnProperty.call(helpArticles, value);
}

const articleSources: Record<string, HelpArticleId> = {
  'internal-references.md': 'connections',
  'shared-memory.md': 'memory',
  'agent-workflow-proposals.md': 'review',
  'agent-conversations-and-tasks.md': 'agent',
  'synthesis-templates.md': 'research',
  'resume.md': 'resume',
  'workflow-settings.md': 'models',
  'migration.md': 'import',
  'cli-and-mcp.md': 'cli',
  'document-tabs.md': 'pages',
};

export function getLinkedHelpArticle(href: string | undefined): HelpArticleId | undefined {
  if (!href) return;
  if (href.startsWith('#help-')) {
    const id = href.slice(6);
    return isHelpArticleId(id) ? id : undefined;
  }
  const id = articleSources[href.split('#')[0]];
  return typeof id === 'string' ? id : undefined;
}
