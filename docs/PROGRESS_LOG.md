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


## Phase 17: Page Action Menu (`...`) — Project Association, Duplication, Conversion & Lifecycle
- **Status:** PASSED (Verified via Rust Integration Tests, Vitest React Suite & Production Builds)
- **Step 0: Core Engine Support (preserving epistemic contracts):**
  - Added `Engine::move_artifact_to_project(artifact_id, from_project_id, to_project_id)` to `crates/sthira-core/src/lib.rs`.
  - **Why this lives in the engine, not the command layer:** `Engine::log_event_tx` is crate-private, and the `session_events` table is strictly append-only (DB triggers reject UPDATE/DELETE). A raw SQL transaction in `sthira-desktop` would have silently dropped the `artifact_attached` / `artifact_detached` audit rows that resume briefs, project history and the reversibility engine all read. The relocation therefore runs as **one SQLite transaction** (soft-detach source → attach target → both audit events) inside the crate that owns the event log.
- **Step 1: Backend IPC Commands (`crates/sthira-desktop/src/commands.rs`, registered in `lib.rs`):**
  1. `cmd_link_artifact_to_project(artifact_id, project_id)` — join-table insert only; **zero content duplication** (artifact row, `content_md`, `content_hash` and `version` untouched). Idempotent (re-linking is a no-op) and fails loudly on a missing artifact rather than writing a dangling row.
  2. `cmd_move_artifact_to_project(artifact_id, from_project_id?, to_project_id)` — single-transaction reassignment via the new engine method; also handles standalone → project (`from = None`).
  3. `cmd_create_project_with_artifact(project_name, artifact_id)` — creates the project (trimmed name validated non-empty), records `projects.origin_artifact_id` for provenance, sets a purpose, attaches the note, and returns the new project id.
  4. `cmd_duplicate_artifact(artifact_id, new_title?, target_project_id?) -> Result<String, String>` — **extended in place** from the legacy `(artifact_id, target_project_id) -> Artifact` shape to the specified signature. Deep-copies the row via `create_artifact` (fresh UUID, timestamp, vault file, initial revision + `artifact_created` event), titles it `new_title` or `"{original_title} (Copy)"`, and attaches the copy to `target_project_id` (defaulting to the source's active project). Returns the new artifact id.
  5. `cmd_convert_artifact_to_object(artifact_id, object_type_id, initial_properties?)` — writes only an `object_instances` row. **The Markdown body is preserved byte-for-byte** (no re-extraction, no authorship laundering, no version bump). Property precedence: schema `default_value`s → `initial_properties` → existing instance values, so re-converting never destroys data.
  6. `cmd_detach_artifact_from_project(artifact_id, project_id)` — returns the note to the Standalone pool.
     - **Deliberate deviation from a hard `DELETE`:** the schema documents `project_artifacts.detached_at` as *"Soft detach preserves historical context"*, `Engine::attach_artifact` re-activates a detached row via `ON CONFLICT ... DO UPDATE SET detached_at = NULL`, and `tests/frozen/phase9_modes_registry.rs` asserts `detached_at IS NOT NULL` after a detach. A hard delete would rewrite history and break a frozen gate, so detach is implemented as a soft detach through `Engine::detach_artifact` (stamps `detached_at` + logs `artifact_detached`). It errors when the note is not actively linked instead of silently no-op'ing.


- **Step 2: Page Action Menu (`crates/sthira-desktop/ui/src/components/PageActionMenu.tsx`, 665 lines):**
  - Discrete three-dots trigger (`MoreHorizontal` + `•••` label in toolbar variant) styled in Warm Stone (`hover:bg-[#EFECE4] rounded-lg p-1.5`); `variant: 'toolbar' | 'row'` and `align: 'left' | 'right'`.
  - **Project Management:** `🔗 Link to Project...` (built-in searchable project picker, or delegated to the parent's `AddToProjectModal` when `onLinkProjectRequested` is supplied), `📦 Move to Project...` (picker, current project marked and disabled), `➕ Create Project with this Note` (inline name input + Create & Open, redirects context), `↩️ Detach from Project` (**only rendered when the note belongs to a project**).
  - **Duplication & Transform:** `📋 Duplicate Note` (calls `cmd_duplicate_artifact`, shows a toast with `[Open]`), `🏷️ Convert to Object Type ▾` (submenu listing every registered type — `Idea`, `Task`, `Decision`, `Source`, plus user-created types), optional `✏️ Rename` (when the parent passes `onRename`, used by the sidebar's inline-rename flow).
  - **Clipboard & Links:** `📄 Copy Markdown` (clipboard API with an `execCommand` fallback for restricted contexts), `🔗 Copy Note Link` (`sthira://artifact/{id}`).
  - **Destructive:** `🗑️ Delete Page...` → inline safe-deletion confirmation whose copy explains the real semantics (shared note ⇒ detached from this project only; otherwise purged), then calls `cmd_delete_artifact`.
  - Escape / outside-click dismissal, busy + inline error states, and a self-contained result toast.
  - **IPC bridge (`api/ipc.ts`)** extended with `linkArtifactToProject`, `moveArtifactToProject`, `createProjectWithArtifact`, `convertArtifactToObject`, `detachArtifactFromProject`; `duplicateArtifact` now returns `Promise<string>` (the new id) and accepts an optional `newTitle`.
- **Step 3: Mount Points:**
  1. **`FocusView.tsx`** — `<PageActionMenu variant="toolbar">` mounted in the document toolbar immediately to the right of the `[ ✏️ Edit ] ⟷ [ 👁️ Preview ]` toggle, hydrated with the live artifact id, title, `editorContent`, `projectId` and the loaded `objectTypes`. New optional `onOpenArtifact` prop (wired from `App.tsx`) lets the duplicate toast and "Create Project with this Note" redirect context.
  2. **`LeftSidebar.tsx`** — the two legacy per-row menus (project pages `btn-page-menu-*` and loose notes `btn-loose-page-menu-*`) were **replaced** by a single unified `PageActionMenu` per note row, hover-revealed via `opacity-0 group-hover/*:opacity-100 focus-within:opacity-100`. Trigger testids are preserved. `Rename` and `Link to Project` delegate back to the sidebar's existing inline-rename state and `AddToProjectModal`, so no capability was lost and the removed dead code (`handleDuplicateNoteAction`, `FolderPlus`, `deleteArtifact` imports) was pruned.
  3. **`WorkspaceIndexView.tsx`** — `PageActionMenu` added to every note row in the **Notes & Pages** tab and to non-project rows of the **All Items** tab, enabling Link / Duplicate / Delete straight from the dashboard.
- **Step 4: Verification & Automated Tests:**
  1. **Backend Integration Tests (`crates/sthira-desktop/tests/page_actions_test.rs`): 5/5 PASSED.**
     - `test_link_artifact_to_project_links_without_duplication` — exactly one active join row, artifact row count unchanged, `content_md`/`content_hash`/`version` identical, re-link idempotent, missing artifact rejected.
     - `test_create_project_with_artifact_creates_project_and_associates_note` — new project with trimmed name and `origin_artifact_id` set, note actively linked, blank name rejected.
     - `test_duplicate_artifact_produces_independent_note_with_fresh_uuid` — fresh UUID, artifact count +1, `(Copy)` default title, content parity, inherits project, and **editing the copy leaves the original untouched**; explicit title + explicit target project verified.
     - `test_convert_artifact_to_object_creates_instance_and_preserves_content` — `object_instances` row with schema defaults + supplied properties, body/hash/version byte-identical, re-convert preserves stored values, unknown type rejected.
     - `test_move_and_detach_artifact_between_projects` — source link inactive after move, target active, `detached_at` stamped (**row preserved, not deleted**), standalone → project move, detach returns to standalone, detaching an unlinked note errors, re-link reactivates the same row.
  2. **Frontend Vitest Suite (`crates/sthira-desktop/ui/src/test/page_action_menu.test.tsx`): 4/4 PASSED.**
     - Test 1 — opening the menu renders every option across all four groups (Detach present when the note is in a project).
     - Test 2 — clicking **Duplicate Note** invokes `cmd_duplicate_artifact` with `{artifactId, targetProjectId}` and surfaces the `[Open]` toast which forwards the new id to `onOpenArtifact`.
     - Test 3 — standalone notes hide Detach; `Link to Project...` opens the searchable picker, live search filters the list, and selecting a project calls `cmd_link_artifact_to_project`.
     - Test 4 — the Convert submenu lists registered types and calls `cmd_convert_artifact_to_object`.
     - `settings_autosave_realtime.test.tsx` updated for the unified menu testids (`page-action-duplicate`, `page-action-link`, `page-action-rename`) and the new string-returning `cmd_duplicate_artifact`.
  3. **Verification Gates:**
     - Full Rust workspace test suite (`cargo test --workspace --no-fail-fast`): **90 passed, 0 failed** (exit 0), incl. new `page_actions_test.rs`: 5/5.
     - Full UI Vitest suite (`npm test`): **60/60 tests PASSED across all 13 test suites**.
     - `npx tsc --noEmit`: 0 errors. Production bundle build (`npm run build`): 0 TypeScript errors, 0 Vite errors.

## Phase 18: Continuous & Scheduled Research / Planning Topics
- **Status:** PASSED (Verified via Rust Integration Tests, Vitest React Suite & Production Builds)
- **Step 1: Database Layer & Models (`crates/sthira-core`):**
  1. **SQLite Schema (`db/schema.sql` + `db/migrations.rs`, schema version 7):**
     - `scheduled_topics(id, title, prompt_query, topic_type ∈ {research, plan}, project_id → projects ON DELETE SET NULL, cadence ∈ {daily, weekly, manual}, model_id, search_provider_id, is_active, last_run_at, created_at)`.
     - `scheduled_topic_runs(id, topic_id → scheduled_topics ON DELETE CASCADE, artifact_id → artifacts ON DELETE CASCADE, sources_scraped_count, executed_at)`.
     - `idx_scheduled_topics_active(is_active, topic_type)` and `idx_scheduled_topic_runs_topic(topic_id, executed_at DESC)`.
     - Migration block `user_version < 7`; fresh databases jump straight to `PRAGMA user_version = 7`.
  2. **Models (`models/scheduled_topics.rs`):** `TopicType`, `TopicCadence` (with `due_interval_ms()`), `ScheduledTopic`, `ScheduledTopicRun`, plus the single shared cadence predicate `is_topic_due(cadence, last_run_at, now_ms)`.
  3. **Repository (`db/scheduled_topics_repo.rs`):** `create_topic`, `get_topic`, `list_topics_by_type` (optional type filter), `list_active_topics`, `update_topic_status`, `update_topic_last_run`, `record_topic_run`, `list_runs_for_topic`, `delete_topic`.
     - `delete_topic` cascades the run rows but **deliberately preserves the generated artifacts** — deleting a topic never destroys a digest the user already read.
- **Step 2: Autonomous Execution Engine & Scheduler:**
  1. **`engines/scheduled_runner.rs`:**
     - `execute_topic_run(engine, topic_id)` / `execute_topic_run_with_provider(engine, topic_id, Option<&dyn SearchProvider>)` — the injectable variant lets the whole autonomous path run deterministically offline.
     - **Research branch:** builds the **anti-duplication boundary** from prior run digests (`build_continuity_boundary` — the `- **statement**` findings of the last 5 runs, capped at 1400 chars), injects it into the inquiry as an explicit `CONTINUITY BOUNDARY` instruction, resolves the search backend (per-topic override → workspace default → DuckDuckGo fallback), runs the existing multi-query pipeline (`generate_search_queries` → `SearchProvider::search` → `scraper::scrape_and_clean_url` → `VerifiedQuote::verify`), and writes `Digest: {Title} — {YYYY-MM-DD}` with **`Authorship::External`**.
     - **Plan branch:** evaluates the linked project's recorded milestones (`history::get_project_history`) and unresolved dependencies (open `epistemic_nodes` joined through `project_artifacts`), synthesizes progress / completed milestones / blockers / next phase, and writes `Plan Review: {Title} — {YYYY-MM-DD}` with **`Authorship::AiGenerated`**. Degrades to a deterministic, fact-only review if synthesis fails or times out.
     - `format_date_utc(now_ms)` implements Hinnant's `civil_from_days` inline — no new date dependency.
     - Both branches attach the artifact to `project_id` when set, then `record_topic_run` + `update_topic_last_run` and emit `CoreEvent::ArtifactChanged`.
     - `list_due_topics(engine, now_ms)` — the single cadence evaluation used by both the scheduler and the tests.
  2. **`crates/sthira-desktop/src/scheduler.rs`:** `spawn_scheduled_topic_scheduler(app_handle, engine)` runs a `tauri::async_runtime::spawn` loop — once immediately on startup (so a window missed while the app was closed fires right away) and then every **30 minutes**. Due topics execute **sequentially** (each run performs multiple scrapes, so serialising keeps background work low-impact), and each completion emits `sthira://scheduled-topic-ready`. Wired into `main.rs` alongside `spawn_event_bridge`.
- **Step 3: Desktop IPC Commands (`commands.rs`, registered in `lib.rs`):**
  - `cmd_list_scheduled_topics(topic_type?) -> Result<Vec<ScheduledTopicView>, String>` — `ScheduledTopicView` carries resolved `project_name` and the `runs[]` history (each with its artifact title) for the UI.
  - `cmd_create_scheduled_topic(topic: CreateScheduledTopicInput) -> Result<String, String>` — validates non-empty title/objective, defaults cadence to `daily`, active by default.
  - `cmd_trigger_scheduled_topic_run(topic_id) -> Result<String, String>` — runs immediately, returns the created artifact id.
  - `cmd_toggle_scheduled_topic_active(topic_id, is_active) -> Result<(), String>`.
  - `cmd_delete_scheduled_topic(topic_id) -> Result<(), String>`.


- **Step 4: UI Enhancements in the Right Mode Rail (`crates/sthira-desktop/ui`):**
  1. **Sub-tab segmented control** added to both `ResearchPanel.tsx` and `PlanPanel.tsx`: `[ ⚡ Quick ]` ⟷ `[ ⏱️ Scheduled Topics ]`, preserving the existing on-the-fly streaming UI untouched on the Quick side. `RightModeRail` forwards `onOpenArtifact` into both panels (prop widened to accept `projectId?: string | null`).
  2. **`components/ScheduledTopicsList.tsx`:**
     - **Topic Creation Card** — title, detailed objective prompt, frequency selector (`Daily (Autonomous) | Weekly | Manual Only`), optional project dropdown, model-override dropdown (populated from the provider catalog), and `[ + Save Scheduled Topic ]` with a saving spinner.
     - **Active Topics List** — Warm Stone cards (`#FAF8F3` container, `#FFFFFF` cards, `#E8E5DC` borders) showing title, objective summary, cadence badge, project chip and relative last-run time; `[ 🔄 Run Now ]` with a live spinner, an Active/Pause toggle, and delete.
     - **Collapsible "Previous Digests / Reviews"** per topic listing past run artifacts with source counts; clicking a run opens the generated note directly in `FocusView`.
     - Subscribes to `sthira://scheduled-topic-ready` (via the new `onScheduledTopicReady` bridge) so autonomous runs raise an in-panel toast and refresh the list.
  - `types/sthira.ts` and `api/ipc.ts` extended with `ScheduledTopicView`, `ScheduledTopicRunView`, `CreateScheduledTopicInput`, `ScheduledTopicReadyEvent` and the five IPC wrappers.
- **Robustness fix found during testing (affects production, not just tests):**
  - The plan branch originally resolved the workspace default model through `providers_repo::get_default_model`, which falls back to *any enabled model* — the seeded Ollama entry. With Ollama not running, every autonomous plan run blocked for ~44s before degrading. Fixed by honouring the column's own contract (`model_id` = "Specific model override, or NULL for default"): the provider registry is now consulted **only when a topic explicitly sets an override**, otherwise the engine's configured model is used — exactly like the research branch. The synthesis call is additionally wrapped in a **90s bounded timeout** so a background job can never hang indefinitely. This took the plan-topic test from 44s to 0.03s.
- **Step 5: Verification & Automated Tests:**
  1. **Backend Integration Tests (`crates/sthira-core/tests/scheduled_topics_test.rs`): 6/6 PASSED.**
     - `test_create_list_and_filter_topics_by_type` — create / list / filter `research` vs `plan`, round-trip a topic, status toggle, delete (and delete-again errors).
     - `test_cadence_evaluation_flags_topics_older_than_24_hours` — the pure window logic (daily never-run/23h/25h, weekly 2d/8d, manual never due) **plus** scheduler-level `list_due_topics` enumeration, including that pausing removes a topic from the due set.
     - `test_execute_research_topic_produces_grounded_digest_with_citations` — mock `SearchProvider` + mock LLM + pre-seeded `web_research_cache`: asserts the provider was queried, the digest title carries the ISO date, **`Authorship::External`**, grounded-evidence and citation sections contain the real source domain, the note is attached to the linked project, and the run row + `last_run_at` are recorded with `sources_scraped_count > 0`.
     - `test_second_run_applies_continuity_boundary` — the first run leaves no boundary; its findings seed the boundary; the second run embeds `Previously Covered` + `Anti-duplication boundary applied` and the search planner receives the `CONTINUITY BOUNDARY` instruction.
     - `test_execute_plan_topic_produces_plan_review` — plan review title/date, **`Authorship::AiGenerated`**, progress/blockers/next-phase sections, project attachment, run recorded, and an unscoped (no-project) review still works.
     - `test_execute_unknown_topic_errors`.
  2. **Frontend Vitest Suite (`crates/sthira-desktop/ui/src/test/scheduled_topics.test.tsx`): 5/5 PASSED.**
     - Test 1 — `ResearchPanel` segmented control toggles Quick ⟷ Scheduled Topics (and the Quick editor unmounts/remounts).
     - Test 2 — `PlanPanel` segmented control toggles, and requests `topicType: 'plan'`.
     - Test 3 — submitting the creation card dispatches `cmd_create_scheduled_topic` with the correct payload, refreshes the list, renders the new card and resets the form.
     - Test 4 — `[ Run Now ]` dispatches `cmd_trigger_scheduled_topic_run` and surfaces the generated artifact via `onOpenArtifact`.
     - Test 5 — the Active/Pause toggle dispatches `cmd_toggle_scheduled_topic_active`, and expanding "Previous Digests" + clicking a run opens it in `FocusView`.
  3. **Verification Gates:**
     - Full Rust workspace test suite (`cargo test --workspace --no-fail-fast`): **96 passed, 0 failed** (exit 0), incl. new `scheduled_topics_test.rs`: 6/6.
     - Full UI Vitest suite (`npm test`): **65/65 tests PASSED across all 14 test suites**.
     - `npx tsc --noEmit`: 0 errors. Production bundle build (`npm run build`): 0 TypeScript errors, 0 Vite errors.

## Phase 18.1: Wall-Clock Scheduling ("run every day at 06:00")
- **Status:** PASSED (Verified via Rust Integration Tests, Vitest React Suite & Production Builds)
- **Motivation:** Phase 18 topics only knew a *rolling interval* ("24h since the last run"), so a daily digest drifted to whatever time it happened to last fire — a 06:00 topic could end up running at 14:00. Users now pick an explicit time of day (and a weekday anchor for weekly topics).
- **Step 1: Schema & Models (`crates/sthira-core`, schema version 8):**
  - Two additive columns on `scheduled_topics`:
    - `schedule_time_minutes INTEGER` — minutes since midnight (UTC), `0..=1439`, `CHECK`-constrained. `NULL` preserves the legacy rolling-interval behaviour, so **existing topics are unaffected by the migration**.
    - `schedule_weekday INTEGER` — ISO weekday anchor `0=Monday .. 6=Sunday`, `CHECK`-constrained; `NULL` anchors on the weekday the topic was created.
  - Migration block `user_version < 8` (two `ALTER TABLE ADD COLUMN`s); fresh databases go straight to `PRAGMA user_version = 8`.
  - `models/scheduled_topics.rs` gained the pure, dependency-free time arithmetic: `format_hhmm`, `parse_hhmm`, `iso_weekday_of` (1970-01-01 was a Thursday, hence `(days + 3).rem_euclid(7)`), `most_recent_scheduled_instant`, `next_scheduled_instant`, plus `ScheduledTopic::is_due(now_ms)` / `ScheduledTopic::next_run_at(now_ms)`.
- **Scheduling semantics (single implementation, shared by scheduler and tests):**
  - **Wall-clock mode** (`schedule_time_minutes` set) — a topic is due for the *most recent occurrence* of its configured moment, but only when that occurrence is **after `created_at`** and **after `last_run_at`**. The `created_at` floor is deliberate: a topic created at 10:00 asking for `06:00` waits until tomorrow instead of firing immediately on creation.
  - **Rolling mode** (`schedule_time_minutes` = `NULL`) — the Phase 18 behaviour (never-run ⇒ due, otherwise `now - last_run > interval`), retained for backward compatibility.
  - `daily` uses today's (or yesterday's) occurrence; `weekly` uses the most recent configured weekday (falling back to the creation weekday).
  - `manual` topics are never autonomously due and expose no next-run, even when a time is set. Paused topics are excluded.
- **Step 2: IPC (`crates/sthira-desktop/src/commands.rs`):**
  - `CreateScheduledTopicInput` gained `schedule_time: Option<String>` (`"HH:MM"`) and `schedule_weekday: Option<u8>`.
  - `handle_create_scheduled_topic` validates the time via `parse_hhmm` (rejecting `24:00`, `12:60`, malformed input with a clear error) and range-checks the weekday.
  - `ScheduledTopicView` gained `schedule_time` (`"HH:MM"`), `schedule_weekday`, `next_run_at` (server-computed) and a human `schedule_label` (`"Daily at 06:00 (UTC)"`, `"Weekly · Mon at 06:00 (UTC)"`, `"Manual only"`, `"Daily (rolling 24h)"`).
- **Step 3: UI (`ScheduledTopicsList.tsx`):**
  - Creation card: a `Run at (UTC)` `<input type="time">` (defaulting to `06:00`) plus an `On day` weekday selector that appears only for `weekly`. Both controls are hidden for `Manual Only`, and a `Fires: Every day at 06:00` hint summarises the choice for daily topics. The time is sent as `null` for manual topics.
  - Topic cards now render the `schedule_label` (with a calendar-clock icon) and a compact `Next in 7h` countdown from `next_run_at` (new `formatUntil` helper).
- **Known limitation (documented, not hidden):** times are evaluated in **UTC**, consistent with the existing `format_date_utc` digest-title convention. The engine clock is a UTC epoch with no tz database available, so local-time scheduling requires a tz-offset setting; the column and helpers are structured so adding one is a presentation-layer change.
- **Step 4: Verification & Automated Tests:**
  1. **Backend (`crates/sthira-core/tests/scheduled_topics_test.rs`): 11/11 PASSED** (5 new wall-clock tests).
     - `test_daily_wall_clock_scheduling_fires_at_configured_time` — not due at 05:30, due at 06:00, still due later that day if it never ran, **not** re-fired the same day after running, fires again at 06:00 the next day, and a topic created at 10:00 for `06:00` correctly waits until tomorrow. Also asserts `next_run_at`.
     - `test_weekly_wall_clock_scheduling_anchors_on_weekday` — the computed instant lands on a Monday, weekly topics don't re-fire within the same week, the next run is always a Monday, and a `NULL` weekday anchors on the creation weekday.
     - `test_hhmm_parsing_and_formatting_roundtrip` — valid parses, out-of-range rejections (`24:00`, `12:60`, `12`), and format/parse round-trips.
     - `test_manual_and_paused_topics_ignore_wall_clock_time` — manual topics never due and no next run; paused topics excluded from `list_due_topics`.
     - `test_wall_clock_topic_runs_end_to_end_and_stamps_last_run` — a 06:00 topic created yesterday is enumerated as due, executes end-to-end, stamps `last_run_at`, and is no longer due afterwards.
  2. **Frontend (`crates/sthira-desktop/ui/src/test/scheduled_topics.test.tsx`): 8/8 PASSED** (3 new).
     - Test 6 — the time picker shows for daily/weekly (with the weekday selector only for weekly) and disappears for manual.
     - Test 7 — the selected `schedule_time: '07:30'` and `schedule_weekday: 2` are dispatched inside `cmd_create_scheduled_topic`.
     - Test 8 — the topic card renders `Daily at 06:00 (UTC)` and the `Next in …` countdown.
  3. **Verification Gates:**
     - Full Rust workspace test suite (`cargo test --workspace --no-fail-fast`): **101 passed, 0 failed** (exit 0), incl. `scheduled_topics_test.rs`: 11/11.
     - Full UI Vitest suite (`npm test`): **68/68 tests PASSED across all 14 test suites**.
     - `npx tsc --noEmit`: 0 errors. Production bundle build (`npm run build`): 0 TypeScript errors, 0 Vite errors.


---

## Phase 19: Autonomous Research in the Center Workspace

**Goal:** Research results must open as a **new artifact in the center `FocusView`** (never trapped in the Right Mode Rail), and web search must be fully **autonomous and query-based** — no URL input — surfaced through a `[ 🌐 Web Search ]` mode pill and a live progress banner.

### Step 1: Core pipeline — real stage streaming + numbered citations (`crates/sthira-core/src/engines/research_engine.rs`)

- **`format_date_utc` hoisted to the crate root** (`crates/sthira-core/src/lib.rs`) via Hinnant's `civil_from_days`, so the scheduled-digest titles and the research pipeline share one date implementation. `scheduled_runner.rs` now delegates to `crate::format_date_utc`.
- **`WebSearchMode { LocalVaultOnly, Auto, AlwaysSearch }`** with `from_str_lossy` / `as_str`. Unknown values fail safe to `Auto`.
- **`ResearchStage { stage, message }`** and **`ResearchSource { index, url, domain, title, favicon_url }`**; `WebResearchResult` extended with `mode_used`, `stages` and `sources`.
- **Real-time stage emission (not post-hoc).** `execute_multi_query_research` takes `on_stage: Option<&(dyn Fn(ResearchStage) + Send + Sync)>` and pushes each stage at the *true* moment it runs: `searching` → `scraped` → `synthesizing` → `done`. `execute_research` wraps the caller's callback so the live `searching` banner already reads `Searching web for "…" via {provider}…` as it is emitted.
- **Numbered citation index.** A single `sources: Vec<ResearchSource>` backs every `[Source N]` marker; findings map URL → index through a lookup closure. The markdown body now emits `## Synthesis`, `## Contradictions & Evolution`, `## Grounded Web Evidence & Verified Findings` (with `[Source N]` + favicon), `## Sources`, epistemic hypotheses and open questions.
- **`synthesize_with_temporal_grounding(...)`** injects the exact instruction `"Today is {date}. Use the retrieved web context for current specifications, versions, and benchmarks. If the retrieved context contradicts older conventions, explicitly flag the difference…"`, wrapped in a **90s bounded timeout** with a deterministic evidence-only fallback that never fabricates a contradiction list.
- **Local-vault evidence path.** `inquiry_keywords`, `keyword_hits`, `search_local_vault(engine, inquiry, limit)` (LIKE on the top-3 keywords, re-ranked in Rust — no FTS5 exists in this schema) and `execute_local_vault_research(...)`, which grounds quotes through the **same** `VerifiedQuote::verify` contract against local artifacts (`sthira://artifact/{id}`), so vault-sourced and web-sourced findings are epistemically comparable.
- **Mode-aware dispatcher `execute_research(engine, inquiry, mode, provider, provider_label, limit_docs, on_stage)`.** `Auto` escalates to the web when vault coverage < 2 hits; `Auto`/`AlwaysSearch` **degrade to the vault** (with a stage message) when no provider is configured rather than failing the run.



### Step 2: IPC — create the page, then fill it (`crates/sthira-desktop/src/commands.rs`)

- **`cmd_create_research_page(query, project_id) -> String`** creates a draft artifact titled `Research: <Query>` with `Authorship::External` (Phase 10 convention — web-derived research is `External`, which is what lets grounded quotes reach `Provenance::SourceBacked`), forces `is_unfinished_draft = 1`, and optionally attaches it to a project. It returns the id **immediately** so the UI can navigate before any slow work starts.
- **`cmd_run_research_into_page(artifact_id, query, web_search_mode, on_progress: Channel<ResearchStage>)`** resolves the default `SearchProvider`, forwards every real stage over the Tauri `Channel`, writes `synthesized_markdown` into the existing artifact, clears `is_unfinished_draft`, and emits `CoreEvent::ArtifactChanged`. **No URL parameter exists anywhere in this signature.**
- Both registered in `crates/sthira-desktop/src/lib.rs`.

### Step 3: Frontend — center navigation, live banner, mode pill

- **`types/sthira.ts`**: `WebSearchMode`, `ResearchStage`, `ResearchSource`, `ResearchRunState`, `ScrapedWebDocument`. `WebResearchResult` / `WebResearchFinding` were **re-aligned to the actual Rust contract** — they previously mirrored a browser mock (`original_prompt`, `generated_queries`, `provider_used`, `finding.url`, `markdown_excerpt`), which would have rendered `undefined` against the real backend.
- **`api/ipc.ts`**: `createResearchPage(query, projectId)` and `runResearchIntoPage(artifactId, query, mode, onStage)` using a Tauri `Channel` (camelCase + snake_case arg keys, matching the existing `cmd_stream_adaptive_run` convention), plus a browser-preview fallback that replays the same stage narrative.
- **`App.tsx` — `handleStartResearch(query, mode, projectId)`** is now the single research entry point: create page → `setActiveArtifactId` → `setCurrentView('focus')` **immediately** → stream stages into `researchRun` → on completion bump `refreshToken` and reload state. It returns the result so the rail can still render the citation list alongside. A `researchRun` prop is passed to `FocusView`.
- **`FocusView.tsx`**: a live progress banner mounted at the top of the center document canvas (`data-testid="research-progress-banner"`), showing the mode, the streaming message and the inquiry, with a spinner while running, a verified shield on completion, an error state, and a dismiss button. A `refreshToken` effect reloads the body so the grounded document replaces the draft placeholder.
- **`RightModeRail.tsx`**: the legacy `Inquiry or URL to Investigate` label and the binary `🌐 Include Web Search` toggle were **removed** and replaced by a plain prompt (`What should Sthira research?`) plus a 3-way `[ 🌐 Web Search: … ▾ ]` pill (`Local Vault Only` / `Auto` / `Always Search Web`). Submitting calls `onStartResearch`, and the fabricated hardcoded `claims` / `openQuestions` arrays were deleted in favour of real grounded findings.
- **`ResearchPanel.tsx`**: carries its own `[ 🌐 Web Search ]` pill and a `Research in Workspace` action that routes through `onStartResearch` (distinct `panel-*` testids, since the panel and the rail form render together).
- The `/research` slash command in `FocusView` already streams inline into the center editor, so it needed no change.

### Step 4: Verification & Automated Tests

1. **Backend (`crates/sthira-core/tests/research_engine_test.rs`): 6/6 PASSED** (new file, offline `MockSearchProvider` + seeded scrape cache).
   - `test_query_without_url_drives_autonomous_search_and_grounded_citations` — a bare question (asserted to contain no `http`) drives the provider; the provider is only ever handed model-generated queries; stages arrive **live and in order** (`searching`/`scraped`/`synthesizing`) with the provider name and source count; the document carries `[Source 1]`, a `## Sources` index, real URLs, favicons, the fake-clock date and the `## Contradictions & Evolution` block.
   - `test_local_vault_only_never_calls_the_search_provider` — provider call count is exactly 0; `## Local Vault Evidence` is emitted with `sthira://artifact/{id}` sources.
   - `test_auto_mode_escalates_only_when_vault_coverage_is_thin` — covered vault stays local (0 provider calls); a thin vault escalates.
   - `test_missing_provider_degrades_to_vault_without_failing` — degrades with a stage notice instead of erroring.
   - `test_web_search_mode_parsing_and_keyword_extraction` and `test_local_vault_search_ranks_by_keyword_overlap`.
2. **Frontend (`crates/sthira-desktop/ui/src/test/research_center_workspace.test.tsx`): 3/3 PASSED** (new file).
   - Creates a page, navigates to `FocusView`, and asserts the banner is inside the **center** canvas (`closest('.max-w-4xl')`) rather than the rail; drives the stage stream by hand and asserts each message appears live; asserts completion flips the banner, exposes dismiss, and loads the grounded `## Synthesis` / `[Source 1]` body into the center editor.
   - Asserts the selected pill mode (`always_search`) is forwarded to the pipeline.
   - Asserts the `ResearchPanel` pill routes into the center workspace.
3. **Updated:** `search_providers_and_rail.test.tsx` (pill replaces the toggle, submits via `onStartResearch`) and `scheduled_topics_test.rs` (`## Sources` / `[Source N]` / temporal-grounding assertions after the markdown restructure).
4. **Verification Gates:**
   - Full Rust workspace (`cargo test --workspace --no-fail-fast`): **107 passed, 0 failed** (exit 0), verified across three consecutive runs.
   - Full UI Vitest suite (`npm test`): **71/71 tests PASSED across all 15 test suites**.
   - `npx tsc --noEmit`: 0 errors. Production bundle build (`npm run build`): 0 TypeScript errors, 0 Vite errors.

### Deviations & notes (flagged, not hidden)

- **`Authorship::ExternalSource` does not exist.** The enum is `User | AiGenerated | External`; this phase uses `Authorship::External`, consistent with Phase 10 and `docs/plan.md`.
- **The progress log lives at `docs/PROGRESS_LOG.md`**, not `.agent_state/PROGRESS_LOG.md` — that path does not exist in this repository.
- **Pre-existing UI/backend contract drift was fixed as part of this phase.** The TS `WebResearchResult` mirrored a browser mock rather than the Rust response, so real Tauri builds would have shown blank citations. This is now aligned and covered by tests.
- **`vitest.config.ts` gained `testTimeout: 20000`.** With 15 suites each spinning up an independent jsdom environment in parallel, full-`App` tests intermittently exceeded the 5s default and flaked (observed in the pre-existing `interactive_ui` and `deletion_and_graph` suites, both unrelated to research). The added headroom makes the suite deterministic — verified green across three consecutive full runs.



---

## Phase 20: Right-Rail Elimination — Center-Workspace Streaming & Scheduled Feeds View

**Goal:** Eliminate the persistent Right Rail drawer. All research, planning and synthesis must stream and display directly inside the full-width center document editor (`FocusView.tsx`). Move Scheduled Topics to a dedicated view reachable from the Left Sidebar.

### Step 1: Removed the persistent Right Mode Rail

- **Deleted `RightModeRail.tsx`, `ResearchPanel.tsx` and `PlanPanel.tsx`** outright (and their now-orphaned `activeDocumentContent` state in `App.tsx`).
- **`App.tsx`**: the rail JSX is gone. The center container is now `flex-1 h-full min-w-0 ... bg-[#FAF8F3]` and carries `data-testid="center-workspace"`, so it takes **all** available width between the Left Sidebar and the window border with no reserved right-hand margin.
- **Deviation (flagged):** the container keeps its view-aware overflow (`currentView === 'focus' ? 'overflow-hidden' : 'overflow-y-auto'`) rather than a blanket `overflow-hidden`. `HomeView` and `WorkspaceIndexView` rely on the parent `<main>` for scrolling, so forcing `overflow-hidden` would have made those views unscrollable. Width/`flex-1` is what the requirement targeted and that is fully satisfied.

### Step 2: Research & Plan now route straight into the center workspace

- **`DocumentRunState`** (new, in `types/sthira.ts`) generalises the Phase 19 `ResearchRunState` to cover both generators: `kind: 'research' | 'plan'`, `message`, and `streamedMarkdown` for live token streaming. `ResearchRunState` remains as a deprecated alias.
- **`App.tsx`** now owns `documentRun` and exposes two entry points:
  - `handleStartResearch(query, mode, projectId)` — calls `cmd_create_research_page`, sets `activeArtifactId`, switches `currentView` to `'focus'` **before** any slow work, then streams the pipeline's real stages into the banner.
  - `handleStartPlan(goal, projectId)` — calls `cmd_create_page` with title `Plan: <Goal>`, navigates immediately, writes a draft body, then calls `streamAdaptiveRun` and accumulates `text_token` chunks into `documentRun.streamedMarkdown` so tokens render live in the page, finally persisting the finished plan with `saveArtifact`.
- **`UniversalInputBar.tsx`**: gained `onStartResearch` / `onStartPlan`. Pressing **Enter** in `Research` or `Plan` mode now navigates straight to the workspace — there is no intermediate `[Research in Workspace]` confirmation step (that button lived in the deleted `ResearchPanel`). The legacy `createPage`+`saveArtifact` path is retained as a fallback only when no center-workspace handler is wired, which keeps the existing `UniversalInputBar` unit tests valid.
- **`HomeView.tsx`** threads both handlers down to the input bar.
- **`FocusView.tsx`**: the banner was generalised to `data-testid="document-run-banner"` / `document-run-message` with `data-run-kind`, showing `Planning` (Sparkles) or `Web Search` / `Local Vault Research` (Globe), the live indicator, the inquiry/goal, and a dismiss control. A new `data-testid="document-live-stream"` block renders accumulated `streamedMarkdown` **inside the center document canvas** while a `plan` run is still generating.


### Step 3: Scheduled & Continuous Topics relocated to the Left Sidebar

- **New `views/ScheduledTopicsView.tsx`** — a full-width center-workspace dashboard with a header, then two sections (`data-testid="scheduled-section-research"` and `scheduled-section-plan`), each wrapping a `ScheduledTopicsList` instance. That component already owns the add-feed form, the cadence selector, the `Run at (UTC)` time picker, the weekday selector, the active/paused toggles and the previous-run digests that link into `FocusView`.
- **`LeftSidebar.tsx`**: added a `⏱️ Scheduled Feeds` navigation item directly beneath `All Files` (and a matching icon in the collapsed rail), with a new `onNavigateScheduledTopics` prop and `data-testid="nav-scheduled-feeds"` / `nav-scheduled-feeds-collapsed`.
- **`App.tsx`**: `currentView` union gained `'scheduled_topics'`; `handleNavigateScheduledTopics` clears the active artifact/object and renders `<ScheduledTopicsView />` inside the center workspace.

### Step 4: Verification & Automated Tests

1. **Frontend (`crates/sthira-desktop/ui/src/test/research_center_workspace.test.tsx`): 4/4 PASSED.**
   - **Research from the Universal Input Bar** — pressing Enter in `Research` mode calls `cmd_create_research_page`, `activeView` transitions to the workspace, the artifact is loaded, the run banner is `data-run-kind="research"`, and it is asserted to live **inside** `[data-testid="center-workspace"]` and inside the center document canvas (`closest('.max-w-4xl')`). Stages are driven by hand and each message is asserted live; completion flips the banner and loads the grounded `## Synthesis` / `[Source 1]` body into the editor.
   - **Plan mode** — asserts `cmd_create_page` was called with title `Plan: <Goal>`, the banner is `data-run-kind="plan"`, pushed `text_token` chunks render live in `document-live-stream` **inside the center canvas**, and the finished plan is persisted via `saveArtifact`.
   - **Structural / no drawer** — asserts the center workspace has `flex-1` and `min-w-0`, carries **no** `mr-*`/`pr-*` right-rail margin, is the *immediate* next sibling of the sidebar (`sidebar.nextElementSibling === center`), and has nothing drawer-like (`ASIDE`) rendered after it. Also asserts the removed rail's surfaces (`research-query-input`, `web-search-mode-pill`, `panel-web-search-mode-pill`) no longer exist anywhere in the shell.
   - **Scheduled Feeds navigation** — clicking `nav-scheduled-feeds` renders both feed sections inside the full-width center workspace.
2. **Updated:**
   - `scheduled_topics.test.tsx` — the two `ResearchPanel`/`PlanPanel` segmented-control tests were replaced with tests asserting `ScheduledTopicsView` renders both research and plan feeds and queries both topic types (`cmd_list_scheduled_topics` with `topicType: 'research'` and `'plan'`). The remaining `ScheduledTopicsList` tests are unchanged.
   - `search_providers_and_rail.test.tsx` — the `RightModeRail` test was removed along with the component; the Phase 10 Web Search Providers test is unchanged.
3. **Two genuine test-harness gaps fixed (removes all unhandled-error noise):**
   - `universal_input_modes.test.tsx` mocked `@tauri-apps/api/core` with only `invoke`, but `streamAdaptiveRun` constructs a Tauri `Channel` — added the missing `Channel` stub.
   - `ScheduledTopicsList` subscribes via `onScheduledTopicReady`, which calls the real `listen()`. Because `setup.ts` makes `isTauri` truthy, that rejected as an **unhandled rejection** under jsdom. Stubbed `onScheduledTopicReady` in both suites that mount the dashboard.
4. **Verification Gates:**
   - `cargo test --workspace --no-fail-fast`: **107 passed, 0 failed** (no Rust changes this phase; confirms no regression).
   - `npm test` (Vitest): **71/71 tests PASSED across all 15 test suites, 0 unhandled errors**.
   - `npx tsc --noEmit`: 0 errors. `npm run build`: 0 TypeScript errors, 0 Vite errors.

### Deviations & notes (flagged, not hidden)

- **`cmd_create_artifact` does not exist in the backend.** Plan drafts are created with the existing `cmd_create_page` (surfaced as `createPage`), which is the equivalent draft-creation command; research uses `cmd_create_research_page`. Both satisfy the requirement to initialise a draft artifact titled `Research: <Query>` / `Plan: <Query>`.
- **Center-container overflow is view-aware, not a blanket `overflow-hidden`** — see Step 1. Home and All Files depend on the parent container for scrolling.
- **`ResearchRunState` was kept as a deprecated type alias** to `DocumentRunState` so any external call sites continue to type-check.
- The `HomeView` empty-state "Research"/"Plan something" links still open the guided `InteractiveFlowModal`. That is a distinct guided modal (not a side drawer) and was left untouched; the requirement about Enter-to-navigate applies to query submission and is fully satisfied.

## Phase 21: Unified "Explore & Plan" Hub (`ExplorePlanView`)
- **Status:** PASSED (Verification Gates Passed: Vitest 76/76, cargo test 107/107, npm run build 0 errors)

### Step 1: Sidebar Renaming & Routing
- Renamed the Left Sidebar navigation entry from `Scheduled Feeds` to `🧭 Explore & Plan` (`data-testid="nav-explore-plan"`), retaining `data-testid="nav-scheduled-feeds"` as an alias for full backward compatibility with earlier test suites.
- Added `onNavigateExplorePlan` prop to `LeftSidebar.tsx` and updated collapsed and expanded button triggers.
- Extended `currentView` in `LeftSidebar.tsx` and `App.tsx` with `'explore_plan'`.
- Routed `handleNavigateExplorePlan` to render `<ExplorePlanView />` in the center workspace.

### Step 2: Tabbed Workspace Implementation (`ExplorePlanView.tsx`)
- Created `crates/sthira-desktop/ui/src/views/ExplorePlanView.tsx` with top segmented tabs matching `WorkspaceIndexView` (`All Files` design):
  1. **Tab 1: `[ 🔍 Research ]`**:
     - Full-width input card ("What would you like to research?") with `Auto` / `Enabled` / `Local Vault Only` Web Search mode toggle, model selector pill, project attachment selector, and `[ 🔍 Start Research ]` button.
     - Submitting creates the research page and immediately navigates/streams into `FocusView` in the center workspace.
     - "Recent Research" list (last 3 items) with `View all research notes →` button that switches to the `All Digests` tab filtered by research type.
  2. **Tab 2: `[ 📋 Plan ]`**:
     - Full-width planning input card ("What project, feature, or milestone set are we planning?") with model selector pill and project selector.
     - Submitting creates the plan artifact and streams milestone synthesis into `FocusView`.
     - "Recent Plans" list with `View all plans →` button that switches to the `All Digests` tab filtered by plan type.
  3. **Tab 3: `[ ⏱️ Trackers ]`**:
     - Badge displaying count of active recurring topics (`data-testid="badge-active-trackers"`).
     - Wide grid of active and background trackers (`data-testid="trackers-grid"`), organized into Research and Plan sections (`data-testid="scheduled-section-research"` and `scheduled-section-plan`).
     - Actions on each card: `[ 🔄 Run Now ]` (`cmd_trigger_scheduled_topic_run`), `[ ⏸️ Pause ]` / `[ ▶️ Resume ]` (`cmd_toggle_scheduled_topic_active`), and `[ 🗑️ Delete ]` (`cmd_delete_scheduled_topic`).
     - `[ + New Tracker ]` button opening a modal to configure a recurring topic (title, prompt, daily/weekly frequency with time and weekday, project attachment).
  4. **Tab 4: `[ 📚 All Digests ]`**:
     - Filterable table of all generated artifacts where `artifact_type IN ('research', 'plan')`:
       - Columns: `Title`, `Type` (Research badge / Plan badge), `Project` chip, `Date Generated`, and `•••` action menu (`PageActionMenu`).
       - Search filter bar at the top to quickly find past research topics.
       - Type filter pills (`All`, `Research`, `Plan`).
       - Clicking any row opens that note in `FocusView`.

### Step 3: Verification & Automated Tests
- **Vitest (`crates/sthira-desktop/ui/src/test/explore_plan_view.test.tsx`): 5/5 PASSED.**
  - Navigating to `Explore & Plan` and switching across all 4 tabs (`Research`, `Plan`, `Trackers`, `All Digests`).
  - Clicking `View all research notes →` switches to the `All Digests` tab filtered by research type.
  - `Trackers` tab renders existing topics and triggers `cmd_trigger_scheduled_topic_run`.
  - Submitting research inquiry triggers `onStartResearch` and opens in `FocusView`.
  - New Tracker modal opens and dispatches `cmd_create_scheduled_topic`.
- **All 16 Vitest test suites: 76/76 tests PASSED (0 failures, 0 regressions).**
- **`cargo test --workspace`: 107/107 tests PASSED.**
- **`npm run build`: 0 TypeScript errors, 0 Vite errors.**

