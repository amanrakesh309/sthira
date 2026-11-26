import type { HelpArticleId } from './workspaceHelpArticles';

export const OPEN_HELP_ARTICLE = 'sthira:open-help-article';

export function HelpTopicButton({ article, label }: { article: HelpArticleId; label: string }) {
  return <button type="button" className="workspace-help-link" onClick={() => {
    window.dispatchEvent(new CustomEvent(OPEN_HELP_ARTICLE, { detail: article }));
  }}>{label}</button>;
}
