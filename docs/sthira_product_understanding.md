# Sthira — Product Understanding (v4)

**Status:** Canonical product understanding, experience specification & architecture manual  
**Last updated:** November 2026

This document captures the product vision, experience decisions, and epistemic invariants for **Sthira**. It serves as the canonical source of truth for product behavior, guiding principles, and resolved experience mechanics.

---

## 1. What Sthira is

Sthira is an **AI-native, local-first knowledge workspace** for research, thinking, planning, and durable knowledge.

It is more than a notes app with AI bolted on, or a chatbot surrounded by folders. Its aim is to bring a user's questions, research, sources, notes, conclusions, plans, and ongoing work into one coherent environment.

A user can:
- Explore a question and collect useful evidence.
- Think through a problem and capture ideas as they arise.
- Grow a loose thought or note organically into a full project without recreating or migrating it.
- Turn research and understanding into decisions, plans, or projects.
- Preserve the underlying material and reasoning as durable knowledge.
- Return later after an interruption and immediately recover the context of their work.
- Reuse knowledge in a different project without recreating or copying it unnecessarily.
- Interact with structured objects (Ideas, Tasks, Decisions, Sources, custom schemas) in Tables and Kanban boards.
- Explore cross-project connections through an interactive 2D physics Knowledge Graph.
- Conduct privacy-first, autonomous web research grounded in verified external quotes and rendered directly into the center document workspace.
- Experience adaptive progressive streaming that shifts conditionally between raw Markdown and interactive generative UI widgets.
- Track evolving questions autonomously over time with continuous topic trackers running on wall-clock schedules (`ExplorePlanView`).
- Refine existing notes with targeted critique prompts and 1-click snapshot rollback, or derive discrete execution plans, ideas, and action checklists with full epistemic lineage tracking (`FocusView`).
- Define custom typed object schemas and pre-filled Markdown templates with Notion/Anytype-style properties.
- Browse, filter, and organize all projects, notes, and objects in a unified "All Files" directory.
- Know model capabilities at a glance with explicit web search capability detection and badging (`Native`, `AgenticToolCall`, `EngineAugmentedOnly`, `Unsupported`).

The experience makes research, planning, and knowledge storage feel like connected parts of the same activity rather than separate apps or disconnected workflows.

### Working name
**Sthira (स्थिर)** is the working name. Its Sanskrit-rooted meaning evokes steadiness, stability, firmness, and continuity. The name's meaning is expressed through a calm, dependable product experience—not through decorative cultural motifs.

> **Tagline:** Knowledge that endures. Intelligence that evolves.

---

## 2. Product vision and intended feeling

Sthira must feel:
- **Calm:** It does not constantly demand attention, use guilt-oriented copy, or create artificial urgency.
- **Capable:** It helps people move from an initial question to deep understanding and action.
- **Trustworthy:** It strictly distinguishes user-recorded information, external source evidence, and AI inference—and prevents AI-generated text from being laundered into "user-recorded truth."
- **Exploratory:** Users can follow curiosity without needing a rigid plan or template first.
- **Steady:** Users can return after days or weeks away and recover their bearings in seconds.
- **User-owned:** The user controls the structure of their workspace, approves or dismisses structural changes, and can reverse any AI-assisted action.

### Core Product Promise
> Help me remember where I was, without making me revisit everything I did.

---

## 3. Guiding principles

### 3.1 Resume first
When a user returns, Sthira first helps them regain context. It never overwhelms them with every project detail, every document, or an uncurated activity log.

### 3.2 Brief by default; detail on demand
The default resume experience is a concise **Level 1 Quick Brief**. Users can request an expanded **Level 2 Detailed Review** when they want deeper context and direct links into their knowledge base.

### 3.3 The knowledge base preserves the details
The resume brief is an orientation and navigation aid—not a replacement for source documents. Detailed research, decisions, notes, and source material remain in the underlying knowledge base. Every claim in a brief or review must point back to verifiable artifacts.

### 3.4 Don't create maintenance work
Users never have to manually maintain a separate summary or status doc just to make resume functionality work. Sthira derives and caches resume briefs automatically from preserved work, invalidating and refreshing them deterministically whenever underlying notes or decisions change.

### 3.5 User owns the structure
Sthira may propose relationships, organization, purpose pivots, or next steps, but never silently imposes them. All structural AI suggestions are queued as inspectable, dismissible proposals (`pending` -> `accepted` | `dismissed`), and dismissed proposals are never nagged back.

### 3.6 Progressive complexity
The product is lightweight at the start and reveals richer structure only when needed. A quick thought can be captured immediately without picking a project or template, and can later be promoted into a project while preserving its full history, revisions, and extracted insights.

### 3.7 Durable knowledge, optional conversations
Transient questions and answers can happen without cluttering the workspace. If a user saves an AI-generated answer as a note, Sthira preserves its **AI authorship** (`ai_generated`) so that future extraction passes never mistake AI prose for a user's own recorded decision unless the user explicitly adopts or records it.

### 3.8 AI is transparent, provenance-locked, and reversible
Sthira enforces a strict three-tier epistemic provenance model across all notes, briefs, and history views:
1. **User-Recorded (`user_recorded`):** Explicitly written by the user in a user-authored note and backed by a verbatim quote span. Speculative or hedged thoughts (*"Maybe we should..."*, *"Could X work?"*) are classified as hypotheses or open questions—never as recorded decisions.
2. **Source-Backed (`source_backed`):** Drawn from an external article, paper, benchmark, or URL (`external` authorship) and backed by a verbatim quote span.
3. **AI-Inferred (`ai_inferred`):** Synthesized interpretations, pattern detections (such as a suspected pivot in project direction), or suggested connections. An AI inference can never be stored or displayed as a recorded decision.

Every non-trivial mutation (whether initiated by AI with permission or via bulk organization) is wrapped in a reversible **Change Set** so the user can inspect and undo it cleanly.

### 3.9 Avoid artificial productivity pressure
Sthira never uses streak counters, overdue badges, or guilt-inducing language (e.g., *"You haven't worked on this in 5 days"* or *"You haven't taken any notes yet"*). When a project has sparse activity, Sthira states what exists neutrally and factually.

---

## 4. Overall workspace experience

Sthira offers fluid, interconnected modes of engaging with the same underlying workspace:

### 4.1 Home — Orient yourself
Home provides a calm, focused launchpad separating active threads from quick capture:
- **Decluttered Layout:** The Home screen is strictly limited to high-signal items (max 3 active projects and max 3 recent documents), eliminating unfiled clutter and distracting note lists.
- **Unified Top Bar:** Clean header featuring brand indicator, Command Palette shortcut (`⌘K`), inline model selector chip, and Settings button (`⚙️`).
- **Active Threads ("Pick Up"):** Ordered by **last substantive work** (ignoring trivial views or whitespace edits), surfacing each project's continuation point or primary unresolved question.
- **Universal Input Card:** A single, central entry point for capturing spontaneous thoughts, asking transient questions, initiating research, planning, or resuming projects. Submitting in `Research` or `Plan` mode navigates immediately to the center workspace.
- **Inline Compute Provider Chip:** Select active local or cloud models (`[ 🦙 Ollama: llama3.2 ▾ ]`, `[ 🚀 Groq: llama-3.3-70b ▾ ]`, `[ ⚡ Anthropic: claude-3-7-sonnet ▾ ]`) with model search capability badging and 1-click settings shortcuts.
- **Calm Proposal Indicator:** A quiet, non-intrusive count of pending AI suggestions—never noisy popups or red warning badges.

### 4.2 Canvas — Think and organize spatially
A flexible spatial canvas lets users arrange notes, sources, text cards, and frames freely.
- **Strict Spatial/Semantic Isolation:** Placing two items next to each other on a canvas **never** automatically creates a semantic knowledge link or biases the AI discovery engine.
- **Reversible Layout Revisions:** If the user asks Sthira to help organize or cluster a canvas, the previous layout is snapshotted in a reversible Change Set (`cmd_revert_change_set`).

### 4.3 Focus — Go deep
A distraction-free, full-width canvas for reading, writing, and synthesizing within a document or project:
- **Full-Width Canvas & Collapsible Epistemic Margin:** The Epistemic Margin is collapsed by default (`showMargin: false`) allowing the editor canvas to expand to full width (`flex-1 max-w-5xl mx-auto`). The top toolbar button `[ 📖 Margin ]` toggles the margin open and closed on demand.
- **Clean Binary Workspace Toggle:** Seamlessly switch between `[ ✏️ Edit ]` (distraction-free Markdown editor with caret-positioned `/` slash menu) and `[ 👁️ Preview ]` (Component-Decorated Rich Workspace) via the top-right toolbar or the `Cmd/Ctrl + E` keyboard shortcut.
- **Component-Decorated Rich Document Renderer:** The preview renders standard Markdown decorated with interactive components:
  - **Interactive Comparison Tables:** Rich Warm Stone matrix with real-time row filtering and a 1-click `[📋 Copy Table]` clipboard button.
  - **Interactive Checklists:** Styled checkboxes (`- [ ]` / `- [x]`) that can be clicked to toggle state, automatically auto-saving back to the document via debounced save.
  - **Epistemic Decision Callouts:** Styled callouts with solid Sage borders, Soft Sand backgrounds, and an inline `[Add to Decisions]` action chip.
  - **Embedded Custom Objects:** Fenced `sthira-object` blocks render as warm stone cards displaying live property pills synced to SQLite.
  - **Mermaid Diagrams:** Fenced `mermaid` blocks dynamically execute diagrams with Warm Stone theme colors.
- **Interactive Document Property Header:** Displayed directly beneath the page title for typed objects (`Idea`, `Task`, `Decision`, `Source`, custom schemas) with live-editable property pills that persist immediately to SQLite.
- **Targeted Re-Research & Note Derivation (Chained Synthesis):**
  - **`[ 🔄 Refine ]` Modal:** Iteratively refines an existing note with user critique and optional web search. Snapshots the prior version in a reversible `ChangeSet` for **1-click undo**.
  - **`[ ⚡ Derive ▾ ]` Dropdown:** Derives structured child artifacts (`📋 Derive Execution Plan`, `💡 Extract Discrete Ideas`, `☑️ Extract Action Checklist`) with parent-child lineage tracking.
  - **Lineage Header Chip:** Derived notes render a prominent header chip: `🔗 Derived from: [Parent Artifact Title]`, allowing 1-click navigation back to the source note.
- **Center-Workspace Streaming Banner:** Research and Plan generators stream live stage updates (`searching` $\rightarrow$ `scraped` $\rightarrow$ `synthesizing` $\rightarrow$ `done`) and tokens directly inside the center document canvas.
- **Debounced Auto-Save (600ms):** Commits edits to both SQLite and disk on typing settle, input blur, and view navigation with a live status pill (`Saving...` $\rightarrow$ `Saved` $\rightarrow$ `Auto-saved`).
- **Real-Time Sidebar Title Sync (0ms):** Typing a heading or title synchronizes with the navigation sidebar instantly on every keystroke.
- **Automatic Line 1 Heading Derivation:** If an artifact title is empty, line 1 (`# Heading`) is extracted automatically.
- **Contextual Resume Pane:** Auto-expands on cold return ($\ge 48$ hours) or unresolved decision conflicts.

### 4.4 Knowledge Graph — Explore connected structure
An interactive 2D force-directed physics graph view (`GraphView`):
- **Node Classification:** Projects (prominent hub nodes), Pages/Notes (Warm Stone cards with draft badges), and Typed Objects (Ideas, Tasks, Decisions, Sources).
- **Edge Relationships:** Structural containment (`belongs_to`) and semantic connections (`supports`, `contradicts`, `extends`, `references`).
- **Interactive Controls:** Smooth pan, mouse-wheel zoom, drag-to-pin simulation, entity filter pills, real-time search query isolation, and floating details cards with direct jump-to-editor actions.

### 4.5 Workspace Navigation Shell (Sidebar & Center Workspace)
- **Left Hierarchy Sidebar:** Collapsible navigation tree with customizable width (persisted to `localStorage`). Displays:
  - **Top Navigation:** `📁 All Files` (master directory of all items with Scope Filter and Sort) and `🧭 Explore & Plan` (unified research, planning, trackers, and digests hub).
  - **Projects Tree:** Expandable project folders with child pages, canvases, and `+ New Page` actions. Capped at 7 items with pinning (`📌`).
  - **Notes & Pages:** Standalone notes capped at 7 items with pinning and section header `View all →` link to All Files.
  - **Custom Objects:** Object collections filtered by registered type with quick creator gear icon.
  - **Unified Page Action Menu (`...`):** Hover-revealed three-dots menu on every note row for instant Link, Move, Duplicate, Convert, Rename, and Safe Delete.
- **Center Workspace Architecture (Elimination of Right Rail):**
  - The legacy Right Mode Rail was permanently eliminated. The center workspace expands to full width (`flex-1 h-full min-w-0 bg-[#FAF8F3]`) between the Left Sidebar and the window frame, providing an unconstrained canvas for reading, editing, and streaming.

---

## 5. Hybrid Project Model

Sthira uses a **hybrid project model** where projects provide context, not exclusive ownership:
- **Many-to-Many Reuse:** Any artifact (note, source, snippet) lives globally and can be attached to multiple projects simultaneously without duplicating the file or splitting its revision history.
- **Nodes Belong to Artifacts:** Decisions, findings, hypotheses, and questions are anchored to the artifact where they were written. When an artifact is reused across Project A and Project B, its insights remain accessible in both contexts with clear attribution (*"Recorded in [Artifact Title]"*).
- **Preserved Purpose Evolution:** A project's purpose is tracked as an append-only timeline (`project_purposes`). When a user pivots a project's goal, the original goal is preserved for the History view rather than overwritten.
- **Organic Promotion:** Any loose note (or group of notes) can be promoted into a project (`origin_artifact_id`), automatically seeding the project's initial goal and history from that starting note.
- **Deletion Safety Hierarchy (Safe Detach vs. Cascade Delete):**
  - **Safe Default:** Deleting a project removes the container and detaches associations, but preserves all exclusive notes as unassociated loose pages in the vault. Zero accidental data loss.
  - **Cascade Delete:** Completely purges exclusive notes if explicitly chosen in the inspection modal, while preserving shared multi-project notes.
  - **Artifact Deletion:** If a note is attached to multiple projects, deleting it from one project detaches it locally while keeping it active in all other projects.

---

## 6. Creation Experience & Universal Input

Users can start anywhere (spontaneous thought, URL paste, quick question) or start with explicit intent.

### 6.1 Frictionless Capture by Default
Typing into the Universal Input and submitting without selecting an explicit mode triggers frictionless capture:
- Immediately saves a durable user-authored note (`authorship = 'user'`).
- Triggers background epistemic extraction.
- Checks active project goals and links the note automatically if strong alignment is detected.
- Never interrupts the user with modal dialogs or configuration screens.

### 6.2 Mode Steering (`Capture`, `Research`, `Plan`, `Act`)
- **`Capture`:** Rapid thought entry with zero UI friction.
- **`Research`:** Systematic investigation formulating search keywords, scraping source documents, verifying quotes, and presenting findings in an Evidence Matrix without mutating project state.
- **`Plan`:** Hierarchical milestone planning breaking tasks into phases, prerequisites, and open questions, rendering as an interactive Milestone DAG.
- **`Act`:** Execution drafts and concrete deliverables wrapped in reversible change sets.

### 6.3 Universal Input Fast-Path Routing
1. **Transient Questions (`"What is speculative decoding?"`):** Answered concisely with zero database clutter. The user is offered an optional action to save the answer (which records it with `authorship = 'ai_generated'`).
2. **Spontaneous Thought Capture (`"Save this thought: smaller models may be easier to specialize"`):** Immediately saved as a durable user-authored note (`authorship = 'user'`).
3. **External Source Capture (URLs or pasted articles):** Saved as an external source (`authorship = 'external'`), preventing external claims or prompt injections inside clipped web text from being treated as user decisions.
4. **Research or Project Initiation (`"Research local coding models under 20B"`):** Generates an inspectable preview plan with `[Create Project]` and `[Save as Note]` buttons.
5. **Project Resumption (`"Continue my webhook project"`):** Matches the existing project and immediately surfaces its Resume Brief without mutating state.
6. **Ambiguous Input (`"Webhooks"`):** Prompts a calm clarification choice (Ask vs. Capture Note vs. Resume/Create Project) rather than guessing and polluting the workspace.

---

## 7. Resume-First Experience & Persistent Working Memory

### 7.1 Level 1: Brief (Default)
A scannable handoff (capped at $\le 75$ visible words) containing:
- **Purpose:** The project's current active purpose.
- **Recent Substantive Progress:** Grounded claims from recent sessions, each tagged with its exact provenance (`user_recorded`, `source_backed`, or `ai_inferred`) and verified quote/node evidence.
- **Unresolved Question / Continuation Point:** Deterministically ranked by recent substantive work (never fooled by simply opening an old file or fixing a typo). Also surfaces **unfinished drafts** if the user previously stopped mid-sentence.
- **Direct Continuation Links:** Exact artifacts to open to resume work immediately.

### 7.2 Level 2: Expanded Review (On Demand)
A structured navigation aid into the project's preserved knowledge base:
- **Established Findings** (`source_backed` & `user_recorded` with verbatim quotes and links).
- **Recorded Decisions & Reasoning** (including unresolved conflicts if a new decision contradicts an older one).
- **Rejected Approaches** and the recorded reasons why they were ruled out.
- **Open Questions** ranked by relevance to recent work.

### 7.3 Persistent Working Memory & Deterministic Cache Invalidation
To ensure returning to work feels instantaneous ($<5\text{ms}$ on cold return):
- Pre-computed **Level 1 Briefs**, **Level 2 Reviews**, and **Global Working Context** are stored in `derived_briefs`, keyed by a cryptographic **`basis_fingerprint`** (computed from the sorted hashes of all attached artifacts, epistemic nodes, and active project purposes).
- **Zero Stale Flag Bugs:** Sthira never relies on a manual `is_stale` boolean. Whenever a note is edited (in-app or on disk), a decision is updated, or a proposal is accepted, the `basis_fingerprint` changes automatically, invalidating the cached brief by construction.
- **Isolated Read-Only Memory Mirrors:** Sthira exports Markdown snapshots to `.sthira_meta/memory_mirrors/` (outside the watched `artifacts/` vault and stamped with `<!-- STHIRA_DERIVED_MEMORY: DO_NOT_INGEST -->`) so external tools or models can inspect current project memory with zero risk of Sthira re-ingesting its own AI summary as a user note.

---

## 8. Resolved Architecture & Experience Decisions

### 8.1 History Presentation
**Both by default.** A concise, segmented **Evolution Narrative** sits at the top (explicitly separating `[Recorded]` segments from `[AI Interpretation]` segments), backed by a chronological **Milestone Timeline** below.

### 8.2 Materiality Filtering (Signal vs. Noise)
Sthira computes edit materiality semantically, not just by character count. Changing `"16GB"` to `"8GB"` or `"will"` to `"will not"` is recognized as a **substantive** change, whereas whitespace reformatting or simple typo fixes are classified as **minor** and filtered out of milestones and resume triggers.

### 8.3 Proactive Organization & Suggestion Budget
Pull-based and strictly budgeted ($\le 2$ ambient proposals per session, only after typing settles). Sthira never auto-links items silently; accepted links create a reversible Change Set, and dismissed proposals are permanently suppressed unless explicitly un-dismissed in the proposal inspector.

### 8.4 Contextual Resume Pane Trigger
Driven by a deterministic core hint (`resume_pane_hint`): auto-expands on a **Cold Return** ($\ge 48$ hours since last substantive work) or when an unfinished mid-sentence draft or conflicting decision needs attention; stays collapsed during active back-to-back sessions.

### 8.5 Communicating Uncertainty & Sparse Evidence
When a project has only saved sources and zero user notes, Sthira never fabricates a research trajectory and never uses guilt framing: *"{N} saved sources about {Topic}. No notes, decisions, or open questions recorded yet. Reviewing the sources is a good place to start."*

### 8.6 Extensible Typed Object System (Anytype-Style Entities)
Enables structured knowledge entities alongside freeform Markdown:
- **Core Types:** `Idea` (💡 spark, exploring, validated), `To-Do / Action` (☑️ due date, priority, completion), `Decision` (⚖️ rationale, superseded_by, linked to epistemic nodes), `Source / Reading` (🔗 URL, author, reading status).
- **Custom Schema Builder:** Users can create custom object types with typed fields (`text`, `number`, `select`, `date`, `relation`, `checkbox`).
- **Multi-View Collections:** Render collections as sortable/filterable **Table / List Views** or drag-and-drop **Kanban Boards**.

### 8.7 Pluggable Privacy-First Web Research
- **Modular Search Trait:** The Rust core defines `SearchProvider`, supporting SearXNG (local/private), Brave Search (cloud API), Tavily (AI search), and DuckDuckGo (headless HTML scraper).
- **Scraper & Cache:** Scrapes pages into clean Markdown, strips HTML noise, and caches results in `web_research_cache`.
- **Grounded Epistemic Ingestion:** External claims must pass `VerifiedQuote::verify()` against scraped text with `Authorship::External`.
- **1-Click Vault Import:** Web citations can be converted into durable, offline Markdown source artifacts linked to projects.

### 8.8 Adaptive Streaming & Conditional Generative UI
- **Dual-Mode Presentation:** The UI dynamically determines whether to render streaming LLM responses as raw text or structured UI widgets:
  - **Standard Markdown Stream:** Used for general questions and lightweight models.
  - **Generative UI Widgets:** Progressively renders `<MilestonePlanDagWidget>`, `<EvidenceComparisonMatrixWidget>`, and `<InteractiveDecisionSelectorWidget>`.
- **Fail-Safe Parser Invariant:** If an LLM emits broken JSON or unclosed widget blocks, the parser gracefully emits a `FallbackNotice` and degrades to clean streaming Markdown without crashing or throwing errors.
- **Top-Right Mode Toggle:** Users can switch at any time between `[ 🎛️ Interactive Widgets ]` and `[ 📄 Plain Markdown ]`.
- **Zero-Stub Production Invariant:** The core runtime and UI strictly invoke the active selected model provider (`OllamaProvider`, `OpenAiCompatibleProvider`) for real inference streaming. Hardcoded static template stubs are permanently eliminated from live runtime paths.

### 8.9 Full-Height Canvas Workspace & Collapsed Epistemic Margin (`FocusView`)
- **Full-Viewport Canvas:** Focus View expands to the full viewport height (`flex-1 h-full overflow-y-auto bg-[#FAF8F3]`) with a generous responsive document sheet (`max-w-4xl lg:max-w-5xl mx-auto px-12 py-10 min-h-full bg-white border-x border-[#E8E5DC]`), eliminating cramped "postage-stamp" floating cards.
- **Collapsed Epistemic Margin by Default:** Extracted claims, verified quotes, and epistemic badges are docked in a dedicated right sidebar (`w-80 border-l border-[#E8E5DC] bg-[#FAF9F6]`). The margin is collapsed by default (`showMargin: false`) to give users maximum writing canvas, and smoothly opens on demand with the `[ 📖 Margin ]` toolbar button.

### 8.10 Notion-Style Slash Command Menu (`/`) & Component-Decorated Rich Workspace
- **Caret-Positioned `/` Menu:** Typing `/` at the beginning of a line or after whitespace opens an interactive popover with keyboard navigation (Arrow Up/Down, Enter, Escape):
  - **Formatting Blocks:** `/h1`, `/h2`, `/h3`, `/todo` (`- [ ] `), `/bullet`, `/table`, `/code`, `/callout`.
  - **Sthira Epistemic & Object Blocks:** `/decision` (registers `UserRecorded` decision node), `/question`, `/idea`, `/task`, plus all user-registered custom types.
  - **Inline AI Assistant:** `/plan <goal>`, `/research <topic>`, `/expand` stream real model completions directly into the document at the cursor position.
- **Formatting Toolbar:** Quick-access toolbar at the top of the canvas provides one-click formatting (`B`, `I`, `H1`, `H2`, `Checklist`, `Code`, `Table`, `/ Insert Block`).
- **Binary Mode Toggle (`Cmd+E`):** Seamlessly switches between raw Markdown editing and `RichDocumentRenderer` where tables are searchable/copyable, checklists are interactively clickable and auto-save back to the document, decision callouts render with Sage badges, and Mermaid diagrams execute dynamically.

### 8.11 Unified "Explore & Plan" Hub (`ExplorePlanView`) & Center-Workspace Streaming
- **Permanent Elimination of the Right Mode Rail:** All research, planning, and synthesis stream directly inside the center document workspace (`FocusView`), completely replacing cramped side drawers.
- **4-Tab Master Hub:**
  1. **`[ 🔍 Research ]`:** Full-width inquiry card with Web Search mode selector (`Auto` / `Always Search Web` / `Local Vault Only`), model selector, and project affiliation.
  2. **`[ 📋 Plan ]`:** Full-width planning card with milestone generation and project creation.
  3. **`[ ⏱️ Trackers ]`:** Management of active recurring research and plan trackers with pause/resume, run-now, and deletion controls.
  4. **`[ 📚 All Digests ]`:** Searchable directory of all past generated Research digests and Plan reviews with type filters and project chips.
- **Smart LLM Title Generation (`generate_smart_title`):** Fast, bounded prompt against the active model generates concise 3-to-6 word titles in Title Case, preventing conversational prompts from becoming messy artifact titles.
- **Grounded Research Fallback Guarantee:** When web search fails or yields 0 results, the system marks `search_status = "web_search_failed"`, renders an amber warning badge (`⚠️ Web search returned no results; synthesized using local context`), and falls back to strictly relevant local vault documents.

### 8.12 Continuous & Scheduled Topic Trackers (Wall-Clock Scheduling & Continuity Boundaries)
- **Wall-Clock Cadence Engine:** Topics can be scheduled for exact times of day (e.g. `06:00 UTC`) and specific weekdays for weekly topics, evaluated in the background every 30 minutes without requiring open modal drawers.
- **Anti-Duplication Continuity Boundary:** Research topics extract key findings from the last 5 digest runs and inject them into the search planner as an explicit `CONTINUITY BOUNDARY`. This prevents repetitive web queries and ensures each scheduled digest covers genuine new developments.
- **Preserved Intelligence on Deletion:** Deleting a scheduled tracker purges run records from the scheduler but **strictly preserves all generated digest artifacts** in the user's vault.

### 8.13 Object Type & Template Management System (Notion/Anytype-Style Schemas)
- **Extensible Schema Engine:** Users can create custom object types with typed properties (`Text`, `Number`, `Checkbox`, `Select`, `Multi-Select`, `Date`, `URL`) and customizable option palettes.
- **Pre-Filled Markdown Templates:** Each object type supports default and named reusable Markdown templates with pre-configured property values. Built-in types ship seeded templates (`idea`, `todo`, `decision`, `source`).
- **Document Property Header:** Typed objects open with a Notion-style property header in `FocusView` displaying interactive chips that auto-save property edits directly to SQLite.
- **Slash Menu Integration:** Typing `/` surfaces a Template Selector popover (`[Default]`, `[Blank]`, or named presets) before instantiating the object.

### 8.14 Page Action Menu (`...`) & Safe Multi-Project Lifecycle Management
- **Unified Action Surface:** Consistent `•••` action menu mounted across FocusView toolbar, Left Sidebar note rows, and All Files table rows.
- **Single-Transaction Relocation:** Moving a note between projects runs as a single SQLite transaction that maintains append-only audit events (`artifact_attached`, `artifact_detached`), guaranteeing epistemic history and resume briefs remain intact.
- **Byte-Preserving Object Conversion:** Converting a note to an object type writes an `object_instances` row while keeping the underlying Markdown body byte-identical.
- **Safe Detach vs. Deletion:** Detaching a note returns it to the standalone pool without deleting data; deleting a note explains real semantics based on whether the note is shared across projects.

### 8.15 Explicit Web Search Capability Detection & Badging
- **Four-Tier Classification:** Every model in Sthira's registry is classified under `WebSearchCapability`:
  - `Native`: Built-in web grounding (e.g. Perplexity Sonar).
  - `AgenticToolCall`: Function calling support for SearXNG / Brave tools (e.g. Claude 3.7, GPT-OSS 120b, Gemma 4 31b).
  - `EngineAugmentedOnly`: Sthira's Rust backend scrapes search results and injects clean Markdown chunks into the prompt (e.g. local Ollama Llama 3.2, Phi-3).
  - `Unsupported`: Offline / embedding models where search is forbidden.
- **Visual Badging & Adaptive Prompts:** Model selectors display color-coded badges (`[🌐 Native Search]`, `[🌐 Web Search Ready]`, `[📄 Web Context Injected]`, `[🚫 Offline Only]`). Search toggles automatically adapt or disable with clear explanatory tooltips.

### 8.16 Targeted Re-Research & Note Derivation (Chained Synthesis & Epistemic Lineage)
- **Database Lineage Architecture (PRAGMA user_version = 10):**
  - Artifacts track provenance via `derived_from_artifact_id REFERENCES artifacts(id) ON DELETE SET NULL` and `derivation_type` (`'refine'`, `'plan_from_research'`, `'extract_ideas'`, `'extract_tasks'`), indexed via `idx_artifacts_lineage`.
- **Targeted Re-Research (`refine_artifact`):**
  - Allows iterative critique of an existing note with custom instructions, model switching, and optional live web search.
  - Automatically snapshots the previous state as a reversible `ChangeSet` (`restore_artifact`), enabling instant **1-click undo**.
- **Note Derivation (`derive_artifact`):**
  - Generates new structured artifacts from an existing note (e.g. converting a Research note into an Execution Plan or discrete Ideas).
  - Automatically links the child note to the source note via `derived_from_artifact_id` and inherits project associations.
- **Lineage Header Chip:** Derived notes render a prominent header badge: `🔗 Derived from: [Parent Artifact Title]`. Clicking the badge navigates immediately to the parent document, making research-to-plan trajectories transparent and traceable.

### 8.17 Selectable Synthesis Templates & Integrated Workflow Engine
- **Pre-Seeded Synthesis Templates (`synthesis_templates`):**
  - Database-backed templates for `research` and `plan` modes, enabling steered markdown structure.
  - **Research Defaults:** "Comparative Tech Evaluation" (Executive Summary, Contenders, Comparison Matrix table, Grounded Sources, Recommendation) and "Deep Dive Investigation".
  - **Plan Defaults:** "Milestone Roadmap" (Scope & Objectives, Phase 1-3 tasks `- [ ]`, Open Decisions) and "Fast Iteration Sprint".
- **Prompt Skeleton Steering:**
  - Sthira injects strict formatting instructions into synthesis prompts:
    `"Synthesize strictly into this markdown skeleton preserving all headers:\n{template_skeleton}"`
  - Guarantees predictable, rich, and high-fidelity artifact structuring across local and cloud LLM providers.
- **4-Tab Explore & Plan Hub:**
  - Unified hub for ideation, scoping, tracking, and review (`[ 🔍 Research ]`, `[ 📋 Plan ]`, `[ ⏱️ Trackers ]`, `[ 📚 All Digests ]`).
  - Native model selector with search capability chips and interactive synthesis template selector.