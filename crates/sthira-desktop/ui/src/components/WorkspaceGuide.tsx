import { useEffect, useId, useLayoutEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { ArrowLeft, ArrowRight, Compass, Search, X } from 'lucide-react';
import { guideTopicIds, guideTopics, guideTours, type GuideTopicId, type GuideTour, type PageTourId } from './workspaceGuideContent';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import {
  getLinkedHelpArticle, helpArticleIds, helpArticles, type HelpArticleId, type HelpDestination,
} from './workspaceHelpArticles';
import '../styles/workspace-guide.css';

const welcomeKey = 'sthira.guide.welcome.v1';
const pageTourKey = 'sthira.guide.page.';

function hasSeenGuide(key: string) {
  try {
    return window.localStorage.getItem(key) !== null;
  } catch {
    return false;
  }
}

function rememberGuide(key: string) {
  try {
    window.localStorage.setItem(key, 'seen');
  } catch {
    // Help remains available when the webview cannot store preferences.
  }
}

export const hasSeenWorkspaceWelcome = () => hasSeenGuide(welcomeKey);
export const rememberWorkspaceWelcome = () => rememberGuide(welcomeKey);
export const hasSeenPageTour = (page: PageTourId) => hasSeenGuide(pageTourKey + page);
export const rememberPageTour = (page: PageTourId) => rememberGuide(pageTourKey + page);

type Spotlight = { left: number; top: number; width: number; height: number };
type GuideMode = 'welcome' | 'help' | 'tour' | 'article';

export function WorkspaceGuide({ entry, initialArticle, initialTour, onClose, onOpenSettings }: {
  entry: 'welcome' | 'help' | 'tour';
  initialArticle?: HelpArticleId;
  initialTour?: GuideTour;
  onClose: () => void;
  onOpenSettings?: (destination: HelpDestination) => void;
}) {
  const [mode, setMode] = useState<GuideMode>(initialArticle ? 'article' : entry);
  const [articleId, setArticleId] = useState<HelpArticleId | undefined>(initialArticle);
  const article = articleId ? helpArticles[articleId] : undefined;
  const [tour, setTour] = useState(initialTour ?? guideTours[0]);
  const [step, setStep] = useState(0);
  const [query, setQuery] = useState('');
  const [openedTopic, setOpenedTopic] = useState<GuideTopicId | null>(null);
  const [spotlight, setSpotlight] = useState<Spotlight | null>(null);
  const dialog = useRef<HTMLDialogElement>(null);
  const closeButton = useRef<HTMLButtonElement>(null);
  const card = useRef<HTMLDivElement>(null);
  const headingId = useId();
  const bodyId = useId();
  const topic = guideTopics[tour.topics[step]];
  const fallback = 'fallback' in topic ? topic.fallback : undefined;
  const title = mode === 'welcome' ? 'Welcome to Sthira' : mode === 'help' ? 'Workspace help' : mode === 'article' ? article?.title : topic.title;
  const stepDescription = mode === 'welcome' || mode === 'tour' ? bodyId : undefined;

  const close = () => {
    if (entry === 'welcome') rememberWorkspaceWelcome();
    onClose();
  };
  const showHelp = () => {
    if (entry === 'welcome') rememberWorkspaceWelcome();
    setMode('help');
  };
  const openArticle = (id: HelpArticleId) => {
    setArticleId(id);
    setMode('article');
  };
  const start = (selected: typeof tour) => {
    setTour(selected);
    setStep(0);
    setMode('tour');
  };
  const finish = () => {
    if (entry === 'welcome' || entry === 'tour') close();
    else setMode('help');
  };

  useLayoutEffect(() => {
    const element = dialog.current;
    const opener = document.activeElement;
    if (!element) return;
    if (typeof element.showModal === 'function') element.showModal();
    else element.setAttribute('open', '');

    return () => {
      if (typeof element.close === 'function') element.close();
      else element.removeAttribute('open');
      if (opener instanceof HTMLElement && opener.isConnected) opener.focus({ preventScroll: true });
    };
  }, []);

  useLayoutEffect(() => {
    if (card.current) card.current.scrollTop = 0;
    closeButton.current?.focus({ preventScroll: true });
  }, [mode, step, articleId]);

  useEffect(() => {
    const escape = (event: KeyboardEvent) => {
      if (event.key !== 'Escape' || !dialog.current?.contains(document.activeElement)) return;
      event.preventDefault();
      event.stopPropagation();
      close();
    };
    window.addEventListener('keydown', escape, true);
    return () => window.removeEventListener('keydown', escape, true);
  }, [entry, onClose]);

  useEffect(() => {
    if (mode !== 'tour') return;
    const measure = () => {
      const anchors = [topic.target, fallback].flatMap(key =>
        key ? Array.from(document.querySelectorAll<HTMLElement>(`[data-guide-target="${key}"]`)) : [],
      );
      const bounds = anchors.map(element => element.getBoundingClientRect()).find(rect =>
        rect.width > 2 && rect.height > 2 && rect.left >= 0 && rect.top >= 0
        && rect.right <= window.innerWidth && rect.bottom <= window.innerHeight,
      );
      const next = bounds ? { left: bounds.left - 4, top: bounds.top - 4, width: bounds.width + 8, height: bounds.height + 8 } : null;
      setSpotlight(previous =>
        previous?.left === next?.left && previous?.top === next?.top
        && previous?.width === next?.width && previous?.height === next?.height ? previous : next,
      );
    };
    measure();
    const observer = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(measure);
    document.querySelectorAll('.sthira-sidebar, main[data-testid="center-workspace"]').forEach(element => observer?.observe(element));
    window.addEventListener('resize', measure);
    window.addEventListener('scroll', measure, true);
    return () => {
      observer?.disconnect();
      window.removeEventListener('resize', measure);
      window.removeEventListener('scroll', measure, true);
    };
  }, [mode, topic.target, fallback]);

  const terms = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
  const matchingArticles = helpArticleIds.filter(id => {
    const { title, markdown } = helpArticles[id];
    const index = `${title} ${markdown}`.toLowerCase();
    return terms.every(term => index.includes(term));
  });
  const filtered = guideTopicIds.filter(id => {
    const { title, location, text } = guideTopics[id];
    const haystack = `${title} ${location} ${text}`.toLowerCase();
    return terms.every(term => haystack.includes(term));
  });

  const target = mode === 'tour' ? spotlight : null;
  const placeBeside = target && target.left + target.width + 24 + 420 < window.innerWidth - 16;
  const placement = placeBeside
    ? { margin: 0, position: 'absolute' as const, left: target.left + target.width + 24, top: '50%', transform: 'translateY(-50%)' }
    : undefined;

  return createPortal(
    <dialog
      ref={dialog}
      className="workspace-guide-layer"
      role="dialog"
      aria-modal="true"
      aria-labelledby={headingId}
      aria-describedby={stepDescription}
      onCancel={event => { event.preventDefault(); close(); }}
      onKeyDown={event => {
        if (event.key === 'Escape') {
          event.preventDefault();
          event.stopPropagation();
          close();
        }
        if (event.key !== 'Tab') return;
        const stops = Array.from(event.currentTarget.querySelectorAll<HTMLElement>('button:not([disabled]), input:not([disabled]), a[href]'));
        const first = stops[0];
        const last = stops.at(-1);
        if (event.shiftKey && document.activeElement === first) {
          event.preventDefault();
          last?.focus();
        } else if (!event.shiftKey && document.activeElement === last) {
          event.preventDefault();
          first?.focus();
        }
      }}
    >
      <div className="workspace-guide-backdrop" aria-hidden="true" onClick={close}>
        {target && <span className="workspace-guide-spotlight" style={target} />}
      </div>
      <div ref={card} className={`workspace-guide-card workspace-guide-${mode}`} style={placement}>
        <header className="workspace-guide-heading">
          <div>
            {mode === 'welcome' && <Compass size={25} strokeWidth={1.5} aria-hidden="true" />}
            <h2 id={headingId}>{title}</h2>
          </div>
          <button key={title} ref={closeButton} type="button" className="workspace-guide-icon"
            aria-label={title ? `Close help, ${title}` : 'Close help'} aria-describedby={stepDescription} onClick={close}><X size={19} /></button>
        </header>
        {mode === 'welcome' && <>
          <p id={bodyId}>Learn how to capture an idea, edit a page, and find your work again.</p>
          <footer className="workspace-guide-actions">
            <button type="button" className="workspace-guide-quiet" onClick={showHelp}>Help</button>
            <span className="workspace-guide-spacer" />
            <button type="button" className="workspace-guide-quiet" onClick={close}>Skip</button>
            <button type="button" className="workspace-guide-primary" onClick={() => start(guideTours[0])}>Start tour <ArrowRight size={16} /></button>
          </footer>
        </>}
        {mode === 'tour' && <>
          <p className="workspace-guide-location">{topic.location}</p>
          <p id={bodyId}>{topic.text}</p>
          <footer className="workspace-guide-actions">
            <button type="button" className="workspace-guide-quiet" onClick={showHelp}>All topics</button>
            <span className="workspace-guide-step">{step + 1} of {tour.topics.length}</span>
            <button type="button" className="workspace-guide-icon" aria-label="Previous step" disabled={step === 0} onClick={() => setStep(step - 1)}><ArrowLeft size={16} /></button>
            <button type="button" className="workspace-guide-primary" onClick={() => { if (step === tour.topics.length - 1) finish(); else setStep(step + 1); }}>
              {step === tour.topics.length - 1 ? 'Done' : <>Next <ArrowRight size={16} /></>}
            </button>
          </footer>
        </>}
        {mode === 'article' && article && <>
          <div className="workspace-help-article">
            <ReactMarkdown remarkPlugins={[remarkGfm]} skipHtml components={{
              a({ href, children }) {
                const id = getLinkedHelpArticle(href);
                return id ? <a href={`#help-${id}`} onClick={event => {
                  event.preventDefault();
                  openArticle(id);
                }}>{children}</a> : <span>{children}</span>;
              },
            }}>{article.markdown}</ReactMarkdown>
          </div>
          <footer className="workspace-guide-actions">
            <button type="button" className="workspace-guide-quiet" onClick={showHelp}><ArrowLeft size={16} aria-hidden="true" /> Back to Help</button>
            <span className="workspace-guide-spacer" />
            {article.location && onOpenSettings && <button type="button" className="workspace-guide-primary" onClick={() => {
              if (article.location) onOpenSettings(article.location.destination);
            }}>{article.location.label}</button>}
          </footer>
        </>}
        {mode === 'help' && <>
          <label className="workspace-guide-search">
            <Search size={17} aria-hidden="true" />
            <input value={query} onChange={event => setQuery(event.target.value)} name="help-search" autoComplete="off" aria-label="Search help topics" placeholder="Search help topics…" />
          </label>
          {!terms.length && <section className="workspace-guide-tours" aria-label="Guided tours">
            {guideTours.map(guide => <button type="button" key={guide.title} onClick={() => start(guide)}>{guide.title}<ArrowRight size={14} aria-hidden="true" /></button>)}
          </section>}
          {matchingArticles.length > 0 && <section className="workspace-guide-topics" aria-labelledby={`${headingId}-articles`}>
            <h3 id={`${headingId}-articles`} className="workspace-help-section-title">Articles</h3>
            {matchingArticles.map(id => <div className="workspace-guide-topic" key={id}>
              <button type="button" onClick={() => openArticle(id)}><span>{helpArticles[id].title}</span><ArrowRight size={14} aria-hidden="true" /></button>
            </div>)}
          </section>}
          <section className="workspace-guide-topics" aria-label="Help topics">
            {matchingArticles.length === 0 && filtered.length === 0 && <p role="status">No help topics match your search.</p>}
            {filtered.length > 0 && <h3 className="workspace-help-section-title">Quick reference</h3>}
            {filtered.map(id => {
              const item = guideTopics[id];
              const expanded = openedTopic === id;
              const topicBodyId = `${headingId}-${id}`;
              return <div className="workspace-guide-topic" key={id}>
                <button type="button" aria-expanded={expanded} aria-controls={topicBodyId} onClick={() => setOpenedTopic(expanded ? null : id)}>
                  <span>{item.title}</span><span className="workspace-guide-location">{item.location}</span>
                </button>
                <p id={topicBodyId} hidden={!expanded}>{item.text}</p>
              </div>;
            })}
          </section>
        </>}
      </div>
    </dialog>,
    document.body,
  );
}
