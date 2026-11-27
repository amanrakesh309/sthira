interface GuideTopic {
  title: string;
  location: string;
  text: string;
  target: string;
  fallback?: string;
}

const overview = {
  capture: {
    title: 'Capture an idea',
    location: 'Home → Capture',
    text: 'Type a thought, then choose Capture to save it as a page. You can also start with New → New Note.',
    target: 'capture', fallback: 'home',
  },
  pages: {
    title: 'Open and edit pages',
    location: 'All Files → Pages',
    text: 'Open a page to edit it. Changes save as you work. Use Open another document to keep multiple pages available in tabs.',
    target: 'editor', fallback: 'files',
  },
  projects: {
    title: 'Organize a project',
    location: 'Home → Your projects',
    text: 'Open a project for its brief and contents. Add a new page or attach an existing one. A project can hold pages, objects, and canvases.',
    target: 'projects', fallback: 'files',
  },
  resume: {
    title: 'Return to work',
    location: 'Home → Pick up where you left off',
    text: 'Open a recent page or canvas from Home. For a factual recap, use Resume from a project or page.',
    target: 'resume', fallback: 'home',
  },
  search: {
    title: 'Find saved work',
    location: 'Sidebar → Search workspace',
    text: 'Search pages and projects by name or content. Use the arrow keys to select a result and Enter to open it.',
    target: 'search',
  },
  connections: {
    title: 'Explore connections',
    location: 'Page toolbar → Connections',
    text: 'See references and backlinks, preview a linked page, or add a reference you can follow later.',
    target: 'connections', fallback: 'files',
  },
  objects: {
    title: 'Work with objects',
    location: 'All Files → Objects',
    text: 'Use object types for information you want to filter or compare, such as books or tasks. Collections show pages of the same type together.',
    target: 'files',
  },
  graph: {
    title: 'See the knowledge graph',
    location: 'Sidebar → Knowledge Graph',
    text: 'Filter or search saved items. Select a node to inspect it, double-click to open it, or drag it to adjust the layout.',
    target: 'graph-toolbar', fallback: 'graph',
  },
  canvas: {
    title: 'Arrange ideas on a canvas',
    location: 'Sidebar → Spatial canvas',
    text: 'Choose a saved page to put it on the canvas, or use Add Note to create a page and open it in the editor. Drag cards to arrange them.',
    target: 'canvas',
  },
  memory: {
    title: 'Save shared memory',
    location: 'Sidebar → Memory',
    text: 'Save context you want to carry across sessions. Memory has its own scope, sources, and revision history. Agents use it when the run grants access.',
    target: 'memory',
  },
  agent: {
    title: 'Give the agent a goal',
    location: 'Sidebar → Agent',
    text: 'Choose a scope and model, then describe the outcome. Review each proposal before it changes a page or project.',
    target: 'agent',
  },
  review: {
    title: 'Review proposals and tasks',
    location: 'Sidebar → Agent tasks',
    text: 'Follow active runs, stop work, or reopen a proposal. Apply the changes you want to keep. Dismiss removes the proposal from review.',
    target: 'tasks',
  },
  research: {
    title: 'Research and plan',
    location: 'Sidebar → Explore & Plan',
    text: 'Research a question, turn findings into a plan, or manage trackers and digests. Review the sources and enable web research only when you want external evidence.',
    target: 'research',
  },
  chat: {
    title: 'Talk through a document',
    location: 'Page toolbar → Document conversation',
    text: 'Ask questions about the open page. Enable suggested edits when you want a draft change to review.',
    target: 'editor', fallback: 'files',
  },
  models: {
    title: 'Connect models and search',
    location: 'Settings → AI & Models',
    text: 'Connect a provider, then enable the workflows you want to use. Web research also needs a search provider. Capture and editing work without AI setup.',
    target: 'models-settings', fallback: 'settings',
  },
  templates: {
    title: 'Choose a template',
    location: 'Settings → Templates',
    text: 'Preview or customize a reusable structure for Research, Plan, or Capture. A workflow default applies to future runs.',
    target: 'settings',
  },
  migration: {
    title: 'Bring in existing notes',
    location: 'Settings → Workspace & Data → Migration',
    text: 'Preview supported files before selecting which pages to import. Review warnings about links or formats, then check the saved import receipt.',
    target: 'settings',
  },
  external: {
    title: 'Connect an external agent',
    location: 'Settings → Agent connections',
    text: 'Share a Personal project or selected shared pages with an agent. Review suggestions in Sthira and disconnect from Settings.',
    target: 'agents-settings', fallback: 'settings',
  },
  vault: {
    title: 'Manage your local vault',
    location: 'Settings → Workspace & Data',
    text: 'Manage the workspace vault and import files here. Keep a backup of the vault before resetting it.',
    target: 'settings',
  },
  shortcuts: {
    title: 'Keyboard shortcuts',
    location: 'Workspace and dialogs',
    text: 'Use Ctrl+K or ⌘K to search. Use Ctrl+Tab to switch open document tabs. Escape closes Help and the tour.',
    target: 'search',
  },
} satisfies Record<string, GuideTopic>;

const focused = {
  fileBrowser: {
    title: 'Browse your saved work',
    location: 'All Files',
    text: 'Switch between Projects, Pages, and Objects to browse what you’ve saved. Use search to find a particular item.',
    target: 'files',
  },
  projectOverview: {
    title: 'Work from a project overview',
    location: 'Project → Contents',
    text: 'Use the project brief and contents to see what belongs here. Add a new page or attach an existing one.',
    target: 'current-screen',
  },
  teamOverview: {
    title: 'Find your team workspace',
    location: 'Team workspace → Overview',
    text: 'Use the workspace switcher to move between Personal and your team workspaces. Personal work stays in your local vault.',
    target: 'current-screen',
  },
  teamMembers: {
    title: 'See who can access the workspace',
    location: 'Team workspace → Members',
    text: 'Open Members to see who belongs to this workspace and their roles. Owners and Admins can invite or manage members within their permissions.',
    target: 'current-screen',
  },
  backlinks: {
    title: 'Follow a reference or backlink',
    location: 'Connections → This page',
    text: 'Outgoing references point from this page. Backlinks show the pages that refer to it. Open a preview to read a linked item and follow the trail back.',
    target: 'connection-panel', fallback: 'connections',
  },
  connectItems: {
    title: 'Add a connection',
    location: 'Connections → Add connection',
    text: 'Search for a saved item. A reference creates a link. Use Add existing in the project overview to attach a page to a project.',
    target: 'connection-panel', fallback: 'connections',
  },
  memoryScope: {
    title: 'Choose what the agent can remember',
    location: 'Memory → Preview shared context',
    text: 'Use project scope for project-specific context. Preview shared context shows eligible entries without a model. Agent runs need the matching scope and grant.',
    target: 'memory-scope', fallback: 'memory',
  },
  memorySources: {
    title: 'Refresh a memory’s sources',
    location: 'Memory → Sources at capture',
    text: 'Check source status and revision history. When the underlying page changes, create a new memory from the current source to refresh its basis.',
    target: 'memory-detail', fallback: 'memory',
  },
  reviewChange: {
    title: 'Apply or dismiss a proposal',
    location: 'Agent → Activity and proposals',
    text: 'Open Review proposal. Check the destination, sources, and proposed change. Apply saves the change. Dismiss clears it from review. If a proposal is stale, request a fresh one.',
    target: 'agent-activity', fallback: 'agent',
  },
  resumeBrief: {
    title: 'Open the factual brief',
    location: 'Project or project page → Resume',
    text: 'Open Resume to see saved facts, open work, and a suggested continuation page. The factual brief is available offline without a model.',
    target: 'resume-action', fallback: 'resume',
  },
  resumeSources: {
    title: 'Check the sources and continue',
    location: 'Resume → Continue',
    text: 'Open the saved continuation page or inspect a source. Use Summarize to request an optional model selection and review its attribution.',
    target: 'resume-pane', fallback: 'resume-action',
  },
  workflowSetup: {
    title: 'Enable a workflow',
    location: 'Settings → Workflows',
    text: 'Enable the workflows you want to use and select an enabled model. Capture and editing are available while AI is off.',
    target: 'workflows-settings', fallback: 'settings',
  },
  searchSetup: {
    title: 'Configure web search',
    location: 'Settings → Web Search',
    text: 'Choose a web-search provider if you need external sources. Each agent run requires an explicit web grant. Local research uses the saved material in scope.',
    target: 'search-settings', fallback: 'settings',
  },
} satisfies Record<string, GuideTopic>;

export const guideTopics = { ...overview, ...focused };
export type GuideTopicId = keyof typeof guideTopics;
export const guideTopicIds = Object.keys(overview) as (keyof typeof overview)[];
export type GuideTour = { title: string; topics: GuideTopicId[] };

export const guideTours: GuideTour[] = [
  { title: 'Getting started', topics: ['capture', 'pages', 'projects', 'resume'] },
  { title: 'Connections', topics: ['connections', 'backlinks', 'connectItems'] },
  { title: 'Memory', topics: ['memory', 'memoryScope', 'memorySources'] },
  { title: 'Agent review', topics: ['agent', 'review', 'reviewChange'] },
  { title: 'Resume', topics: ['resumeBrief', 'resumeSources'] },
  { title: 'AI setup', topics: ['models', 'workflowSetup', 'searchSetup'] },
  { title: 'Full workspace tour', topics: guideTopicIds.filter(id => id !== 'shortcuts') },
];

export const pageTours = {
  home: { title: 'Home', topics: ['capture', 'projects', 'resume'] },
  workspace_index: { title: 'All Files', topics: ['fileBrowser', 'pages', 'objects'] },
  focus: { title: 'Page editor', topics: ['pages', 'connections', 'chat'] },
  project: { title: 'Project', topics: ['projectOverview', 'resumeBrief'] },
  objects: { title: 'Object collection', topics: ['objects'] },
  graph: { title: 'Knowledge Graph', topics: ['graph'] },
  canvas: { title: 'Spatial canvas', topics: ['canvas'] },
  memory: { title: 'Memory', topics: ['memory', 'memoryScope', 'memorySources'] },
  agent: { title: 'Agent', topics: ['agent', 'review', 'reviewChange'] },
  explore_plan: { title: 'Explore & Plan', topics: ['research'] },
  team: { title: 'Team workspace', topics: ['teamOverview', 'teamMembers'] },
  settings: { title: 'Settings', topics: ['models', 'templates', 'vault'] },
} satisfies Record<string, GuideTour>;

export type PageTourId = keyof typeof pageTours;
