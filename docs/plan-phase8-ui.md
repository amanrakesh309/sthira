Sequential Execution Slices (Phase 8A $\rightarrow$ 8D)

### Slice 8A: The Tauri v2 IPC Bridge & Live Event Stream

1. **State Management:** Initialize `sthira_core::Engine` with `TieredModelProvider` (`llama3.2:latest` local router + `gemma4:31b-cloud` primary synthesis + `gpt-oss:120b-cloud` deep review) and register it via `.manage(engine)`.
2. **Command Surface (`commands.rs`):** Expose exact 1-to-1 commands:
* `cmd_route_input(raw: String) -> Result<RouterPlan, String>`
* `cmd_apply_plan(plan: RouterPlan, approved: bool) -> Result<ApplyOutcome, String>`
* `cmd_get_home_state() -> Result<HomeState, String>`
* `cmd_get_resume_brief(project_id: String, level: u8) -> Result<Brief, String>`
* `cmd_get_resume_pane_hint(project_id: String) -> Result<ResumePaneHint, String>`
* `cmd_get_project_history(project_id: String) -> Result<ProjectHistory, String>`
* `cmd_get_artifact_context(artifact_id: String) -> Result<ArtifactContext, String>`
* `cmd_save_artifact(artifact_id: String, content_md: String, project_id: Option<String>) -> Result<Artifact, String>`
* `cmd_decide_proposal(proposal_id: String, decision: String) -> Result<String, String>`
* `cmd_revert_change_set(change_set_id: String) -> Result<String, String>`
* `cmd_promote_to_project(artifact_ids: Vec<String>, name: String) -> Result<String, String>`
* `cmd_canvas_ops(...)` (Load/move/update `canvas_items` with `ChangeSetId` support).


3. **Event Bridge (`events.rs`):** Spawn a background task subscribing to `engine.subscribe()` and emitting `sthira://core-event` to the webview whenever `ArtifactChanged`, `ExtractionFinished`, `ProposalsReady`, or `ChangeSetApplied` fires.
4. **Verification Gate 8A:** Write a Rust integration test in `crates/sthira-desktop/tests/ipc_bridge_test.rs` invoking every command handler directly against a temp vault and verifying round-trip JSON serialization.

---

### Slice 8B: Home View & Universal Input Preview (`HomeView.tsx`)

1. **Universal Input Bar (`UniversalInputBar.tsx`):**
* Debounce user input (`250ms`) or trigger on `Enter` to call `cmd_route_input(raw)` (read-only, powered by `llama3.2:latest` + fast-path regex).


* Display a subtle **Intent Preview Card** directly below the input before mutating the knowledge base:


* `TransientQa`: Renders the answer inline with an optional `[Save as AI Note]` button (which saves with `Authorship::AiGenerated`).


* `CaptureNote` / `CaptureSource`: Shows `[Saving Note...]` and optional project link suggestion.


* `StartResearch` / `GuidedProjectInit`: Shows a preview of the proposed project workspace with `[Create Workspace]` and `[Just Save as Note]` buttons.


* `ResumeProject`: Shows the matched project's Level 1 Quick Brief and jumps straight into Focus View on `Enter`.


* `AmbiguousClarify`: Renders 3 calm pill buttons: `[Ask Question]`, `[Capture Thought]`, or `[Start Research]`.






2. **Active Threads Zone (`threads`):** Render up to 3 active project cards ordered by `last_substantive_at`, displaying the project name, its top open question or unfinished draft breadcrumb, and a 1-click `[Resume ->]` action.


3. **Loose Captures Zone (`loose_captures`):** Chronological list of unfiled thoughts with a `[Promote to Project]` action wired to `cmd_promote_to_project`.



---

### Slice 8C: Focus View, Contextual Resume Pane & History Drawer (`FocusView.tsx`)

1. **Direct Continuation Landing:** Opening a project loads its primary continuation artifact directly in the Markdown editor. If `is_unfinished_draft == true`, highlight the unfinished line calmly at the cursor.


2. **Contextual Resume Pane (`ContextualResumePane.tsx`):**
* Calls `cmd_get_resume_pane_hint(project_id)`. Auto-opens if `Show { reason: ColdReturn }` (or gap $\ge 48\text{h}$); remains collapsed into a subtle top-right pill (`[Brief]`) otherwise.


* **Default View (Level 1 Brief):** Displays Purpose, Recent Progress, and Unresolved Question in $\le 75$ words with clickable artifact citation chips.


* **On-Demand Toggle (`[Expand Review]`):** Calls `cmd_get_resume_brief(project_id, 2)` to display Established Findings, Recorded Decisions, Rejected Approaches, and Open Questions—each rendered with `<EpistemicBadge origin="{claim.origin}"/>` and verbatim quote tooltips.




3. **Epistemic Margin & Ambient Proposals:**
* Alongside the editor, display extracted `epistemic_nodes` for the current artifact (showing verbatim quote highlights on hover).


* Render at most 2 pending `ai_proposals` (`AmbientProposalPill.tsx`) with `[Accept]` and `[Dismiss]` buttons. Accepting a proposal triggers `UndoToast.tsx` (`"Link added — [Undo]"` calling `cmd_revert_change_set`).




4. **Project History Tab (`ProjectHistoryDrawer.tsx`):**
* Renders the **Segmented Evolution Narrative** at the top (visually distinguishing `Recorded` segments from `AiInterpretation` segments).


* Renders the **Milestone Timeline** below with explicit attribution labels (`"You recorded"`, `"Supported by source"`, `"Sthira inferred"`) and clickable artifact links.





---

### Slice 8D: Flexible Spatial Canvas View (`CanvasView.tsx`)

1. **2D Pan/Zoom Workspace:** Render `canvas_items` (`artifact` cards, `text` labels, and `frame` groups) with drag-and-drop positioning saved via `cmd_canvas_ops`.


2. **Strict Spatial vs. Semantic Separation:**
* Dragging two cards next to each other updates `(x, y)` coordinates ONLY—it must never draw a semantic link line or call link creation.


* Semantic links (`artifact_links`) are rendered as explicit connector lines with relationship labels (`supports`, `contradicts`, `extends`) only when created by the user or accepted from an `ai_proposal`.




3. **Reversible Canvas Layouts:** Any batch layout change creates a `canvas_layout_revisions` entry and surfaces the `[Undo]` toast wired to `cmd_revert_change_set`.



---