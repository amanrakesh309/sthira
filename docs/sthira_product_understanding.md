# Sthira — Product Understanding (v3)

**Status:** Canonical product understanding, experience specification & architecture manual  
**Last updated:** 29 October 2026

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
- Interact with structured objects (Ideas, Tasks, Decisions, Sources) in Tables and Kanban boards.
- Explore cross-project connections through an interactive 2D physics Knowledge Graph.
- Conduct privacy-first web research grounded in verified external quotes.
- Experience adaptive progressive streaming that shifts conditionally between raw Markdown and interactive generative UI widgets.

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
Home separates deep ongoing threads from quick captures:
- **Active Threads ("Pick Up"):** Ordered by **last substantive work** (ignoring trivial file views or whitespace edits), surfacing each project's continuation point or primary unresolved question.
- **Loose Captures:** Recent unfiled notes, thoughts, and saved sources that haven't been assigned to a project yet.
- **Universal Input Card:** A single, central entry point for capturing spontaneous thoughts, asking transient questions, initiating research, planning, or resuming projects.
- **Inline Compute Provider Chip:** Select active local or cloud models (`[ 🦙 Ollama: llama3.2 ▾ ]`, `[ 🚀 Groq: llama-3.3-70b ▾ ]`, `[ ⚡ Anthropic: claude-3-7-sonnet ▾ ]`) with 1-click settings shortcuts.
- **Calm Proposal Indicator:** A quiet, non-intrusive count of pending AI suggestions—never noisy popups or red warning badges.

### 4.2 Canvas — Think and organize spatially
A flexible spatial canvas lets users arrange notes, sources, text cards, and frames freely.
- **Strict Spatial/Semantic Isolation:** Placing two items next to each other on a canvas **never** automatically creates a semantic knowledge link or biases the AI discovery engine.
- **Reversible Layout Revisions:** If the user asks Sthira to help organize or cluster a canvas, the previous layout is snapshotted in a reversible Change Set (`cmd_revert_change_set`).

### 4.3 Focus — Go deep
A distraction-free environment for reading, writing, and synthesizing within a document or project:
- **Debounced Auto-Save (600ms):** Automatically commits edits to both SQLite and disk on typing settle, input blur, and view navigation.
- **Live Status Pill:** Shows `Saving...` with spinner, transitions to `Saved` with an emerald checkmark, and settles into `Auto-saved`.
- **Real-Time Sidebar Title Sync (0ms):** Typing a heading or title synchronizes with the navigation sidebar instantly on every keystroke.
- **Automatic Line 1 Heading Derivation:** If an artifact title is empty, line 1 (`# Heading`) is extracted automatically to keep the workspace organized.
- **Contextual Resume Pane:** Auto-expands on cold return ($\ge 48$ hours) or unresolved decision conflicts; collapses into an ambient pill during active sessions.
- **Epistemic Margin:** Extracted claims and quotes highlighted alongside the text with hover provenance tooltips.
- **Ambient Proposals:** Up to 2 un-intrusive contextual proposals with 1-click `[Accept]` and `[Dismiss]`, backed by an `UndoToast`.

### 4.4 Knowledge Graph — Explore connected structure
An interactive 2D force-directed physics graph view (`GraphView`):
- **Node Classification:** Projects (prominent hub nodes), Pages/Notes (Warm Stone cards with draft badges), and Typed Objects (Ideas, Tasks, Decisions, Sources).
- **Edge Relationships:** Structural containment (`belongs_to`) and semantic connections (`supports`, `contradicts`, `extends`, `references`).
- **Interactive Controls:** Smooth pan, mouse-wheel zoom, drag-to-pin simulation, entity filter pills, real-time search query isolation, and floating details cards with direct jump-to-editor actions.

### 4.5 Workspace Navigation Shell (Sidebar & Rail)
- **Left Hierarchy Sidebar:** Collapsible navigation tree with customizable width (persisted to `localStorage`). Displays:
  - **Projects Tree:** Expandable project folders with child pages, canvases, and `+ New Page` actions.
  - **Unassociated Pages:** Loose captures created without project assignment.
  - **3-Dots Settings Menu (`...`):** Pin to top (`📌`), Duplicate (`(Copy)`), Add to Project modal, Inline rename (`Enter`/`Escape`), and Safe Deletion.
- **Right Operational Mode Rail:** Collapsible workspace rail hosting dedicated consoles:
  - `CapturePanel`: Fast capture with background epistemic extraction.
  - `ResearchPanel`: Search exploration, web source inclusion toggle (`[🌐 Include Web Search]`), citation cards, and 1-click `[Import to Vault]`.
  - `PlanPanel`: Hierarchical milestone decomposition with interactive DAG widgets.
  - `ActPanel`: Synthesis console, deliverable drafts, and reversible change sets.

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

### 8.9 Full-Height Canvas Workspace & Docked Epistemic Margin (`FocusView`)
- **Full-Viewport Canvas:** Focus View expands to the full viewport height (`flex-1 h-full overflow-y-auto bg-[#FAF8F3]`) with a generous responsive document sheet (`max-w-4xl lg:max-w-5xl mx-auto px-12 py-10 min-h-full bg-white border-x border-[#E8E5DC]`), completely eliminating cramped "postage-stamp" floating cards.
- **Docked Collapsible Epistemic Margin:** Claims, verified quotes, and epistemic badges are cleanly docked in a dedicated right sidebar (`w-80 border-l border-[#E8E5DC] bg-[#FAF9F6]`), toggleable with a single click (`btn-toggle-epistemic-margin`).

### 8.10 Notion-Style Slash Command Menu (`/`) & Rich Visual Editing
- **Caret-Positioned `/` Menu:** Typing `/` at the beginning of a line or after whitespace opens an interactive popover with keyboard navigation (Arrow Up/Down, Enter, Escape):
  - **Formatting Blocks:** `/h1`, `/h2`, `/h3`, `/todo` (`- [ ] `), `/bullet`, `/table`, `/code`, `/callout`.
  - **Sthira Epistemic & Object Blocks:** `/decision` (registers `UserRecorded` decision node), `/question`, `/idea`, `/task`.
  - **Inline AI Assistant:** `/plan <goal>`, `/research <topic>`, `/expand` stream real model completions directly into the document at the cursor position.
- **Formatting Toolbar:** Quick-access toolbar at the top of the canvas provides one-click formatting (`B`, `I`, `H1`, `H2`, `Checklist`, `Code`, `Table`, `/ Insert Block`).
- **Live Edit / Rich Preview Toggle:** Seamlessly switches between raw markdown editing and a rich typography preview where headings render with proper typography, tables are styled, and checklists (`- [ ] `) are interactively clickable and auto-save back to the document.