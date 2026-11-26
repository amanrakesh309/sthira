# AE5 completion, guided onboarding, Help, and coworking plan

## Status and implementation order

The owner has prioritized finishing the remaining AE5 implementation batch before guided onboarding and human coworking. UX/CW are additional product phases, separate from historical phases 1–25 and Agent Extension phases AE1–AE5. The AE5 local-export foundation and migration-agent/review completion batch are implemented. UX1 now has an optional welcome, guided tours, offline Help, and contextual explanations. Native/live acceptance gaps remain separately tracked. The owner selected shared workspaces, invitations, roles, comments and reviewed changes for the first coworking release. Simultaneous document editing comes afterward.

| Order | Phase | Intended result | Status |
| --- | --- | --- | --- |
| 1 | AE5 completion batch | Add migration-agent guidance, selective review, scalable history and representative export validation. | Implemented; native/live follow-up remains |
| 2 | UX1 — Guided onboarding and Help | Learn the application through an optional tour and searchable in-app documentation. | Dedicated walkthroughs and offline articles implemented. See UX1 acceptance below. |
| 3 | CW1 — Accounts and workspace membership | Sign in, manage an account, create/join a team workspace, and manage members and roles. | Linux pilot sign-off includes per-device revocation and workspace membership. |
| 4 | CW2 — Shared work and synchronization | Work asynchronously on shared pages/projects with comments, reviewed changes, attribution and conflict handling. | Next implementation phase. |
| Later follow-up | CW3 — Live co-editing | Edit the same document simultaneously with presence and collaborator cursors. | Planned after CW2; scheduling relative to other follow-ups remains open |

Firebase Authentication with Google sign-in is selected for the CW1 pilot. Hosting, the production shared-workspace database, and synchronization architecture are open. [Product policy](policies/product.md) governs ownership and local-first behavior. [Verification policy](policies/verification.md) governs acceptance evidence.

## AE5 completion batch before UX1

The following AE5 completion capabilities are implemented. Their reproducible acceptance scope remains:

- **Migration-agent guidance:** Use explicitly selected export inventories and bounded source excerpts to explain mappings, unsupported material and proposed organization. Model output remains an attributed suggestion; source text cannot grant filesystem access or tool permissions. Preserve the model-free manual import path when AI is disabled or unavailable.
- **Selective document review:** Choose individual eligible files from a saved preview, inspect the exact proposed content and destination, and explicitly accept the chosen selection. Recompute supported links against that selection, preserve omitted/unavailable links literally with warnings, and retain retry identity and conflict protections.
- **Scalable saved history:** List lightweight import summaries with pagination and fetch full plans/previews on demand. Avoid loading fifty complete source/preview bodies just to populate the batch picker.
- **Representative export validation:** Exercise supported Notion, Obsidian and Google Docs exports, including nested paths, Unicode, links, repeated batches, mixed unsupported files and changed-source conflicts. Publish precise fidelity limits without treating a source preset as a rich-format converter.

Verify selected-source inventory → guidance → review → import → memory/agent use → reopen/recovery with disposable vaults. Retest source immutability, attribution, stale selections, retry receipts, cancellation and guarded rollback. Native GUI/restart and live-model checks are separate acceptance assertions and stay open when unexecuted; passing engine/component tests alone must not be called complete native acceptance.

This is the implemented scope of the AE5 completion batch. Live account connectors, ZIP/media ingestion, database property mapping, additional rich formats and automatic source merging remain separately scoped migration extensions. Completing this batch does not certify those formats or integrations. See the implemented [migration contract](features/migration.md) for current support.

## UX1 — Guided onboarding and Help

Open **Help** from the sidebar to replay Getting started, take the full workspace tour, or choose a dedicated walkthrough for Connections, Memory, Agent review, Resume, or AI setup. When a control is visible, the guide highlights it. Each step gives its location so the reader can follow along from another view. New, empty desktop vaults offer an optional welcome. The local webview stores dismissal in a versioned preference. Help searches the text of its bundled articles and the short tour topics. Setup states and Connections link directly to the relevant offline articles.

### Guided tour

- Offer the welcome once on the first desktop launch, including when the vault already contains saved work. Offer a short walkthrough once per app screen. Keep **Show page tour** in the sidebar and **Show Settings tour** in Settings for replay.
- Keep the initial tour short: Capture → edit/save → project overview → returning to work. Offer separate focused tours for connections, memory, agent/review, Resume and AI setup, with pointers to tasks, CLI/MCP and the supported migration workflow.
- Explain the difference between personal saved knowledge, shared memory, AI proposals and externally sourced content. Explain model setup and review at the point they become relevant.
- Adapt steps to the current route, available controls and enabled workflows. Preserve unsaved work and return focus when the tour closes. Missing or hidden targets must have a safe fallback.
- Use read-only explanations by default. Practice content belongs in an explicitly chosen disposable example workspace; never seed or modify the owner's existing vault to demonstrate a feature. A tour must not dispatch a model, import files or enable workflows automatically.
- Record each offer locally as it opens, including when the user closes the app mid-tour. Keep previously recorded welcome preferences and avoid resetting tour history on ordinary upgrades.
- Support keyboard navigation, Escape, visible focus, screen-reader labels, reduced motion, light/dark themes and constrained desktop layouts.

### Help and documentation

- Add a persistent Help entry, a searchable documentation view and contextual help from major workflows and error/setup states. Help search searches documentation, separately from private workspace content.
- Bundle core help for offline reading without sign-in or a model. Cover getting started, pages/editor/tabs, projects, objects, connections, memory, agent/chat/tasks, proposal review/recovery, Research/Plan/Resume, AI/search setup, CLI/MCP, migration, backup and troubleshooting.
- Reuse maintained feature guides through a documented content pipeline. Keep developer-only details separate from user instructions, and keep visible labels and shortcuts consistent with the current UI.
- Include restart-tour access, a shortcut reference, a glossary, supported-format limits and privacy/sharing explanations. Diagnostic copying must be an explicit action with secrets and private content excluded.
- Use current synthetic screenshots when they help explain a feature. Identify browser fixtures versus native execution and follow the evidence policy for retained documentation images. Planned coworking content must stay visibly marked as planned until implemented.

Edit user-facing instructions in [Workspace Help](features/workspace-help.md) and the maintained Memory, migration, and proposal guides. The [article registry](../crates/sthira-desktop/ui/src/components/workspaceHelpArticles.ts) selects excerpts by their `##` headings, supplies article IDs and optional Settings destinations, and maps links between bundled articles. Vite includes the Markdown at build time. Use `#help-<article-id>` for in-app links and update the registry when a selected heading changes. The UI test suite checks that each entry resolves to a populated article.

### Acceptance

Exercise fresh setup, skip/replay, upgrade preferences, missing tour targets, dirty-editor navigation, disabled AI and offline Help search. Verify documentation routes, links and search results against the actual application. Test keyboard/focus and representative light/dark/constrained layouts; run native restart checks for saved preferences. A tour callback or browser fixture alone does not prove native persistence.

Native Linux checks completed all five dedicated tours, verified Skip across restart, and verified that a previous build's welcome preference carried over in a copied QA profile. Native Help checks exercised missing targets, offline article body search, internal links, disabled Agent setup, the Workflows route, focus restoration, and light and dark layouts at 125% scale in a 980 × 800 window. Opening Help preserved an edited page in a native QA vault. A UI integration test also preserved the draft when saving failed. Browser QA verified nested Help and Settings keyboard behavior and reduced-motion styles with network access disabled. [Live web and Inkling checks](features/synthesis-templates.md#deferred-live-checks) remain deferred.

In native Linux screen-reader QA, Orca announced the welcome, two tour steps, the Help index, a backup article, and the restored Help button after Escape. The isolated Speech Dispatcher session used eSpeak NG and reported completed playback to a null audio device.

The current native Linux build showed the welcome on a populated QA vault. The Knowledge Graph and All Files tours opened on first visit, stayed dismissed across app restart, and replayed on demand. Two screenshots of the idle graph taken 1.5 seconds apart had identical pixels. UI regressions also cover Settings replay and graph redraws during search, filtering, and dragging.

## CW1 — Accounts and workspace membership

The owner signed off on Linux per-device revocation and workspace membership for the pilot. The native loopback run verified the ambiguous-401 recovery path, and two-profile Google and Firebase QA verified live revocation. The live two-account membership run verified workspace creation, invitations, joining, roles, removal, and ownership transfer using an explicitly configured disposable store. [Linux sign-in and session checks](features/google-sign-in.md)

### Desktop flows

1. **Create.** In the workspace switcher, choose **Create workspace**, enter a name, and confirm. The service creates an empty workspace and gives the creator the Owner role in one transaction. Switch to the workspace after confirmation and offer **Invite people**. Personal remains available in the switcher and works offline without sign-in. A new workspace starts with an empty team context.
2. **Invite.** In **Workspace settings → Members**, choose an email and a role. The service issues a copyable, single-use invitation code that expires after seven days. The inviter shares it through their own channel. Authorized managers can see pending, accepted, expired, and revoked invites, revoke pending invites, or issue replacements. Email delivery and link handling can follow separately.
3. **Join.** Choose **Join workspace**, paste the code, and sign in if needed. The service checks the intended recipient against the account's service-verified email before revealing workspace details. Show the workspace name and offered role, then require **Accept invitation**. Store membership against the stable Firebase UID after acceptance. If the account is wrong, offer account switching without consuming the invite.
4. **Manage.** Show active members and their roles. Authorized managers can change roles or remove members. Members can leave a workspace. Transfer ownership to an existing member with explicit confirmation from both parties, then apply the transfer in one transaction. The outgoing Owner becomes an Admin. An Owner transfers ownership before leaving.

### Permission matrix

| Action | Owner | Admin | Editor | Viewer |
| --- | --- | --- | --- | --- |
| Open the workspace and see member display names and roles | Yes | Yes | Yes | Yes |
| Rename the workspace | Yes | Yes | No | No |
| Invite, change roles between Editor and Viewer, and remove Editors or Viewers | Yes | Yes | No | No |
| Invite, promote, demote, or remove Admins | Yes | No | No | No |
| Initiate ownership transfer | Yes | No | No | No |
| Leave the workspace | After transfer | Yes | Yes | Yes |
| Read permitted shared content in CW2 | Yes | Yes | Yes | Yes |
| Edit, comment, and submit proposals in CW2 | Yes | Yes | Yes | No |
| Approve another member's proposal in CW2 | Yes | Yes | No | No |

Each verified account may create a workspace. Roles apply per workspace. An Admin can manage Editors and Viewers and can assign either of those roles. Restrict invite recipient details to people authorized to manage the corresponding invitation. The CW2 rows are recommendations for the next phase. CW1 ships workspace metadata, invitations, and membership controls.

### Identity and authorization architecture

- Reuse Firebase Google sign-in and the Sthira device-session verifier. Keep account, device session, workspace, membership, invitation, and personal-vault identities separate. Verify the current session and the requested permission on every team operation. Apply role changes and removal on the member's next protected request. An unavailable authority blocks team mutations and leaves the personal vault usable.
- Define a persistence interface for workspaces, members, invites, and activity before selecting a production database. Use in-memory tests and a disposable durable QA implementation to exercise transactions and restarts. Hosting and production persistence remain open.
- Generate high-entropy invite codes and store only their digests. Rate-limit attempts and keep codes out of logs. Consume an invitation and create membership atomically after rechecking its expiry, recipient, and the issuer's current authority to grant the offered role. Revoke or replace outstanding invitations when the issuer loses that authority. Duplicate accepts must yield one membership and preserve existing roles. An already consumed invitation cannot restore a removed membership.
- Keep membership and permission state server-authoritative and validate updates at commit. Use idempotent creation and acceptance so network retries cannot duplicate a workspace or membership. Audit invite creation and revocation, acceptance, role changes, removal, and transfer with actor and workspace identity.
- Treat existing AE3 connection tokens as scoped agent/service credentials, separate from human sessions. Later shared-workspace grants must be explicitly bound to the workspace, issuing member and permitted actions. Do not turn the current owner-local CLI/MCP commands into remote authenticated APIs by merely adding login.

### Build sequence

1. Define the service operations, permission rules, and storage contract. Exercise create/join, authorization, invitation state, and concurrent changes in deterministic service tests.
2. Build the workspace switcher and Create/Join flows on Linux. Add Members, invitation management, role changes, removal, and ownership transfer.
3. Run native Linux QA with isolated vaults and keyrings, then a two-account run against Sthira Auth QA when both test accounts are available. Keep live web Research and Inkling 2 Mini deferred.

### Acceptance

Use two distinct verified accounts and two independent workspaces. Have A create a workspace, invite B, and confirm that B joins only after accepting. Verify both memberships survive app and service restart. Change B's role and verify its permissions on the next request. Remove B while signed in and confirm access is denied and the workspace disappears on refresh. Use the other workspace to verify isolation and ownership transfer. Service tests cover the four roles, wrong-account acceptance, expiration, revocation, duplicate accepts, revoked device sessions, and last-owner protection. Compare the personal vault before and after membership operations. Shared content, cross-workspace search, history, and agent enforcement belong to CW2 acceptance.

Native Linux acceptance passed all six membership checks with both loopback identities and two distinct live Google accounts against Sthira Auth QA. The rerun completed both Google sign-ins, verified invite and role enforcement, confirmed that removal blocked access and reuse of the invite, and confirmed that ownership transfer survived app and service restart. Personal remained usable offline and the owner vault checksum matched. The shared-workspace database and hosting decisions remain open. Live web Research and Inkling 2 Mini remain deferred.

## CW2 — Shared work and synchronization

CW2 starts with asynchronous collaboration on shared pages in the Linux app. The CW2 local shared-page pilot is implemented: create or publish a reviewed page copy → discuss it → propose an edit → review and apply → see the accepted revision on another client. Shared project grouping and scoped shared Agent/MCP contributions remain later CW2 work; simultaneous co-editing belongs separately in CW3.

### Shared pages

- **Create in a team workspace** or **Publish a copy** of an explicitly selected personal page. Copy preview runs locally before upload, shows destination, team visibility, reviewed content, provenance, and explicit omissions/redactions (`sthira://` / `#/sthira/` links, blocked schemes, local vault paths, internal project/epistemic IDs), and verifies the source did not change between preview and publish. The personal original remains untouched in the user's Personal vault. Shared copies receive stable `shp_*` IDs and independent server revision history (within a 48 KiB UTF-8 Markdown cap per payload).
- Shared pages use CommonMark and GFM. The service checks link and image destinations, including definitions inside blockquotes and lists. Page copies report omitted raw HTML in the preview. Use Markdown for formatting shared pages. Original provenance is preserved separately from the publisher, revision author, and reviewer.
- Published pages currently use workspace-wide active-member visibility enforced on every service request. Adding shared pages to team projects (shared project grouping) remains later CW2 work.
- Owners and Admins have **Archive** and **Restore** controls: archived pages are excluded from active lists unless requested, remain inspectable in history, and refuse new threads, replies, proposals, or approvals until restored. Account removal preserves attribution (`authorDisplayName`, `reviewerDisplayName`, and provenance) and team revisions.

### Comments and activity

- Page-level comment threads attach to a shared page and record `baseRevisionNumber` when started, remaining accessible across later revisions without text-range anchoring.
- Owners, Admins, and Editors can start threads, reply, and resolve or reopen threads they started; Owners and Admins can moderate any thread (`Resolve`/`Reopen`). Viewers read threads without write controls. Discussions are stored separately from document revisions, review decisions, and evidence.
- In-app **Activity** provides a workspace-scoped, membership-checked cursor feed for page publications, copies, archive/restore events, threads, replies, moderation, and proposal lifecycle decisions without email or push.

### Change review and permissions

| Action | Owner or Admin | Editor | Viewer |
| --- | --- | --- | --- |
| Read shared pages, comments and revision history | Yes | Yes | Yes |
| Create a shared page or publish an explicitly reviewed copy | Yes | Yes | No |
| Post or reply to comments and resolve own threads | Yes | Yes | No |
| Submit, revise, or withdraw an authored proposal | Yes | Yes | No |
| Request changes, reject or apply another member's proposal | Yes | No | No |
| Archive and restore shared pages | Yes | No | No |

- Authoring uses a private draft in `DocumentEditor` (`Propose edit` or `Revise & resubmit`). A submitted proposal records `baseRevisionNumber`, proposed title/content, author, submission count, and summary. `SharedProposalReview` shows submitted base vs proposed and current page vs proposed views alongside conflict warnings and decision history. Reviewers can **Apply change**, **Request changes**, or **Reject**; authors can **Withdraw proposal** or revise/resubmit.
- Every post-publication edit requires review, and reviewers cannot approve, reject, or request changes on their own proposal (`self_review_blocked`). Single-reviewer workspaces require adding another Owner or Admin to review that reviewer's own proposals.
- Applying or reviewing a proposal (`Apply change`, `Request changes`, or `Reject`) binds to `expectedSubmissionNumber` from the rendered review view and runs inside one database `IMMEDIATE` transaction that rechecks reviewer role/membership and device session validity (`SessionRecheck`), page existence/non-archived state, proposal eligibility, exact submission match (`409 stale_submission` if replaced, preserving any typed review note until refreshed), and `current_revision_number == base_revision_number`. It writes the new revision, updated page head, review decision, activity event, and scoped idempotency receipt (including submission number in the payload digest and lost-response replay) together. Competing stale proposals keep their drafts and require explicit **Rebase & revise** before resubmitting; Markdown content across client preview sanitization and direct service writes enforces the same CommonMark/GFM destination policy (`http:`/`https:`/`mailto:`/`#fragment` allowed; inline, reference-style, autolink, image, and raw HTML links/vault refs sanitized in copy preview and rejected on direct writes).
- Scoped shared Agent/MCP participation remains later CW2 work and stays distinct from human review in the UI.

### Synchronization and offline behavior

- Shared content uses the server-authoritative store (`sthira-session-service` with explicit `--enable-shared-content` / `STHIRA_ENABLE_SHARED_CONTENT=1`) with immutable revisions, monotonic version numbers, bounded cursor pagination, and manual **Refresh** / tab-return refresh.
- Team reads, publishing, commenting, and reviewing require a verified connection and active membership/device session. **Personal** remains offline-first and stored strictly separately from team data (`sthira.db` and vault Markdown files do not store shared pages). Unsent shared drafts are keyed strictly by verified device session account binding (`read_verified_session_account_uid`, verified via `/v1/devices` when missing or stale while preserving keyring credentials across temporary outages), workspace ID, and draft key (`shared-drafts/u-<sha256(uid)>/<workspace_id>/`) with serialized flushes across **Back**, tab/workspace navigation, and `WindowEvent::CloseRequested`, explicit outdated-base comparison & **Rebase onto rev Y** gating when a target page advances before resubmit, and explicit **Submit** / **Resubmit** after reconnect—never an automatic background write queue or durable offline shared-page replica.
- Membership removal (`AccessLost`) or device session revocation clears active team views, drops team access on next check, and purges cached local drafts for that workspace or account.

### Shared database decisions and acceptance

The production shared-workspace database and hosting decisions remain open; CW2 QA uses a transactional SQLite store (`schema_migrations` v2) co-located behind the `SharedContentStore` storage boundary when explicitly opted in. Before selecting and deploying the production database and host, evaluate a managed/self-hosted transactional relational store guaranteeing:

| Guarantee area | Production requirement |
| --- | --- |
| **Transactional authorization & apply** | Single-transaction membership/role/session check alongside atomic revision increment, review decision, activity write, and scoped idempotency receipt (including lost-response replay). |
| **Workspace isolation** | Tenant/workspace-scoped queries and foreign/composite keys preventing cross-workspace reads, mutations, or ID enumeration (`404` on foreign workspace IDs). |
| **Backup & restore** | Automated backups, defined RPO/RTO targets, and verified restore drills covering membership, revisions, discussions, and decisions together. |
| **Retention & cleanup** | Explicit policies for archived pages, removed-member attribution preservation vs account deletion, audit log retention, and client-side local draft cleanup. |
| **Versioned migrations** | Additive, forward-versioned schema migrations (`v1` → `v2` and onward) plus clean data portability out of the QA adapter. |

The current CW2 build passed the two-account native Linux journey with loopback OIDC/Firebase. The run verified nested Markdown reference redaction and direct-write rejection, submission-bound review, exact Activity destinations for a comment, a proposal submission, and an approval, workspace and removed-member isolation, and A → B → A switching in one profile. Immediate Back, normal window close, outdated-base rebase, and restart persistence passed. The owner vault checksum matched.

Automated service and UI regressions covered late draft reads and edits during a pending save. An earlier post-cleanup Linux release build also passed its smoke check. Production database and hosting acceptance, shared project grouping, scoped shared Agent/MCP, live Sthira Auth QA, and cross-platform checks remain open. Live web Research and Inkling 2 Mini remain deferred.

## CW3 — Live co-editing follow-up

After the first shared-workspace release, add simultaneous document editing, presence and collaborator cursors. Evaluate a CRDT-based editor synchronization design compatible with TipTap; [Yjs/Hocuspocus](https://tiptap.dev/docs/hocuspocus/getting-started/overview) is a candidate, not a selected backend.

Prototype schema compatibility, persistence, offline merging, reconnect, per-user undo, authorization/revocation and Markdown round-trip before adoption. Editor merge convergence does not establish business-level correctness: revisions, source fingerprints, AI proposal bases, attribution, relationship changes and guarded undo still need their own integration design. Keep page synchronization and other domain operations consistent without bypassing their validation.

## Open decisions for shared-workspace deployment

| Decision | Required outcome |
| --- | --- |
| Service hosting | Choose hosted, self-hosted or a supported combination; document operations, costs and data location before deployment. |
| Identity provider and sign-in methods | Firebase Authentication with Google sign-in and the signed-off Linux per-device session pilot. Live two-account Linux workspace membership acceptance passed. Additional account lifecycle controls remain open. |
| Sharing and permissions | Define workspace/project/page visibility, role and review permissions, invitation/owner rules and private-data boundaries. |
| Shared-data architecture | Define the server authority, device caches, supported resource types, sync protocol, recovery and offline policy. |
| Privacy and retention | Define team-content ownership, member removal, export/deletion, backups and team AI credential usage. Do not promise end-to-end encryption or remote cache erasure without an implemented design. |

The Linux pilot and CW2 service contracts can be tested with disposable stores while these decisions remain open. Billing, public sharing, enterprise SSO, a public social/community network and unrestricted agent delegation are not prerequisites for the first coworking release. Existing native, live and platform acceptance gaps remain tracked in [known limitations](known-limitations.md).
