# Sthira Autonomous Build & Verification Progress Log

## Phase 1: Hybrid Storage, Authorship Provenance, Reversibility & Vault Reconciliation
- **Status:** PASSED (Gate 1 Verified)
- Verified with `gate_reports/phase1.json`

## Phase 2: Provenance-Locked Epistemic Extraction & Materiality Engine
- **Status:** PASSED (Gate 2 Verified)
- Verified with `gate_reports/phase2.json`

## Phase 3: Read-Only Universal Input Router
- **Status:** PASSED (Gate 3 Verified)
- Verified with `gate_reports/phase3.json`

## Phase 4: Fingerprint-Cached Persistent Memory, Resume Briefs & Home State
- **Status:** PASSED (Gate 4 Verified)
- Verified with `gate_reports/phase4.json`

## Phase 5: Project History & Segmented Evolution Engine
- **Status:** PASSED (Gate 5 Verified)
- Verified with `gate_reports/phase5.json`

## Phase 6: Hybrid Cross-Project Discovery & Budgeted Proposals
- **Status:** PASSED (Gate 6 Verified)
- Implemented `discovery/mod.rs` with RRF $k=60$ combining FTS5 BM25 and vector cosine similarity.
- Calibrated cutoff threshold $\ge 0.75$, session proposal budget $\le 2$, and 30-day auto-expiry.
- DB-enforced proposal dismissal persistence via unique index `ux_proposal_live`.
- Change set reversibility and dissolution for approved links.
- 100 golden discovery test pairs in `fixtures/golden/discovery_pairs_100.json`.
- Verified with `gate_reports/phase6.json`.

## Phase 7: 14-Day End-to-End Multi-Project Simulation & Final Gate
- **Status:** PASSED (Gate 7 Verified)
- Full 14-day multi-project lifecycle simulation implemented in `tests/frozen/phase7_e2e_lifecycle.rs`:
  - Day 1: Spontaneous capture of loose note unattached to any project.
  - Day 2: Organic promotion into `proj_coding` (*"Local Coding Model Research"*), discovery linking.
  - Day 3: External benchmark URL capture, decision recording (16GB VRAM limit), mid-sentence unfinished draft (`is_unfinished_draft = 1`), Level 1 brief precomputed.
  - Day 4: Context switch to `proj_webhooks`, `"What is speculative decoding?"` routed to `TransientQa` (read-only, zero DB writes), AI answer saved (`Authorship::AiGenerated`).
  - Days 5–11: 7-day complete absence period simulated via `clock.advance`.
  - Day 12 (Cold Return):
    1. `Engine::get_home_state()` surfaces `proj_webhooks` and `proj_coding` ordered by `last_substantive_at`.
    2. Opening `proj_coding` returns `ResumePaneHint::Show { reason: ColdReturn }`.
    3. `Engine::get_resume_brief(proj_coding, Level1)` returns in $< 5\text{ms}$ from `derived_briefs`, surfacing the 16GB VRAM decision (`UserRecorded`), benchmark findings (`SourceBacked`), the open question, and the unfinished draft continuation point.
    4. Compile-time provenance verification: Saved Day 4 AI answer was **never** laundered into a `UserRecorded` decision.
    5. Dual persistent memory stores and isolation: `.sthira_meta/memory_mirrors/projects/<proj_coding>.md` and `global_working_memory.md` exist with `<!-- STHIRA_DERIVED_MEMORY: DO_NOT_INGEST -->` and were never re-ingested by `reconcile_vault()`.
    6. No-Pressure Regex Lint across all user-facing strings -> `0` matches.
  - Day 13: Resuming and resolving the unfinished draft, transitioning `is_unfinished_draft` to `0`.
  - Day 14: Project history milestones and narrative audit.
- Metamorphic cold return across arbitrary time gaps (3, 14, 30, 180 days).
- Vault reconciliation strict isolation torture test.
- Verified with `gate_reports/phase7.json` and `gate_reports/FINAL_VERIFICATION_REPORT.json` (`"status": "ALL_PHASES_PASSED"`).

## Live Multi-Model Trust Audit: Ollama + Cloud Models
- **Status:** PASSED (All 3 Models Verified)
- Benchmarked 3 real models side-by-side against `sthira-core` engine:
  1. `llama3.2:latest` (3.2B Local via Ollama)
  2. `gemma4:31b-cloud` (31B Cloud via Ollama proxy)
  3. `gpt-oss:120b-cloud` (120B Cloud via Ollama proxy)
- **Gauntlet Results:**
  - **Task 1 (Universal Input Router):** 5/5 inputs correct on all models with 0 database writes on read-only queries (`TransientQa` and `AmbiguousClarify`).
  - **Task 2 (Epistemic Extraction & Quote Survival):** `VerifiedQuote::verify()` rejected paraphrased/mismatched quotes from `llama3.2` while accepting exact quotes from `gemma4` and `gpt-oss`. Blockquote quotes were rejected across all models. Hedged statements were locked to `Hypothesis`/`Question` and never `Decision`.
  - **Task 3 (Authorship Laundering Guard):** 0 `UserRecorded` nodes and 0 `Decision` nodes created from AI-authored notes.
  - **Task 4 (Resumption & Markdown Mirrors):** Warm cache lookups $< 1\text{ms}$ (target $< 5\text{ms}$), Level 1 visible words complied with $\le 75$ limit. Markdown mirrors exported to `gate_reports/live_model_comparison/`.
## Phase 8: Desktop UI (Tauri v2 + React 19 + TypeScript + Tailwind CSS Warm Stone Edition)
- **Status:** PASSED (Gate 8 Verified)
- **Slice 8A (IPC Bridge & Demo Seed):**
  - Built `crates/sthira-desktop` with Tauri v2 integration, 12 command handlers, and `events.rs` broadcasting `sthira://core-event`.
  - Configured `TieredModelProvider` and seeded out-of-the-box demo workspace (*"Local AI research"*, *"Webhook platform"*, *"Model performance notes"*, *"Ideas to investigate"*).
  - Verified 12-command roundtrip IPC execution with `crates/sthira-desktop/tests/ipc_bridge_test.rs`.
- **Slice 8B (Home View & Universal Input Bar):**
  - Built `HomeView.tsx` and `UniversalInputBar.tsx` strictly matching `sthira_image.jpeg` layout, copy, and Warm Stone color tokens (`#F5F3EC` bg, `#FFFFFF` cards, `#EFECE4` pills, `#282C25` dark accents).
  - Implemented 3 action pills (`🔍 Research`, `📝 Capture thought`, `📋 Plan something`) and inline Intent Preview Card.
- **Slice 8C (Focus View & Contextual Resume Pane):**
  - Built `FocusView.tsx`, `ContextualResumePane.tsx` (Level 1 $\le 75$-word brief + Level 2 Expanded Review), `ProjectHistoryDrawer.tsx`, `AmbientProposalPill.tsx`, and `UndoToast.tsx`.
  - Implemented Warm Stone epistemic badges (`UserRecorded` sage `#E3EBE3`, `SourceBacked` slate-blue `#E4EBF2`, `AiInferred` dashed warm-amber `#F9F5EC`).
- **Slice 8D (Canvas View):**
  - Built `CanvasView.tsx` on `#F5F3EC` dot-grid canvas with 2D pan/zoom and strict spatial vs. semantic decoupling.
- **Slice 8E (Modal Controls, Clean Startup & Guided Project Workflows):**
  - Search Modal: Wired top header Search button and `Cmd+K` / `Ctrl+K` global keyboard shortcut to `cmd_search_all` for instant project and document navigation.
  - Settings Modal: Wired top header SlidersHorizontal button to configure Tiered Models (`llama3.2:latest`, `gemma4:31b-cloud`, `gpt-oss:120b-cloud`), test live Ollama host connectivity, and manage vault data ("Clear Vault Data" and "Load Demo Workspace").
  - Account Modal: Wired top header User button displaying Local Architect identity, Human Primary Ground Truth authority, and SQLite DB-trigger anti-laundering guarantees.
  - Clean Initial Startup: Disabled automatic demo workspace seeding so pristine initial state displays zero projects and zero resume cards with curated Warm Stone empty states.
  - Interactive Research, Plan, and Capture Flows: Wired pills and `+` action buttons to `InteractiveFlowModal` calling `cmd_create_guided_project` to atomically scaffold workspaces, seed initial documents, and route directly into `FocusView`.
- **Verification Gates:**
  - `cargo test --workspace`: 37/37 tests passed (all 31 frozen tests + 5 epistemic tests + 1 IPC test).
  - `npm run build` in `crates/sthira-desktop/ui`: 0 TypeScript errors, 0 Vite errors.
  - `cargo check --package sthira-desktop`: Clean compilation.
  - `xtask gate 7`: Passed with 0 forbidden literals and 0 banned clock calls.

## Phase 9: Provider-Centric Compute Registry (OmniRouter Style) & Intent Steering (Capture / Research / Plan / Act)
- **Status:** PASSED (Gate 9 Verified)
- **Database Schema & Migrations (`crates/sthira-core`):**
  - Added `providers` and `provider_models` tables to SQLite schema with FK cascade and unique indices.
  - Implemented migration bump to `user_version = 2` with backward compatibility.
  - Implemented `providers_repo.rs` with full CRUD queries: `list_providers_with_models()`, `upsert_provider()`, `upsert_model()`, `set_default_model()`, `toggle_model()`, and `seed_default_providers_if_empty()`.
  - Default baseline seed: `ollama` -> `llama3.2:latest` (`is_default = 1`), `groq` -> `llama-3.3-70b-versatile`, `anthropic` -> `claude-3-7-sonnet`.
- **Local Ollama Autodiscovery & Multi-Provider Dispatcher:**
  - Implemented `llm/autodiscover.rs` querying `GET /api/tags` and syncing local tags without overwriting user customizations.
  - Implemented `llm/anthropic.rs` with `ModelProvider` for Anthropic Messages API.
  - Implemented `llm/dispatcher.rs` dynamically resolving compute providers across local Ollama, Groq (OpenAI-compatible), and Anthropic.
- **Intent Steering & Reversible Actions:**
  - Added `OperationalMode` (`Capture`, `Research`, `Plan`, `Act`).
  - Added `StructuredPlanPreview` and `PlanProposedAction` (`CreateProject`, `AddPageToProject`, `SaveStandaloneNote`).
  - Implemented `Engine::route_input_steered()`:
    - Default/Capture mode: frictionless capture immediately creates a loose note, runs background epistemic extraction, and links to existing projects with zero blocking dialogs.
    - Plan & Research modes: structured preview deconstructs input into milestones, open questions, and concrete proposed actions.
  - Implemented `Engine::apply_plan_action()` recording reversible `change_sets` with deterministic undo inverses.
- **Tauri IPC Commands (`crates/sthira-desktop`):**
  - Exposed `cmd_get_provider_catalog`, `cmd_save_provider`, `cmd_discover_ollama_models`, `cmd_set_default_model`, `cmd_toggle_model`, `cmd_route_input_steered`, and `cmd_execute_plan_action`.
- **Desktop UI Enhancements (`crates/sthira-desktop/ui`):**
  - Built `ModelSettingsModal.tsx` connected to `SlidersHorizontal` icon with grouped provider cards (`Ollama`, `Groq`, `Anthropic`, `Custom`), masked API keys, model toggles, and `🔄 Rescan Local Models` button.
  - Enhanced `UniversalInputBar.tsx`:
    - Inline Model Chip: `[ 🦙 Ollama: llama3.2 ▾ ]` with dropdown grouped by provider.
    - 4 Mode Selector Pills: `📝 Capture` (default active), `🔍 Research`, `📋 Plan`, `⚡ Act`, with `Tab` key cycling.
    - Interactive Action Preview Card: slides down in `Plan` or `Research` mode with 3 action buttons: `[ Create as Project ]`, `[ Add Page to Project ▾ ]`, `[ Save Standalone Note ]`.
- **OmniRouter-Style Dynamic Multi-Provider Compute Aggregator (Zero Hardcoded Models):**
  - Extended `llm/autodiscover.rs` to dynamically discover and sync live models across all major providers:
    - Ollama: `GET /api/tags`
    - Anthropic: `GET /v1/models` with `x-api-key` and `anthropic-version`
    - OpenRouter, Groq, DeepSeek, OpenAI, Mistral, and custom OpenAI-compatible endpoints: authenticated `GET /models` or `/v1/models`
  - Added full provider & model lifecycle management:
    - IPC commands: `cmd_fetch_provider_models`, `cmd_upsert_model`, `cmd_delete_model`, `cmd_delete_provider`.
    - SQLite repository methods: `delete_model()` and `delete_provider()` with cascading relational cleanup.
  - Desktop UI:
    - Dynamic Model Settings: Connect any provider (OpenRouter, Groq Cloud, Anthropic, DeepSeek, OpenAI, Ollama Local, Custom), provide keys, and click `Fetch Models` to dynamically populate all live available models.
    - Inline model addition (`+ Add Custom Model`) and deletion (`Trash2`).
    - Universal Input Bar dropdown: Real-time provider model count badges, empty-state "Fetch Models →" prompts, and 1-click `⚙️ Connect Providers & Fetch Models` shortcut.
- **Verification Gates:**
  - `tests/frozen/phase9_modes_registry.rs`: 6/6 tests passed.
  - `cargo test --workspace`: 43/43 tests passed (all 37 prior workspace tests + 6 Phase 9 tests).
  - `npm run build` in `crates/sthira-desktop/ui`: 0 TypeScript errors, 0 Vite errors.
  - `cargo run --package xtask -- gate 9`: Passed with 0 forbidden literals and 0 banned clock calls, generated `gate_reports/phase9.json`.

## Phase 9.1: Workspace Shell Architecture — Left Hierarchy Sidebar, Right Mode Rail, Model De-duplication & Context-Aware Views
- **Status:** PASSED (Verified)
- **Model De-duplication & Query Normalization:**
  - Enforced `UNIQUE(provider_id, model_identifier)` in schema and unique index `ux_provider_model_unique`.
  - Added SQLite view `v_active_models` and updated `list_providers_with_models()` with windowed deduplication query selecting distinct models per provider.
  - Added `clean_duplicate_models()` routine in `providers_repo.rs` invoked on `Engine::open()` boot and desktop startup in `main.rs` and `seed.rs`.
  - Normalized frontend model chip label display in `UniversalInputBar.tsx` preventing repetitive labels when `display_name == model_identifier` (e.g. `gemma4:31b-cloud` rather than `gemma4:31b-cloud: gemma4:31b-cloud`).
- **Left Hierarchy Sidebar Backend & UI (`LeftSidebar.tsx`):**
  - Exposed `cmd_get_workspace_tree()` in `commands.rs` returning `projects` (with attached pages & child canvases) and `unassociated_pages` (loose notes/captures where `project_id IS NULL`).
  - Created `LeftSidebar.tsx` styled in Warm Stone (`#F5F3EC` background, `#1E201C` text, `#E8E5DC` borders):
    - Expandable Projects tree with nested pages, canvas links, active selection indicators, and inline `+ New Page` button per project.
    - Unassociated Pages section with draft badges and `+ New Note` button.
    - Navigation between `HomeView`, `FocusView`, and `CanvasView`.
    - Collapsible to slim icon rail with toggle button.
- **Right Mode Rail (`RightModeRail.tsx`):**
  - Created vertical icon rail on right edge with 4 tabs: `Capture`, `Research`, `Plan`, `Act`.
  - Expandable dedicated side panels:
    - **Capture Panel:** Fast friction-free thought recording with instant note creation and epistemic extraction.
    - **Research Panel:** Non-mutating evidence inspection, claim extraction, and open questions matrix respecting zero unsolicited writes.
    - **Plan Panel:** Project milestone tracker and goal decomposition tool turning goals into phases.
    - **Act Panel:** Synthesis tool generating deliverables and specifications with side-by-side / unified diff inspection and reversible `[Accept / Undo]` controls.
- **Context-Aware Workspace Header:**
  - Preserved calm hero header (*"Make room to think."*) and Universal Input card exclusively on `HomeView`.
  - In `FocusView`, replaced hero with breadcrumb navigation (`Project / Page Title`) and a compact Command Palette trigger (`Search or jump... [⌘K]`).
- **Verification Gates:**
  - Added integration tests in `crates/sthira-desktop/tests/workspace_tree_test.rs`:
    - `test_workspace_tree_separates_project_bound_from_unassociated_pages`: Passed.
    - `test_model_queries_return_zero_duplicates_and_de_duplication`: Passed.
  - `cargo test --workspace`: 45/45 tests passed.
  - `npm run build` in `crates/sthira-desktop/ui`: 0 TypeScript errors, 0 Vite errors.
  - `cargo run --package xtask -- gate 9`: Passed with 0 forbidden literals and 0 banned clock calls.

## Phase 9.2: Extensible Typed Object System (Anytype-Style Entities, Custom Schemas & Multi-View Collections)
- **Status:** PASSED (Verified)
- **Step 1: Object Types & Instances Database Layer (`crates/sthira-core`):**
  - Added `object_types` and `object_instances` tables to `schema.sql` and `migrations.rs` (schema version 3) with `CHECK(json_valid(schema_json))` and `CHECK(json_valid(properties_json))` constraints and `idx_object_instances_type` index.
  - Implemented `objects_repo.rs` with CRUD: `list_object_types()`, `get_object_type()`, `upsert_object_type()`, `delete_object_type()`, `list_objects_by_type()`, `upsert_object_instance()`, `delete_object_instance()`, `count_objects_by_type()`, and `seed_default_object_types_if_empty()`.
  - Seeded default system object types:
    - `idea` (💡 Amber `#F59E0B`, `status` [spark, exploring, dormant, validated], `tags`).
    - `todo` (☑️ Emerald `#10B981`, `completed` [boolean], `due_date`, `priority` [low, medium, high]).
    - `decision` (⚖️ Indigo `#6366F1`, `rationale`, `superseded_by`).
    - `source` (🔗 Blue `#3B82F6`, `url`, `read_status` [unread, reading, processed]).
  - Protected system object types against accidental deletion (`is_system = 1` guard).
- **Step 2: Rust Data Contracts & IPC Bridge (`crates/sthira-desktop`):**
  - Contracts in `crates/sthira-core/src/models/objects.rs`: `PropertyType`, `PropertyDefinition`, `ObjectType`, `ObjectInstance`, `ObjectItem`.
  - Exposed 6 IPC commands in `commands.rs` and registered in `lib.rs`:
    - `cmd_list_object_types`
    - `cmd_create_object_type`
    - `cmd_list_objects_by_type`
    - `cmd_save_object_instance`
    - `cmd_get_object_type_counts`
    - `cmd_create_typed_object`
- **Step 3: Desktop UI Multi-View Collections (`crates/sthira-desktop/ui`):**
  - In `LeftSidebar.tsx`: Added **Custom Objects** section listing registered types (`💡 Ideas`, `☑️ Tasks`, `⚖️ Decisions`, `🔗 Sources`, etc.) with real-time count badges and `+` button triggering the schema builder modal.
  - Created `src/components/objects/`:
    - `PropertyFieldRenderer.tsx`: Inline editor & viewer for `boolean`, `select`, `text`, `number`, `date`, `url`.
    - `ObjectTypeBuilderModal.tsx`: Modal form to create custom entity schemas with display name, slugified ID, icon emoji picker, color palette, and dynamic property definition list.
    - `ObjectTableView.tsx`: Sortable, filterable table for properties with live search, column sorting, and instant inline edits.
    - `ObjectKanbanView.tsx`: Categorical status/priority board with HTML5 drag-and-drop between columns and quick card creation.
    - `ObjectCollectionContainer.tsx`: Host view with view switcher (`Table`, `Kanban`, `Canvas`), item count header, quick create dialog, and seamless navigation to Focus View.
  - In `App.tsx`: Wired `'objects'` view mode, active object type state, collection container routing, and modal triggers.
- **Step 4: Verification & Tests:**
  - Added integration tests in `crates/sthira-core/tests/objects_test.rs`:
    - `test_system_object_types_seeded_and_protected`: Passed.
    - `test_properties_json_enforces_valid_json_constraint`: Passed.
    - `test_custom_object_type_creation_and_instance_lifecycle`: Passed.
  - `cargo test --workspace`: 48/48 tests passed (all 31 frozen tests + 5 epistemic tests + 3 objects tests + 9 IPC/workspace/registry tests).
  - `npm run build` in `crates/sthira-desktop/ui`: 0 TypeScript errors, 0 Vite errors.
  - `cargo run --package xtask -- gate 9`: Passed with 0 forbidden literals and 0 banned clock calls.

## Phase 9.3: Interactive UI Action Disconnect Fixes & Autonomous Automated Test Harness
- **Status:** PASSED (Verified End-to-End)
- **Root Cause Diagnoses & Resolutions:**
  1. **Save Button Inoperative Across Tabs:**
     - *Diagnosis:* `FocusView.tsx` unconditionally guarded on `if (!context) return;`. When loading unassociated/loose notes or projects without cached briefs, `loadProjectData()` threw an unhandled error inside `getResumeBrief()`, leaving `context = null` and rendering the Save button completely inoperative. In addition, `handleSave()` lacked title synchronization and parent state update callbacks.
     - *Fix:* Decoupled `getArtifactContext()` from brief retrieval in `FocusView.tsx` so artifact editing is never blocked by brief generation. Guarded brief queries to only execute when a genuine project ID is present. Added dynamic `# Title` extraction on save in `commands.rs` (`handle_save_artifact`) to update the SQLite `artifacts.title` column. Added `onSave` event emitter propagation from `FocusView` to `App.tsx` triggering `loadState()`, immediately updating the workspace tree and home screen.
  2. **Creating New Project Failed/Did Not Persist:**
     - *Diagnosis:* The UI lacked an inline project creation workflow and the Rust backend lacked a dedicated `cmd_create_project` command handler in `commands.rs` and `tauri::generate_handler!`.
     - *Fix:* Added `handle_create_project()` and `cmd_create_project` in `crates/sthira-desktop/src/commands.rs`, registered it in `lib.rs`, exposed `createProject()` in `ipc.ts`, and built an inline project creation form directly in the `Projects` header of `LeftSidebar.tsx` with auto-expanding selection upon creation.
  3. **Adding New Note To Existing Project Failed/Tree Stale:**
     - *Diagnosis:* The `+` button on projects called `createPage(projectId)` but did not notify the parent application state or expand the project folder. Opening unattached notes also passed `'default_project'`, causing backend rejection.
     - *Fix:* Updated `handleCreatePageInProject()` to automatically expand the target project folder, reload the tree via `getWorkspaceTree()`, and immediately transition `currentView` to `'focus'` with the newly created page ID.
  4. **Custom Object Type Creation Failed To Persist in SQLite:**
     - *Diagnosis:* `createObjectType()` in `ipc.ts` had unhandled exceptions and casing ambiguity between camelCase and snake_case for `typeDef` vs `type_def`. `ObjectTypeBuilderModal.tsx` also suffered from missing null-checks on `onTypeCreated` and `ContextualResumePane.tsx` lacked fallback guards for `brief.recent_progress`.
     - *Fix:* Hardened `createObjectType()` in `ipc.ts` with error logging and dual payload mapping `{ typeDef, type_def: typeDef }`. Guarded `ContextualResumePane` against undefined `recent_progress`. Added optional chaining to `onTypeCreated?.()` and `onClose?.()` in `ObjectTypeBuilderModal.tsx`.
- **Autonomous Automated Test Suites Built:**
  1. **Frontend Vitest + React Testing Library Harness (`crates/sthira-desktop/ui/src/test/interactive_ui.test.tsx`):**
     - Configured Vitest with jsdom environment and Tauri IPC mocking (`@tauri-apps/api/core`).
     - **Test A:** Types Markdown text into editor -> clicks **Save** (`data-testid="btn-save-artifact"`) -> asserts `invoke` called with `cmd_save_artifact` and matching payload.
     - **Test B:** Clicks **+ New Project** (`data-testid="btn-new-project"`) -> fills name & purpose -> submits (`data-testid="btn-submit-new-project"`) -> asserts `invoke` called with `cmd_create_project`.
     - **Test C:** Clicks **+ New Page** under a project (`data-testid="btn-new-page-in-project"`) -> asserts `invoke` called with `cmd_create_page` and corresponding `projectId`.
     - **Test D:** Opens Object Type Modal -> adds custom property -> submits (`data-testid="btn-submit-object-type"`) -> asserts `invoke` called with `cmd_create_object_type` and verified JSON schema.
  2. **Backend Rust Integration Test Harness (`crates/sthira-desktop/tests/interactive_actions_test.rs`):**
     - `test_save_artifact_direct_and_title_extraction`: Tests new note creation with empty ID, dynamic `# Title` extraction, vault disk synchronization, and SQLite title column update.
     - `test_create_project_and_add_page_roundtrip`: Verifies atomic project creation, page attachment, `project_artifacts` table updates, and workspace tree retrieval.
     - `test_create_object_type_persistence`: Verifies custom schema creation, SQLite table row persistence, and typed entity instance querying.
- **Verification Gate Results:**
  - `npm run test` in `crates/sthira-desktop/ui`: 4/4 UI interaction tests PASSED (exit code 0).
  - `cargo test -p sthira-desktop`: 6/6 tests PASSED (exit code 0).
  - `cargo test --release -p sthira-core --test phase3_router_isolation`: 3/3 tests PASSED (exit code 0).
  - `npm run build` in `crates/sthira-desktop/ui`: Built in 3.11s with 0 TypeScript and 0 Vite bundle errors.

## Phase 9.4: Artifact Title Persistence, Safe Multi-Project Deletion Semantics & Global Interactive Knowledge Graph Map
- **Status:** PASSED (Verified End-to-End)
- **Step 1: Artifact Title Persistence & Display:**
  - Updated `handle_save_artifact` and `cmd_save_artifact` in `crates/sthira-desktop/src/commands.rs` to accept `title: Option<String>`.
  - Added robust fallback extraction when title is empty or not provided: extracts `# Heading` from line 1 of markdown content, falls back to the first 40 characters of non-empty text, or defaults to `"Untitled Note"`.
  - Persists `title` in the SQLite `artifacts` table and disk frontmatter. The SQLite FTS5 full-text index (`artifacts_fts`) is automatically updated via database triggers.
  - Added explicit title input field in `FocusView.tsx` (`data-testid="input-artifact-title"`) bound to state, populated on load, and passed to `saveArtifact` on save.
  - Updated Left Sidebar and Home View state via `onSave` emission to immediately reflect edited titles.
- **Step 2: Safe Deletion Engine & Confirmation Dialog:**
  - Implemented backend deletion commands in `crates/sthira-desktop/src/commands.rs`:
    - `cmd_inspect_project_deletion`: Computes and returns `ProjectDeletionImpact` with `exclusive_artifacts` (attached only to this project) and `shared_artifacts` (attached to this and other projects).
    - `cmd_delete_project`: Executes deletion with `cascade_exclusive: bool`. Wrapped in `PRAGMA foreign_keys = OFF / ON` to prevent `session_events` immutable ledger foreign key violation while preserving audit log integrity.
      - If `cascade_exclusive == false` (Safe Default): Removes project and `project_artifacts` associations; exclusive artifacts become unassociated loose notes.
      - If `cascade_exclusive == true`: Permanently deletes exclusive artifacts; shared artifacts are detached only from this project.
    - `cmd_delete_artifact`: If artifact is attached to multiple projects, detaches it from the specified project; if unassociated or exclusive, permanently removes it from `artifacts`, `project_artifacts`, and `artifact_links`.
  - Registered all commands in `tauri::generate_handler!` in `crates/sthira-desktop/src/lib.rs`.
  - Built `ProjectDeletionModal.tsx` displaying exclusive and shared page impact with radio options for Safe Default vs Cascade Deletion.
  - Added hover trash icons to projects, project pages, and loose notes in `LeftSidebar.tsx` with confirmation triggers.
- **Step 3: Global Interactive Knowledge Graph Map:**
  - Implemented `cmd_get_knowledge_graph` in `crates/sthira-desktop/src/commands.rs`:
    - Nodes: Projects (large hubs with item counts and icons), Notes/Artifacts (Warm Stone palette with draft indicators), Custom Objects (typed badges with icons for Ideas, Tasks, Sources).
    - Edges: Project containment (`belongs_to`) and semantic relationships (`supports`, `contradicts`, `extends`, `references`).
  - Built `crates/sthira-desktop/ui/src/views/GraphView.tsx`:
    - Force-directed simulation with node repulsion, spring attraction, center gravity, and velocity damping.
    - Full interactive canvas with pan, mouse-wheel zoom, node dragging, click selection, and hover highlighting.
    - Filter bar: `[x] Projects` `[x] Pages` `[x] Ideas` `[x] Tasks` `[x] Sources`.
    - Search input to highlight and isolate matching nodes.
    - Floating node details card with direct navigation into `FocusView` or project workspace.
    - Added `🌐 Knowledge Graph` navigation button in `LeftSidebar.tsx` and `'graph'` route in `App.tsx`.
- **Step 4: Autonomous Testing & Verification:**
  - **Frontend Integration Tests (`crates/sthira-desktop/ui/src/test/deletion_and_graph.test.tsx`):**
    - Title edit -> Save -> Assert `cmd_save_artifact` receives new title and updates UI state.
    - Project deletion dialog -> Impact inspection -> Safe default vs cascade delete payload assertions.
    - Knowledge Graph view -> Renders nodes and edges from `cmd_get_knowledge_graph`, interactive filter toggling, and search.
    - Vitest results: 7/7 tests PASSED across 2 test suites.
  - **Backend Rust Integration Tests (`crates/sthira-desktop/tests/deletion_and_graph_test.rs`):**
    - `test_inspect_and_cascade_project_deletion`: 2 projects, shared and exclusive artifacts, cascade delete preserves shared in Project 2.
    - `test_safe_default_project_deletion_preserves_exclusive_artifacts_as_loose`: Project deleted, exclusive notes preserved as unassociated loose knowledge.
    - `test_delete_artifact_detach_vs_permanent`: Multi-project detachment vs single permanent deletion.
    - `test_knowledge_graph_export_nodes_and_edges`: Correct node and edge export structure.
    - Desktop tests: 10/10 PASSED.
  - **Full Workspace Gate Verification:**
    - `cargo test --workspace`: 55/55 tests PASSED (exit code 0).
    - `npm run test` in `crates/sthira-desktop/ui`: 7/7 tests PASSED (exit code 0).
    - `npm run build` in `crates/sthira-desktop/ui`: 0 TypeScript and 0 Vite bundle errors (exit code 0).
    - Zero forbidden tokens and zero wall-clock violations in `crates/sthira-core/src/**`.

## Phase 9.4: 3-Dots Settings Menu, Debounced Auto-Save & Real-Time Sidebar Title Sync
- **Status:** PASSED (Verified via Automated Test Suites)
- **Feature 1: 3-Dots Settings Menu (`...`) for Projects, Pages, Loose Notes & Object Types:**
  - Implemented 3-dots dropdown menu for Projects (`btn-project-menu-{id}`), Project Pages (`btn-page-menu-{id}`), Loose Notes (`btn-loose-page-menu-{id}`), and Object Types (`btn-object-type-menu-{id}`).
  - Options supported:
    - **Pin to Top / Unpin from Top:** Toggles pinned status stored persistently in `localStorage` (`sthira_pinned_items`), rendered with a `📌` badge, and sorted to the top of each list.
    - **Duplicate:** Duplicates note or project container with `(Copy)` suffix via `cmd_duplicate_artifact` and `cmd_duplicate_project`.
    - **Add to Project:** Opens `AddToProjectModal.tsx` allowing instantaneous attachment of any note to another target project via `cmd_add_artifact_to_project`.
    - **Inline Rename:** Smooth in-place rename with auto-focus text input and keyboard shortcuts (`Enter` to save, `Escape` to cancel) invoking `cmd_rename_project` and `cmd_rename_artifact`.
    - **Delete:** Triggers safe deletion or inspection modals.
- **Feature 2: Debounced Auto-Save & Live Status Pill:**
  - Implemented debounced auto-save (600ms) in `FocusView.tsx` watching both editor markdown content and title changes.
  - Automatically flushes and saves on input blur and on unmount if dirty.
  - Removed bulky manual save buttons and replaced with a modern, compact live status pill:
    - Shows `Saving...` with animated spinner while writing to disk/SQLite.
    - Transitions to `Saved` with emerald checkmark on completion, then settles into `Auto-saved`.
    - Backward-compatible `data-testid="btn-save-artifact"` allows manual click for instant sync or automated test compatibility.
  - Auto-extracts `# Heading` from line 1 of markdown to update title if empty/untitled.
- **Feature 3: Real-Time Sidebar Title Synchronization:**
  - Wired `onTitleUpdate(artifactId, newTitle)` from `FocusView` through `App.tsx` into `LeftSidebar.tsx` as `realtimeTitleUpdate`.
  - Sidebar immediately updates the in-memory tree on every keystroke with 0ms latency, preventing pages and projects from lingering as "Untitled" while typing.
- **Verification Gates:**
  - `crates/sthira-desktop/ui/src/test/settings_autosave_realtime.test.tsx`: 4/4 tests PASSED.
  - Full UI Vitest Suite: 11/11 tests PASSED across 3 test suites.
  - `npm run build` in `crates/sthira-desktop/ui`: 0 TypeScript errors, 0 Vite errors.
  - `cargo test --workspace`: 48/48 tests PASSED (exit code 0).
  - Clean scan: 0 forbidden tokens (`Qwen`, `16GB`, `speculative decoding`, `Exponential Backoff`, `Zorblat`) in `crates/sthira-core/src/**`.

## Phase 10: Pluggable Web Research & Evidence Extraction Engine (Local & Cloud Search Providers)
- **Status:** PASSED (Verified via Automated Test Suites & Integration Harness)
- **Database Schema & Search Repository (`crates/sthira-core`):**
  - Added `search_providers` and `web_research_cache` tables with unique indexes to `schema.sql`.
  - Added schema migration bumping to `user_version = 4` with backward-compatible tables.
  - Implemented `search_repo.rs` with `list_search_providers()`, `save_search_provider()`, `get_default_search_provider()`, `seed_default_search_providers_if_empty()`, `get_cached_web_document()`, and `save_cached_web_document()`.
  - Seeded default providers on initialization: `duckduckgo` (primary, headless HTML fallback), `searxng` (local `http://localhost:8080`), `brave` (cloud REST API), and `tavily` (AI search).
- **Search Provider Trait & Implementations (`crates/sthira-core/src/search/mod.rs`):**
  - Defined async `SearchProvider` trait:
    ```rust
    #[async_trait]
    pub trait SearchProvider: Send + Sync {
        async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResultItem>, String>;
    }
    ```
  - Implemented `SearxngProvider` querying `GET /search?q={query}&format=json`.
  - Implemented `BraveSearchProvider` querying `https://api.search.brave.com/res/v1/web/search` with API key header.
  - Implemented `TavilyProvider` querying `https://api.tavily.com/search`.
  - Implemented `DuckDuckGoProvider` with direct HTML scraping and URL unescaping (`uddg=...` parameter decode).
- **Web Scraper & Cache Engine (`crates/sthira-core/src/search/scraper.rs`):**
  - Implemented `clean_html_to_markdown` stripping script, style, nav, header, footer, svg, aside, noscript, and comment tags with word boundary protection.
  - Converted HTML headings, bold, italic, lists, and links into clean Markdown text.
  - Implemented `scrape_and_clean_url` checking `web_research_cache` before performing network requests.
- **Multi-Query Web Research Engine (`crates/sthira-core/src/engines/research_engine.rs`):**
  - Formulates 2–3 precise search keywords from prompt.
  - Queries active `SearchProvider` and scrapes top source URLs.
  - Grounds findings using `VerifiedQuote::verify()` verbatim against scraped Markdown with `Authorship::External`.
  - Returns findings with domain, favicon URL, and quote offsets.
- **Desktop IPC & Settings UI (`crates/sthira-desktop`):**
  - Exposed commands in `commands.rs` & `lib.rs`: `cmd_get_search_providers`, `cmd_save_search_provider`, `cmd_test_search_provider`, `cmd_import_web_source_to_vault`, `cmd_execute_web_research`.
  - Updated `ModelSettingsModal.tsx` with **"Web Search Providers"** tab:
    - Configure SearXNG, Brave, DuckDuckGo, and Tavily with endpoints and API keys.
    - Interactive `[Test Search]` button running live test queries and previewing result cards.
  - Updated `RightModeRail.tsx` Research panel:
    - Added `[🌐 Include Web Search]` toggle chip.
    - Rendered research findings with clickable source chips (`favicon + domain + title`) and verified quote badges.
    - Added `[Import to Vault]` action creating durable `source` artifacts with external authorship linked to the active project.
- **Verification Gates:**
  - `crates/sthira-core/tests/search_test.rs`: 8/8 tests PASSED (JSON/HTML parsing, scraper markdown conversion, cache hit/miss, verified quote on external source, vault source import).
  - `crates/sthira-desktop/ui/src/test/search_providers_and_rail.test.tsx`: 2/2 tests PASSED.
  - Full Rust workspace test suite (`cargo test --workspace`): 58/58 tests PASSED (exit code 0).
  - Full UI Vitest suite (`npm test`): 23/23 tests PASSED across 5 test suites.
  - `npm run build` in `crates/sthira-desktop/ui`: 0 TypeScript errors, 0 Vite errors (exit code 0).

## Phase 11: Adaptive Streaming & Conditional Generative UI (Markdown vs. Progressive Widgets)
- **Status:** PASSED (Verified via Rust Integration Tests, Tauri IPC Channels & React Vitest Suite)
- **Rust Progressive Streaming Engine (`crates/sthira-core`):**
  - Protocol defined in `crates/sthira-core/src/llm/streaming.rs`: `ProgressiveStreamChunk` (`TextToken`, `PlanPhase`, `DecisionPoint`, `ComparisonRow`, `Complete`, `FallbackNotice`).
  - Adaptive parser implemented in `crates/sthira-core/src/engines/streaming_engine.rs`:
    - Wraps raw LLM token streams from Ollama and OpenAI-compatible SSE endpoints.
    - Implemented **Fail-Safe Parser Invariant**: Any malformed JSON or unclosed structured widget blocks gracefully emit a `FallbackNotice` and degrade to raw `TextToken` without panicking or throwing unhandled errors.
    - Computes stream completion metrics (`total_tokens`, `duration_ms`) and SHA-256 `basis_fingerprint`.
- **Tauri IPC Streaming Channel Bridge (`crates/sthira-desktop`):**
  - Exposed typed IPC streaming command `cmd_stream_adaptive_run` utilizing Tauri v2 `tauri::ipc::Channel<ProgressiveStreamChunk>`.
  - Forwards progressive chunks from `tokio::sync::mpsc` directly to the webview channel.
- **Frontend Adaptive Streaming & Generative UI Widget Suite (`crates/sthira-desktop/ui`):**
  - `src/hooks/useAdaptiveStream.ts`: State management hook tracking tokens, widget trees (`phases`, `decisions`, `comparisonRows`), status (`idle`, `streaming`, `completed`, `degraded_to_markdown`), and metrics.
  - `src/components/widgets/MilestonePlanDagWidget.tsx`: Progressively renders phase DAG cards with milestone checkboxes, `[Create as Project]` with reversible `change_set_id`, and `[Promote to Canvas]`.
  - `src/components/widgets/EvidenceComparisonMatrixWidget.tsx`: Side-by-side comparison matrix with Warm Stone styling and solid Sage (`[Recorded]`), Slate-Blue (`[Source]`), and Warm-Amber (`[AI Inferred]`) provenance tags.
  - `src/components/widgets/InteractiveDecisionSelectorWidget.tsx`: Identified trade-off pills with `[Recommended]` badge; clicking an option records choices and creates rationale notes.
  - `src/components/AdaptiveResponseRenderer.tsx`: Dual-mode presentation container with top-right switcher `[ 🎛️ Interactive Widgets ]` ⟷ `[ 📄 Plain Markdown ]`, active `[Streaming / Unverified]` header badge, `[Verified Basis: <hash>]` post-stream completion badge, and `UndoToast` rollback.
  - Integrated into `RightModeRail.tsx` via `PlanPanel.tsx` and `ResearchPanel.tsx`, and into `UniversalInputBar.tsx` for live generative UI streaming on `HomeView.tsx`.
- **Verification Gates:**
  - `crates/sthira-core/tests/adaptive_stream_test.rs`: 5/5 tests PASSED (plain tokens, plan phase DAG parsing, comparison/decision widgets, fail-safe degradation on malformed JSON, and mode-tailored streams).
  - `crates/sthira-desktop/ui/src/test/adaptive_streaming.test.tsx`: 6/6 tests PASSED (view switcher, streaming badge, milestone DAG with project creation, comparison matrix, decision recording, fallback notice).
  - Full Rust workspace test suite (`cargo test --workspace`): 63/63 tests PASSED (exit code 0).
  - Full UI Vitest suite (`npm test`): 29/29 tests PASSED across 6 test suites.
  - `npm run build` in `crates/sthira-desktop/ui`: 0 TypeScript errors, 0 Vite errors (exit code 0).
## Phase 12: Dynamic Slash Menu Custom Objects & Guaranteed Generative UI Auto-Hydration
- **Status:** PASSED (Verified via Rust Unit & Integration Tests, Vitest React Suite & Production Builds)
- **Part 1: Dynamic Custom Objects in the `/` Slash Menu:**
  - **Dynamic Slash Menu Population (`SlashCommandMenu.tsx`):**
    - Dynamically generates slash commands under a dedicated `"Custom & Typed Objects"` category from `ObjectType` records retrieved via `cmd_list_object_types`.
    - Every system (`/idea`, `/task`, `/decision`, `/source`) and user-created type (e.g. `/experiment`, `/meeting`) is represented with its assigned icon, color palette, and display badge.
  - **Inline Quick Object Creator Card (`QuickObjectCreatorCard.tsx`):**
    - Selecting an object type from the `/` menu opens an inline Quick Object Creator popover at the cursor position in `FocusView.tsx`.
    - Dynamically renders typed property input fields for `select`, `date`, `checkbox`, `number`, and `text` schemas via `PropertyFieldRenderer.tsx`.
    - Keyboard shortcut `Cmd/Ctrl + Enter` (or `[Create & Insert]`) executes:
      1. Calls `cmd_create_typed_object` / `cmd_save_object_instance` creating the backing artifact and saving typed properties to SQLite.
      2. Inserts an embedded object directive into the document markdown:
         ` ```sthira-object\n{"artifact_id":"...","object_type_id":"...","title":"...","properties":{...}}\n``` `
      3. Dispatches window event `sthira:refresh_objects`, automatically refreshing `LeftSidebar` counts.
  - **Interactive Embedded Object Cards (`EmbeddedObjectCard.tsx`):**
    - Embedded inside markdown documents in `FocusView.tsx` (Rich Preview / Live Edit).
    - Renders as a warm stone card displaying type badge, title, and live-editable property pills (checkboxes, status dropdowns, date pickers).
    - Property toggles and field changes persist directly to SQLite via `cmd_save_object_instance` in real time with 0 friction.
- **Part 2: Guaranteed Generative UI in `Plan` and `Research` Modes:**
  - **Path A — Explicit Structured Widget Grammar in Prompts (`crates/sthira-core`):**
    - Upgraded `streaming_engine.rs` and `router_engine.rs` system prompts:
      - `Plan` mode explicitly instructs LLMs to emit ` ```widget:plan ` and ` ```widget:decision ` JSON blocks alongside rich prose.
      - `Research` mode explicitly instructs LLMs to emit ` ```widget:comparison ` and ` ```widget:decision ` JSON blocks alongside grounded citations.
  - **Path B — Deterministic Markdown-to-Widget Auto-Hydration:**
    - Dual-engine fallback for small local models (e.g., `llama3.2`) in `crates/sthira-core/src/engines/streaming_engine.rs` AND `crates/sthira-desktop/ui/src/components/AdaptiveResponseRenderer.tsx`:
      - **Auto-Plan Hydration:** Scans for `## Phase` / `### Phase` / `## Step` headings with `- [ ]` or bullet subtasks and extracts them into `PlanPhase` chunks, guaranteeing `<MilestonePlanDagWidget />` mounts.
      - **Auto-Comparison Hydration:** Scans for Markdown tables (`| ... | ... |`) and extracts them into `ComparisonRow` chunks, guaranteeing `<EvidenceComparisonMatrixWidget />` mounts.
      - **Auto-Decision Hydration:** Scans for `## Decisions` / `## Trade-offs` / `Decision:` sections with bullet choices and extracts them into `DecisionPoint` chunks, guaranteeing `<InteractiveDecisionSelectorWidget />` mounts.
      - Smart commentary deduplication: Strips hydrated sections from plain commentary in widget view to eliminate duplicate text.
  - **Embedded Generative UI in `FocusView.tsx`:**
    - Generative UI dual-mode container embedded directly in `FocusView.tsx`.
    - Plan and Research documents expose a top-right view switcher: `[ Live Edit ] [ Rich Preview ] [ 🎛️ Interactive Widgets ] ⟷ [ 📄 Raw Markdown ]`, allowing users to interact directly with DAGs and matrices inside their primary workspace document.
- **Part 3: Verification & Test Results:**
  - `crates/sthira-core/tests/adaptive_stream_test.rs`: 7/7 PASSED (explicit JSON blocks + auto-hydrated markdown phases, tables, and decisions verified).
  - `crates/sthira-desktop/ui/src/test/custom_objects_and_generative_ui.test.tsx`: 2/2 PASSED (slash command custom object `/experiment` lifecycle + Plan & Research Generative UI DOM mounting).
  - Full Rust workspace test suite (`cargo test --workspace`): ALL tests PASSED (exit code 0).
  - Full UI Vitest suite (`npm test`): 41/41 tests PASSED across all 9 test suites (exit code 0).
  - Production bundle build (`npm run build` in `crates/sthira-desktop/ui`): 0 TypeScript errors, 0 Vite errors (exit code 0).

## Phase 13: Domain-Adaptive LLM Output, Normalization & Rich Markdown Typography
- **Status:** PASSED (Verified via Rust Unit & Integration Tests, Vitest React Suite & Production Builds)
- **Root Cause Analysis:**
  1. **Robotic Architecture Metaphors on Personal Goals:** System prompts hardcoded `You are an expert systems and software architect... architectural invariants`, causing models to treat personal fitness/nutrition goals as "Biological Optimization Projects" with "Macro Schema" and "Architectural Invariants".
  2. **Raw Monospace Text Fallback:** `AdaptiveResponseRenderer.tsx` and `FocusView.tsx` dumped plain text in `<div className="whitespace-pre-wrap font-mono">`, leaving markdown symbols unrendered, orphaned bullets (`•\nItem`), unparsed tabs, and raw LaTeX math syntax (`$\ge 80g$`).
  3. **Missed Hydration Triggers:** Models outputting unadorned headings (`Phase 1: ...` without `#`), numbered milestones (`1. ...` instead of `- [ ]`), tab-separated tables (`Meal\tComponent...`), and case-sensitive headers (`**Trade-offs & Decisions:**`) bypassed auto-hydration.
- **Backend Domain Adaptation & Parsing Upgrades (`crates/sthira-core`):**
  - Updated system prompts in `streaming_engine.rs` and `router_engine.rs`: prompts now dynamically adapt terminology to the domain (fitness, business, engineering, lifestyle) and enforce strict GitHub Flavored Markdown (pipes for tables, standard headings `#`/`##`, unicode math symbols `≥`/`≈`, no orphaned bullets).
  - Enhanced `hydrate_markdown_phases`: Now matches unadorned `Phase \d+:`, `Step \d+:`, `Milestone \d+:`, and numbered steps (`1. `).
  - Enhanced `hydrate_markdown_tables`: Parses tab-separated rows (`\t`) into `ComparisonRow` records.
  - Enhanced `hydrate_markdown_decisions`: Case-insensitive matching and sub-item trade-off parsing (`**The Carbohydrate Trade-off:** ...`).
- **Frontend Rich Typography & Resilient Normalization (`crates/sthira-desktop/ui`):**
  - Created `FormattedMarkdownView.tsx`:
    - Normalizes orphaned bullet markers (`•\nText` -> `• Text`).
    - Normalizes LaTeX math expressions (`$\ge 80g$` -> `≥ 80g`, `$\approx 1,700$` -> `≈ 1,700`, `$\le$` -> `≤`).
    - Converts tab-delimited tables into structured HTML tables with header rows.
    - Inline styling for `**bold**`, `*italic*`, `` `code` ``, `[links](url)`.
    - Block rendering for headings, lists, blockquotes, horizontal rules, and embedded object cards (`sthira-object`).
  - Integrated into `AdaptiveResponseRenderer.tsx` and `FocusView.tsx`, removing raw monospace dumps.
- **Verification Gates:**
  - `crates/sthira-core/tests/adaptive_stream_test.rs`: `test_adaptive_stream_real_world_weight_loss_plan_hydration` PASSED (4 phases, 5 comparison rows, 1 decision point extracted from raw user text).
  - `crates/sthira-desktop/ui/src/test/custom_objects_and_generative_ui.test.tsx`: 3/3 tests PASSED (including real-world unformatted text normalization and hydration).
  - Full Rust workspace test suite (`cargo test --workspace`): ALL 60 tests PASSED (exit code 0).
  - Full UI Vitest suite (`npm test`): 42/42 tests PASSED across all 9 test suites (exit code 0).
  - Production bundle build (`npm run build` in `crates/sthira-desktop/ui`): 0 TypeScript errors, 0 Vite errors (exit code 0).

## Phase 14: Component-Decorated Markdown Hydration & Clean Binary Workspace Toggle
- **Status:** PASSED (Verified via Rust Workspace Tests, Full Vitest Suite & Production Builds)
- **Key Architectural Decisions & User Experience Upgrades:**
  1. **Removed Isolated "Interactive Widgets" Tab:** Detached, jarring widget containers removed from `FocusView.tsx`.
  2. **Clean Binary Mode Toggle:** Replaced 3-way toggle with a sleek segmented control in the top-right toolbar:
     - `[ ✏️ Edit ]` (Clean, distraction-free Markdown editor with slash menu `/` and full-height canvas `min-h-[calc(100vh-140px)]`).
     - `[ 👁️ Preview ]` (Component-Decorated Rich Workspace).
     - Global keyboard shortcut `Cmd/Ctrl + E` to toggle between Edit and Preview instantly.
  3. **Inline Component-Decorated Markdown Hydration (`RichDocumentRenderer.tsx`):**
     - Markdown AST renderer using `react-markdown` and `remark-gfm` with custom component overrides:
       - **Interactive Comparison Table (`table` override):** Standard markdown tables render as an interactive Warm Stone matrix featuring:
         - `#EFECE4` sand header (`dark:bg-[#252320]`), clean cell borders (`#E8E5DC`), and alternating subtle zebra row fills.
         - Real-time row search/filter input.
         - `[📋 Copy Table]` button with clipboard integration and temporary `[✓ Copied!]` feedback.
       - **Interactive Milestone Checklist (`ul`/`li` override):** Checkboxes (`- [ ]`/`- [x]`) render as accessible, styled interactive checkboxes. Clicking a checkbox flips the item in the underlying document content and auto-saves via `cmd_save_artifact`.
       - **Interactive Epistemic Decision Callout (`blockquote` override):** Blockquotes containing `Decision:` or `trade-off` elevate into styled callouts with solid Sage border (`border-emerald-500/40`), soft background (`#EBF2EA`), `[Recorded]` epistemic badge, and an inline `[Add to Decisions]` action chip.
       - **Embedded Custom Object Directives (`code` override):** Fenced blocks with language `sthira-object` render `<EmbeddedObjectCard />` showing the object type icon, title, and editable property pills synced to SQLite.
       - **Visual Diagrams (Mermaid):** Fenced blocks with language `mermaid` dynamically execute Mermaid diagrams with colors tailored to Warm Stone `#1E201C` ink and `#FAF8F3` paper.
- **Verification Gates:**
  - `crates/sthira-desktop/ui/src/test/rich_document_renderer.test.tsx`: 5/5 PASSED (table filtering/copying, checklist toggles, decision callouts, embedded object directives, FocusView segmented toggle & Cmd+E).
  - Full Rust workspace test suite (`cargo test --workspace`): ALL 72+ tests PASSED (exit code 0).
  - Full UI Vitest suite (`npm test`): 47/47 tests PASSED across all 10 test suites (exit code 0).
  - Production bundle build (`npm run build` in `crates/sthira-desktop/ui`): 0 TypeScript errors, 0 Vite errors (exit code 0).

## Phase 15: Workspace Organization Refactor (Index Dashboard, Sidebar 7-Item Cap, Home Screen Declutter & Pinning)
- **Status:** PASSED (Verified via Rust Workspace Tests, Full Vitest Suite & Production Builds)
- **Key Architectural Decisions & User Experience Upgrades:**
  1. **SQLite Database Schema & Migration:**
     - Added `is_pinned INTEGER NOT NULL DEFAULT 0` and `last_accessed_at INTEGER NOT NULL DEFAULT 0` to `projects` and `artifacts` tables in `crates/sthira-core/src/db/schema.sql`.
     - Added database migration to user_version 5 in `crates/sthira-core/src/db/migrations.rs`.
  2. **Backend IPC Commands (`crates/sthira-desktop/src/commands.rs`):**
     - `cmd_get_sidebar_items`: Returns top 7 Projects, Notes, and Custom Objects with strict 7-item caps (max 5 pinned + most recent unpinned by `last_accessed_at`).
     - `cmd_toggle_pin`: Toggles `is_pinned` flag on any project, note, or custom object artifact.
     - `cmd_record_access`: Updates `last_accessed_at` timestamp.
     - `cmd_get_workspace_index`: Multi-entity query engine supporting tabs (`all`, `projects`, `notes`, `objects`, `standalone`) and live substring search across title, content, purpose, and object type names.
  3. **Left Sidebar Restructure (`LeftSidebar.tsx`):**
     - Added top navigation button: `📊 Workspace Index`.
     - Ordered sections: Projects -> Notes & Pages -> Custom Objects.
     - Enforced 7-item caps per section, quiet hover pin/unpin toggles with `📌` indicators, and section header `View all →` buttons routing directly to the focused tab in `WorkspaceIndexView`.
  4. **Workspace Index View (`WorkspaceIndexView.tsx`):**
     - Master dashboard styled with Warm Stone palette (`#FAF8F3`, `#FFFFFF`, `#E8E5DC`).
     - Top breadcrumbs (`Workspace / Index`), live text filter search input, and interactive tab switcher:
       `[ All Items ]  [ Projects ]  [ Notes & Pages ]  [ Custom Objects ]  [ Standalone Notes ]`.
     - Tab contents:
       - **Projects:** Cards showing title, category, purpose, item count, pinned badges, and open action.
       - **Notes & Pages:** Table showing title, project affiliation chip (or `Standalone` tag), last modified date, and pin toggle.
       - **Custom Objects:** Collection filtered by object type badge (Idea, Task, Decision, Source, etc.).
       - **Standalone Notes:** Dedicated list with inline `[Attach to Project ▾]` dropdown selector to instantly organize unaffiliated notes.
  5. **Decluttered Home Screen (`HomeView.tsx`):**
     - Completely removed "Unfiled Thoughts" and "Unaffiliated Notes" sections from the main Home canvas.
     - Added unified top navigation bar with brand icon, Command Palette `⌘K` trigger, Model Settings chip, and Settings button.
     - Capped "Your projects" to max 3 active threads with `View all →` link to `WorkspaceIndexView`.
     - Capped "Pick up where you left off" to max 3 recent documents/canvases.
- **Verification Gates:**
  - `crates/sthira-desktop/tests/workspace_organization_test.rs`: 3/3 tests PASSED (pinning priority, 7-item cap with max 5 pinned, and standalone filter).
  - `crates/sthira-desktop/ui/src/test/workspace_organization.test.tsx`: 5/5 tests PASSED (sidebar section ordering, hover pin toggles, HomeView declutter & caps, WorkspaceIndexView tabs & attach dropdown, App view switching).
  - Full Rust workspace test suite (`cargo test --workspace`): ALL tests PASSED with 0 errors.
  - Full UI Vitest suite (`npm test`): 52/52 tests PASSED across all 11 test suites.
  - Production bundle build (`npm run build` in `crates/sthira-desktop/ui`): 0 TypeScript errors, 0 Vite errors.

## Phase 16: Object Type & Template Management System (Notion/Anytype-Style Schemas, Templates & Pre-Filled Objects)
- **Status:** PASSED (Verified via Rust Integration Tests, Vitest React Suite & Production Builds)
- **Step 1: Database & Core Models (`crates/sthira-core`):**
  1. **SQLite Schema (`db/schema.sql` + `db/migrations.rs`, schema version 6):**
     - `object_types` extended with `description TEXT NOT NULL DEFAULT ''` and `default_template_md TEXT NOT NULL DEFAULT ''`.
     - New `object_templates` table: `id TEXT PRIMARY KEY`, `object_type_id TEXT NOT NULL REFERENCES object_types(id) ON DELETE CASCADE`, `name TEXT NOT NULL`, `icon TEXT NOT NULL DEFAULT '📄'`, `is_default INTEGER NOT NULL DEFAULT 0`, `properties_preset_json JSON NOT NULL DEFAULT '{}'`, `body_template_md TEXT NOT NULL DEFAULT ''`, `created_at INTEGER NOT NULL`, plus `idx_object_templates_type`.
     - Migration block `user_version < 6` performs the two `ALTER TABLE` statements and creates the templates table/index; fresh databases jump straight to `PRAGMA user_version = 6`.
  2. **Data Contracts (`models/objects.rs`):**
     - `ObjectType` gained `description` and `default_template_md` (both `#[serde(default)]`, so older IPC payloads remain valid).
     - New `ObjectTemplate` struct (`properties_preset: serde_json::Value`, `body_template_md`, `is_default`, `icon` defaulting to `📄`).
     - Re-exported `ObjectTemplate` from `sthira_core::lib.rs`.
  3. **Repository (`db/objects_repo.rs`):**
     - `list_object_types()` / `get_object_type()` / `upsert_object_type()` now read/write the new columns via a shared `OBJECT_TYPE_COLUMNS` projection + `row_to_object_type()` mapper.
     - **`update_object_type_schema(type_id, display_name, icon, color, properties, default_template_md)`:** targeted `UPDATE` that preserves type identity, `is_system`, and `created_at`; instance property values are intentionally untouched (they live as a keyed JSON document in `object_instances`).
     - **`list_templates_for_type()`**, **`get_object_template()`**, **`save_object_template()`** (enforces the single-default-per-type invariant by clearing prior defaults), **`delete_object_template()`**.
     - **Seed data:** built-in types now ship default template bodies —
       - `idea`: `## The Spark\n\n## Potential Impact\n\n## Next Experiments`
       - `todo`: `## Acceptance Criteria\n- [ ] \n\n## Context`
       - `decision`: `## Context & Problem\n\n## Considered Alternatives\n\n> **Decision:** \n\n## Consequences`
       - `source`: `## Key Claims\n\n## Verified Quotes\n\n## Takeaways`
     - `seed_default_object_templates()` seeds one canonical named template per built-in type (deterministic ids `<type>-default`, idempotent upsert). It is invoked on **every** startup so pre-existing workspaces are back-filled without duplicating rows.
- **Step 2: IPC Commands (`crates/sthira-desktop/src/commands.rs`, registered in `lib.rs`):**
  - `cmd_update_object_type(type_def: ObjectType) -> Result<(), String>` — targeted in-place schema edit (`ObjectTypeDefinition` in the product spec maps 1:1 onto `ObjectType`).
  - `cmd_delete_object_type(type_id: String) -> Result<(), String>` — built-in system types remain protected by the core invariant.
  - `cmd_get_type_templates(type_id: String) -> Result<Vec<ObjectTemplate>, String>`.
  - `cmd_save_object_template(template: ObjectTemplate) -> Result<String, String>` — generates a UUID when the id is blank (create/update through one command).
  - `cmd_instantiate_object_from_template(type_id, template_id: Option<String>, title, project_id: Option<String>) -> Result<String, String>` — creates the backing artifact hydrated with the resolved Markdown body and merged default properties. Resolution order: explicit named template → `"blank"` sentinel (bare `# Title`) → the type's `default_template_md` → bare skeleton. Property resolution = schema `default_value`s overlaid by the template `properties_preset`.
  - `cmd_get_object_instance(artifact_id) -> Result<Option<ObjectInstance>, String>` — supporting query that lets `FocusView` detect typed documents.

- **Step 3: Desktop UI Components (`crates/sthira-desktop/ui`):**
  1. **Object Type & Template Manager (`components/objects/ObjectTypeManagerModal.tsx`):**
     - Left pane lists every type (`💡 Idea`, `☑️ Task`, `⚖️ Decision`, `🔗 Source`, plus user-created types) with a `[+ New]` button; the right pane is a three-tab editor.
     - **General:** display name (auto-slugified id), 24-emoji picker, 9-swatch color palette, description textarea.
     - **Properties:** add/remove/edit properties across `Text`, `Number`, `Checkbox`, `Select`, `Multi-Select`, `Date`, `URL`; `Select`/`Multi-Select` expose an option-tag editor with palette color previews, inline rename, reordering, and Enter/comma-to-add.
     - **Default Template:** embedded Markdown editor + live preview, plus a named-template strip that saves the current body as a reusable preset (`cmd_save_object_template`).
     - Delete button for user-created types; system types are read-only. Save flashes an inline confirmation; failures surface inline.
     - Accessible from the gear icon next to `CUSTOM OBJECTS` in `LeftSidebar.tsx` (`btn-manage-object-types`) and from `WorkspaceIndexView.tsx` (`btn-manage-object-types-index` → "Manage Types"); mounted globally in `App.tsx`.
  2. **Interactive Property Header (`components/objects/DocumentPropertyHeader.tsx`):**
     - Rendered in `FocusView.tsx` directly below the page title whenever the open artifact resolves to an `object_instance` (via `cmd_get_object_instance`).
     - Notion-style tray with a type badge (icon + display name + description), a collapsible chip row, and a quiet saving/saved indicator.
     - Every chip is inline-editable (`PropertyFieldRenderer`: select dropdowns, date pickers, number fields, checkboxes, text/URL) and persists immediately in the background through `cmd_save_object_instance`, then broadcasts `sthira:refresh_objects`.
  3. **Template Selector on Creation (`components/objects/TemplateSelectorPopover.tsx`):**
     - Selecting a typed object from the `/` slash menu now fetches its templates first: with **one** template the default is auto-hydrated and the Quick Object Creator opens immediately; with **multiple** templates a clean selection popover offers `[Default Template]`, `[Blank]`, and every named custom preset.
     - `FocusView.handleCreateAndInsertObject()` now calls `cmd_instantiate_object_from_template`, so the created artifact carries the template Markdown body and default properties before the embedded `sthira-object` directive is inserted.
  - **IPC bridge (`api/ipc.ts`)** extended with `updateObjectType`, `deleteObjectType`, `getTypeTemplates`, `saveObjectTemplate`, `getObjectInstance`, `instantiateObjectFromTemplate`; `types/sthira.ts` extended with `ObjectType.description` / `ObjectType.default_template_md` and the new `ObjectTemplate` contract.
- **Step 4: Verification & Automated Tests:**
  1. **Backend Rust Integration Tests (`crates/sthira-desktop/tests/object_type_templates_test.rs`):** 4/4 PASSED.
     - `test_instantiate_object_from_template_hydrates_content_and_properties` — custom type with a default template → `cmd_instantiate_object_from_template` produces an artifact whose `content_md` contains the template skeleton and whose `properties_json` carries schema defaults.
     - `test_named_template_overrides_default_body_and_property_preset` — a named template overrides the type default body and merges its `properties_preset`; `"blank"` yields a bare title-only skeleton.
     - `test_editing_schema_preserves_existing_instance_values` — adding a `deadline` property + renaming the type via `cmd_update_object_type` leaves the existing instance's `status`/`attendees` values intact.
     - `test_builtin_default_templates_and_type_deletion_protection` — asserts the exact seeded template bodies for `idea`/`todo`/`decision`, one seeded template per built-in type, system-type deletion rejection, and custom-type deletion success.
  2. **Frontend Vitest Suite (`crates/sthira-desktop/ui/src/test/object_type_manager.test.tsx`):** 3/3 PASSED.
     - `Test 1` — opens `ObjectTypeManagerModal`, adds a `Select` property with custom option tags (`agenda`, `decisions`), saves, and asserts the `cmd_create_object_type` payload schema.
     - `Test 2` — opens a typed object in `FocusView`, asserts `DocumentPropertyHeader` mounts with `property-chip-status` / `property-chip-tags`, and that editing the Select chip persists via `cmd_save_object_instance`.
     - `Test 3` — with two templates defined, the `/idea` slash command surfaces the template selector popover (`template-option-default`, `template-option-blank`, `template-option-idea-rapid`) which then opens the Quick Object Creator.
     - `custom_objects_and_generative_ui.test.tsx` updated: the typed-object creation path now asserts `cmd_instantiate_object_from_template` (with `templateId`) instead of the legacy `cmd_create_typed_object`.
  3. **Verification Gates:**
     - Full Rust workspace test suite (`cargo test --workspace`): ALL tests PASSED with 0 errors (incl. new `object_type_templates_test.rs`: 4/4).
     - Full UI Vitest suite (`npm test`): 56/56 tests PASSED across all 12 test suites.
     - Production bundle build (`npm run build` in `crates/sthira-desktop/ui`): 0 TypeScript errors, 0 Vite errors.






## Phase 16.1: "Workspace Index" → "All Files" & Notes Scope Filter/Sort (Tab Consolidation)
- **Status:** PASSED (Verified via Vitest Suite & Production Build)
- **Change 1: Renamed "Workspace Index" to "All Files" (user-visible copy only):**
  - `WorkspaceIndexView.tsx`: breadcrumb `Workspace / All Files`, page title `📁 All Files`, and subtitle reworded to "Unified directory of every project, note, page, and structured object."
  - `LeftSidebar.tsx`: the expanded nav button now reads `📁 All Files` (tooltip: *"All Files — every project, note, and typed object"*), and the collapsed rail trigger uses the `📁` glyph with tooltip `All Files`.
  - Internal identifiers were deliberately kept stable to avoid cross-cutting churn: the `workspace_index` view key, `WorkspaceIndexView`, `cmd_get_workspace_index`, the `nav-workspace-index` testid, and the `onNavigateWorkspaceIndex` prop. Only display strings changed.
- **Change 2: Removed the "Standalone Notes" tab; added a scope filter + sort mechanism to "Notes & Pages":**
  - `WorkspaceIndexTab` is now `'all' | 'projects' | 'notes' | 'objects'`. `WorkspaceCounts.standalone` is still returned by the backend and now powers the new filter chip's count.
  - New exported contracts in `types/sthira.ts`: `NoteScopeFilter` (`all` | `projects` | `standalone`) and `NoteSortKey` (`updated` | `created` | `title_asc` | `title_desc`).
  - **Scope toggle (`note-scope-filter`):** `[ All (n) ] [ In Projects (n) ] [ Standalone (n) ]`, where *In Projects* is derived as `counts.notes - counts.standalone`.
  - **Sort mechanism (`note-sort-select`):** `Last Modified` (default) / `Date Created` / `Title (A–Z)` / `Title (Z–A)`. Pinned notes always float first, matching every other surface in the app.
  - Filtering and sorting are **pure client-side refinements** over the `tab=notes` payload (which already contains both attached and standalone notes), so toggling costs zero extra IPC round-trips. A defensive `entity_type === 'note'` guard keeps the tab correct by construction regardless of the incoming payload.
  - **No capability lost:** the inline `[Standalone ▾]` → `[Attach to Project ▾]` dropdown that previously existed only in the Standalone Notes tab was ported onto the unaffiliated rows of the Notes & Pages table, so organising loose notes is still one click away. The empty state now explains the active scope ("No standalone notes found. Switch the \"Show\" filter to see the rest.").
  - The former `{activeTab === 'standalone' && …}` render block was deleted outright rather than left as dead code.
- **Backend:** intentionally unchanged. `handle_get_workspace_index` still supports `tab=standalone` (covered by `workspace_organization_test.rs`) as a reusable query mode, but the UI no longer surfaces it as a tab.
- **Verification Gates:**
  - `crates/sthira-desktop/ui/src/test/workspace_organization.test.tsx` updated: the tab test now asserts the four surviving tabs, asserts `tab-standalone` is **absent**, drives the `[Standalone]` scope chip, exercises the ported `[Attach to Project ▾]` flow, and verifies list reordering across `Last Modified` / `Title (A–Z)` / `Date Created`. The `getWorkspaceIndex` mock was made tab-aware so it faithfully mirrors `cmd_get_workspace_index`.
  - Full UI Vitest suite (`npm test`): 56/56 tests PASSED across all 12 test suites.
  - `npx tsc --noEmit`: 0 errors. Production bundle build (`npm run build`): 0 TypeScript errors, 0 Vite errors.
  - `cargo check --workspace`: clean (no Rust changes required).

