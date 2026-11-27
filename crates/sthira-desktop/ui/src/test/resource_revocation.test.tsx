import { useEffect } from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { SharedPagesPanel } from '../components/SharedPagesPanel';
const api = vi.hoisted(() => ({ list: vi.fn(), page: vi.fn(), proposal: vi.fn(), flush: vi.fn() }));
vi.mock('../api/workspaces', async original => ({
  ...await original<typeof import('../api/workspaces')>(),
  listSharedPages: api.list, getSharedPage: api.page, getProposalDetails: api.proposal,
}));
vi.mock('../api/liveCollab', () => ({subscribeCollabEvents: async () => () => {}}));
vi.mock('../components/RichDocumentRenderer', () => ({ RichDocumentRenderer: ({content}: {content: string}) => <article>{content}</article> }));
vi.mock('../components/LiveSharedDraftEditor', () => ({ LiveSharedDraftEditor: ({baseContentMd}: {baseContentMd: string}) => <article aria-label="Live editor">{baseContentMd}</article> }));
vi.mock('../components/SharedProposalReview', () => ({ SharedProposalReview: ({details}: {details: {proposedContentMd: string}}) => <article aria-label="Proposal review">{details.proposedContentMd}</article> }));
vi.mock('../components/SharedPageDraftEditor', () => ({ sharedDraftKeyForMode: () => 'fixture-draft', SharedPageDraftEditor: ({registerBeforeLeave}: {registerBeforeLeave: (guard: (() => Promise<boolean>) | null) => void}) => {
  useEffect(() => {registerBeforeLeave(api.flush);return () => registerBeforeLeave(null);},[registerBeforeLeave]);
  return <article aria-label="Private draft editor">PRIVATE_UNDEBOUNCED_DRAFT</article>;
} }));
const ok = (data: unknown) => ({outcome: 'ok', data});
const page = {pageId:'page',workspaceId:'ws',title:'Private page',currentRevisionNumber:1,entryMode:'new',provenance:{originKind:'userAuthored'},omittedSummary:[],publisherDisplayName:'A',lastEditorDisplayName:'A',unresolvedThreadCount:0,pendingProposalCount:1};
const detail = {page,currentRevision:{title:'Private page',contentMd:'PRIVATE_BODY',revisionNumber:1,authorDisplayName:'A'},revisions:[],threads:[],proposals:[{proposalId:'proposal',pageId:'page',summary:'Suggestion',status:'pending',submissionCount:1,baseRevisionNumber:1}],revisionsNextCursor:null};
const props = {workspaceId:'ws',workspaceName:'Lab',currentUid:'b',myRole:'editor' as const,authorDisplayName:'B',refreshSignal:0,pendingOpenRequest:{pageId:'page'},onAccessLost:vi.fn(),onSessionChange:vi.fn(),onErrorNotice:vi.fn()};
describe('Resource revocation in open shared views', () => {
  beforeEach(() => {vi.resetAllMocks();api.flush.mockResolvedValue(true);api.list.mockResolvedValue(ok({items:[page],nextCursor:null}));api.page.mockResolvedValue(ok(detail));api.proposal.mockResolvedValue(ok({proposal:{proposalId:'proposal',pageId:'page',agentProposal:false},proposedContentMd:'PRIVATE_PROPOSAL'}));});
  it('rechecks an open live editor and evicts its body after access is denied', async () => {
    const view=render(<SharedPagesPanel {...props}/>);
    await screen.findByText('PRIVATE_BODY');fireEvent.click(screen.getByRole('button',{name:'Edit together'}));
    expect(await screen.findByLabelText('Live editor')).toHaveTextContent('PRIVATE_BODY');
    api.page.mockResolvedValue({outcome:'error',code:'page_not_found',message:'Unavailable'});
    api.list.mockResolvedValue(ok({items:[],nextCursor:null}));
    view.rerender(<SharedPagesPanel {...props} refreshSignal={1}/>);
    await waitFor(() => expect(screen.queryByLabelText('Live editor')).not.toBeInTheDocument());
    expect(screen.queryByText('PRIVATE_BODY')).not.toBeInTheDocument();
    expect(screen.getByRole('alert')).toHaveTextContent('no longer available');
  });
  it('preserves an authorized live editor through unrelated refreshes', async () => {
    const view=render(<SharedPagesPanel {...props}/>);
    await screen.findByText('PRIVATE_BODY');fireEvent.click(screen.getByRole('button',{name:'Edit together'}));
    await screen.findByLabelText('Live editor');const reads=api.page.mock.calls.length;
    view.rerender(<SharedPagesPanel {...props} refreshSignal={1}/>);
    await waitFor(()=>expect(api.page.mock.calls.length).toBeGreaterThan(reads));
    expect(screen.getByLabelText('Live editor')).toHaveTextContent('PRIVATE_BODY');
  });
  it('flushes a private draft before revocation unmounts its pending debounce', async () => {
    let finish!: (saved: boolean) => void;
    api.flush.mockImplementation(() => new Promise<boolean>(resolve => {finish=resolve;}));
    const view=render(<SharedPagesPanel {...props}/>);
    await screen.findByText('PRIVATE_BODY');fireEvent.click(screen.getByRole('button',{name:'Propose edit'}));
    await screen.findByLabelText('Private draft editor');
    api.page.mockResolvedValue({outcome:'error',code:'page_not_found',message:'Unavailable'});
    api.list.mockResolvedValue(ok({items:[],nextCursor:null}));
    view.rerender(<SharedPagesPanel {...props} refreshSignal={1}/>);
    await waitFor(()=>expect(api.flush).toHaveBeenCalledOnce());
    expect(screen.getByLabelText('Private draft editor')).toBeInTheDocument();
    finish(true);
    await waitFor(()=>expect(screen.queryByLabelText('Private draft editor')).not.toBeInTheDocument());
    expect(screen.queryByText('PRIVATE_UNDEBOUNCED_DRAFT')).not.toBeInTheDocument();
  });
  it('clears a cached proposal and its source page after permission loss', async () => {
    const view=render(<SharedPagesPanel {...props}/>);
    await screen.findByText('PRIVATE_BODY');fireEvent.click(screen.getByRole('button',{name:'Open review'}));
    await screen.findByText('PRIVATE_PROPOSAL');
    api.proposal.mockResolvedValue({outcome:'error',code:'proposal_not_found',message:'Unavailable'});
    api.list.mockResolvedValue(ok({items:[],nextCursor:null}));
    view.rerender(<SharedPagesPanel {...props} refreshSignal={1}/>);
    await waitFor(()=>expect(screen.queryByText('PRIVATE_PROPOSAL')).not.toBeInTheDocument());
    expect(screen.queryByText('PRIVATE_BODY')).not.toBeInTheDocument();
  });
});
