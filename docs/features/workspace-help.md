# Workspace Help

Sthira can open these articles without a network connection or a model. Help searches the bundled article text and tour topics.

## Workspace basics

The desktop app offers its welcome once, then offers a short tour the first time you open each app screen. Use **Show page tour** in the sidebar or **Show Settings tour** in Settings to replay it. Close or skip a tour and continue where you left off.

From **Home**, choose **Capture** and save a thought, or use **New → New Note**. Open a saved page to edit it. The editor shows save progress and offers recovery when a save fails.

Use **All Files** to browse projects, pages, and objects. Open a project to see its brief, members, and references. Attaching a page adds it to the project. Adding a reference creates a link you can follow later.

Use the sidebar **Knowledge Graph** to inspect relationships. Search and filters keep the graph’s layout steady. Drag a graph node to move it and its connection lines. Open **Spatial canvas** to arrange cards for a project or idea.

On an empty canvas, choose a saved page from the picker. On a canvas with cards, choose **Add existing page** to open the picker again. Use **Add Note** to create a saved page and open it in the editor. Choose **Canvas** from the editor to return to the same board. Drag cards to arrange them, or use **Auto-Align** for a grid.

- [Pages and tabs](#help-pages)
- [References and backlinks](#help-connections)
- [Resume a project](#help-resume)

## Pages and tabs

Open a page to edit it. The title and body use the same save state. Use **Open another document** to open another page in a tab.

Ctrl-click or ⌘-click a page to open it in another tab. The tab strip appears when more than one document is open. Closing a tab removes it from the strip. Sthira keeps the saved page.

Switching away from unsaved work attempts a save first. If the save fails, return to the draft and retry before leaving.

## References and backlinks

Open a saved page and choose **Connections**. Switch between page and project scope where available. Outgoing references link from the current item. Backlinks show what points back to it.

Select a connection to preview the target. Use the preview trail to step back, or open the linked page at full size. Use **Add connection** to search for a saved item.

To place an existing page in a project, use **Add existing** from the project overview. To connect two pages without changing project membership, add a reference.

## Resume a project

Choose **Resume** in a project overview or a page opened within a project. The factual brief shows saved references, unfinished work, and a continuation page. It works offline without model setup.

Open a source to check the underlying page. Choose **Summarize** or **Refresh summary** only when you want a model-generated selection. Check its model label and source links separately from the saved facts.

If a model is unavailable, continue from the factual brief. Change summary preferences under **Settings → Workflows → Resume**.

## Agent, document chat, and tasks

In **Agent**, choose a project or workspace scope, enter a goal, and choose **Start working**. Use the tasks button beside Agent to follow progress, reopen results, or stop a run while working elsewhere.

Choose a row in **Recent goals** to open a saved run. In **Activity and proposals**, expand a row to see what the agent did and what the tool returned. Use **Review proposal** for proposed changes.

Use document chat for questions about an open page. Allow **Suggest a document edit** when you want a reviewable edit. Open **View agent activity** to follow its run when available.

- [Review a proposal and recover from a failed change](#help-review)
- [Troubleshoot setup and failed runs](#help-troubleshooting)

## Research and Plan

In **Explore & Plan**, use **Research** to synthesize evidence and **Plan** to draft next steps. Enable the workflow and assign a model before starting. Choose a template for the run when enabled, or choose no template.

For agent Research, inspect the evidence basis, source links, and numbered citations in **Review proposal**. If an external search finds no sources, check whether the result used local evidence. Apply the result to save it. Use the saved research page as the source for a follow-up Plan to preserve the link.

- [Set up models and search](#help-models)
- [Review and recovery](#help-review)

## Set up models and search

1. Open **Settings → AI & Models**. Add an enabled provider connection, then select or discover its model.
2. Open **Workflows**. Enable the features you want and choose a model or inherit the workspace default.
3. For external Research, configure a provider under **Web Search**. Enable web research explicitly when you need external evidence.

Use Capture and the editor with AI disabled. A model connection and a web-search connection serve different purposes. The search provider receives search terms. The selected model receives the approved research context.

- [Troubleshoot setup and failed runs](#help-troubleshooting)
- [Privacy, scope, and sharing](#help-privacy)

## Back up and recover your vault

Close Sthira and any CLI clients using the vault before copying it. Back up the whole vault folder, including `sthira.db` and the Markdown files. Markdown copies alone omit items such as project membership, memory revisions, and run history.

On a standard desktop setup, the vault lives at `~/.sthira/vault`. If the app was launched with `STHIRA_VAULT_DIR`, use that folder. Keep a backup away from the active vault.

To check a backup, open a copy in a separate vault using `STHIRA_VAULT_DIR`. Check pages, projects, and memory before replacing any active data. Keep the original backup intact. In **Workspace & Data**, review the confirmation before using Reset Vault.

For a team workspace, use **Recovery → Create backup → Export backup**. Keep the exported file away from the service storage. To recover, select an empty team workspace, choose the file, inspect its preview, and explicitly confirm **Restore workspace**. Current target members keep their roles; credentials and invitations are excluded. Imported decisions and acknowledged drafts are read-only source records. See [Shared workspace backup and recovery](shared-workspace-backups.md).

## Troubleshoot setup and failed runs

- **A workflow is disabled.** Open Workflows, enable it, and assign an enabled model.
- **No model is available.** Check the connection and model catalog in AI & Models. For a local server, make sure its app or process is running.
- **Search failed or returned no sources.** Check Web Search settings. Review the evidence basis before applying a result. A proposal based on the local vault can still be useful.
- **A proposal is stale.** Reopen the source and request a fresh proposal. Review the new version before applying.
- **A page failed to save.** Keep the editor open, check the error, and retry. Save or copy unsaved text before attempting recovery.

- [Set up models and search](#help-models)
- [Back up and recover your vault](#help-backup)

## Privacy, scope, and sharing

Pages, personal work, and app preferences can stay local. A configured provider receives the context required for the workflow you explicitly run.

- **Saved pages** hold your knowledge. Project membership organizes which pages belong to a project.
- **Shared memory** holds curated context. Its scope, source references, and revision history determine where it can be reused.
- **AI proposals** wait for review before they change saved work. Choose Apply to keep the change or Dismiss to clear it from review.
- **Web sources** come from external pages. Their links and citations describe the evidence used for the proposal.

When giving an agent a goal, choose the smallest project scope you need. Review the workspace-memory and web grants before starting. Use Agent connections to disconnect a client you set up in Sthira.

## Shared workspaces and roles

Use the sidebar workspace switcher to move between **Personal** and any team workspaces you belong to. **Personal** stays stored in your local vault and continues to work offline without signing in.

- **Roles:** Team workspaces use four roles per workspace: **Owner**, **Admin**, **Editor**, and **Viewer**. Only the Owner can manage Admins or propose an ownership transfer. Admins can rename the workspace and invite or manage Editors and Viewers.
- **Invitations:** Managers can enter one or multiple recipient email addresses (comma- or newline-separated) with one workspace role in **Workspace settings → Members** to issue single-use codes expiring after seven days. Each invitation grants workspace access (not access to a single project). Existing members, pending invites, and invalid addresses are reported per recipient with independent **Copy code**, **Replace**, and **Revoke** actions.
- **Ownership transfer and leaving:** An Owner keeps their role until the invited member accepts ownership, after which the outgoing Owner becomes an Admin and can leave the workspace. If your access changes or you are removed, Sthira returns you to **Personal** on refresh.

### Shared pages, live drafts, comments, and review

- **Publishing & copying safely:** Open the **Pages** tab in a team workspace to create a new shared page or choose **Publish a copy** from **Personal**. Sthira shows a local preview listing team visibility, provenance, and converted/omitted private dependencies (`sthira://` links, local vault paths, internal metadata) before anything is shared, leaving your Personal original untouched.
- **Live co-editing (`Edit together`):** Owners, Admins, and Editors can open **Edit together** on any active shared page to co-edit one shared draft in real time with collaborator presence badges, labeled cursors, and per-user undo (**Undo** / **Ctrl+Z**). Live edits remain inside the shared draft—never altering the published page or polluting revision history with micro-steps—and freeze while a submission is pending review. Disconnecting pauses new edits, saves unacknowledged updates locally per verified account and workspace as **Unsynced**, and reconciles on reconnect.
- **Comments & activity:** Owners, Admins, and Editors can start page-level comment threads (recorded with the revision open at the time), reply, and resolve or reopen their own threads; Owners and Admins can moderate any thread. **Activity** shows recent shared-page, comment, review, and project discussion events and updates live over the workspace channel.
- **Proposing edits & independent review:** Edits go through **Submit for review** (from **Edit together**) or **Propose edit** → **Change review**. Publishing requires independent approval (**Owner** or **Admin** other than the submitter or any draft contributor) of the exact submitted checkpoint. **Request changes** reopens the shared draft for further edits; **Apply change** creates one attributed published revision. If the page moved to a newer revision first, the proposal marks itself stale so you can explicitly **Rebase & revise**. Owners and Admins can archive or restore shared pages.
- **Offline drafts & privacy.** Team reads and writes require an active workspace connection. Unsent drafts are stored outside your Personal vault, scoped to your account, workspace, and draft. Signing out clears regular shared-page drafts and keeps live-edit recovery for the same account after sign-in. Workspace removal or device revocation clears the affected local drafts when detected.

### Team backup and recovery

Owners and Admins use **Recovery** to save/export snapshots and preview a file for an empty target workspace. Current workspace members can inspect restored history. Exported files are unencrypted team content; keep a separate safe copy. Restore preserves target roles and never reactivates imported approvals or agent access. Saved draft text can be copied into a fresh proposal for independent review.

### Shared project grouping & discussions

- **Visibility and roles:** Team projects are visible across active workspace members and reuse workspace roles. Owners, Admins, and Editors can create projects (`name` and optional `purpose`), attach or detach shared pages, and participate in project **Discussion** threads (Viewers read; authors or Owners/Admins resolve and reopen); only Owners and Admins can rename, edit project purpose, moderate others' threads, or archive and restore projects.
- **Attaching existing pages:** Open a project and select **Add existing page** to attach active shared pages from that workspace. A page keeps one shared identity, live draft, revision history, comments, proposals, and workspace **Pages** listing across all projects it belongs to.
- **Remove from project vs. archiving:** **Remove from project** detaches only that project-page membership while keeping the page available in workspace **Pages** and any other project. **Archive project** hides the project from active lists and pauses project metadata, membership, and discussion writes while leaving its pages accessible elsewhere; **Restore project** recovers the same project and memberships. Archiving a page preserves its project memberships but hides it from active project lists until restored.

## Glossary

- **Artifact** means a saved page or other workspace item with its own identity.
- **Reference** is a link between saved items. A **backlink** is an incoming reference.
- **Membership** means an item belongs to a project.
- **Lineage** records which saved source a new item was derived from.
- **Proposal** is a suggested change waiting for you to apply or dismiss.
- **Template** is a starting structure for Capture, Research, or Plan.
- **Vault** is the folder that holds the workspace data.

## Keyboard shortcuts

| Action | Shortcut |
| --- | --- |
| Search the workspace | Ctrl+K or ⌘K |
| Cycle open document tabs | Ctrl+Tab or ⌘Tab where the host allows it |
| Cycle document tabs backward | Add Shift |
| Close the active document tab | Ctrl+W or ⌘W where the host allows it |
| Move between controls | Tab or Shift+Tab |
| Open a selected search result | Enter |
| Close Help, a tour, or an info tip | Escape |

Settings, Help, and other active dialogs handle their own keyboard controls. Close them to resume workspace shortcuts.

## External clients

On Linux, open **Settings → Agent connections**, choose an app, and select a personal project. Leave access on **Can read** for sharing context. Choose **Can read and suggest changes** to receive proposals for review.

For a team project, set **Destination → Shared workspace**. An Owner/Admin enables workspace agent access; then choose a project and its explicit pages. Read access is the default. External clients use the generated configuration; Sthira's native runs use the selected connection and your configured Agent model. Another eligible Owner/Admin must apply suggestions. Review shows the initiating member, agent, model/provider and saved source revisions. Disconnect blocks future access and pending approval.

Use **Connect** for an installed client or **Create setup** to copy a local MCP configuration. Then open a fresh session in the client and try the starter prompt. Use **Test connection** to check local access and **Disconnect** to revoke it. Advanced local clients can still use the [CLI and MCP setup](cli-and-mcp.md).
