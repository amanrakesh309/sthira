# Sthira (स्थिर)

> **AI-Native, Local-First Knowledge Workspace & Research Engine**  
> *Deterministic epistemic safety, zero-guilt resumption, bi-directional Markdown-SQLite synchronization, and adaptive generative UI.*

Sthira is an offline-first knowledge workspace engineered for deep thinking, long-horizon research, and planning. It fuses the tactile durability of human-readable Markdown files with the relational rigor, vector search, and transactional integrity of an embedded SQLite database (`rusqlite` + `sqlite-vec`), wrapped in a desktop application built with Tauri v2, React 19, and Tailwind CSS.

---

## Key Architectural Principles

1. **Dual-Store Synchronization (Disk + SQLite):**
   - Human-readable notes live in `.sthira_vault/artifacts/` as Markdown files with YAML frontmatter.
   - High-performance relational state, FTS5 full-text indexes, and vector embeddings (`sqlite-vec`) live in `sthira.db`.
   - Bidirectional reconciliation automatically detects edits, new files, renames, and external changes with conflict resolution.

2. **Compile-Time Epistemic & Authorship Integrity:**
   - Authorship (`User`, `External`, `AiGenerated`) is strictly preserved at ingestion.
   - Epistemic provenance (`UserRecorded`, `SourceBacked`, `AiInferred`) is computed deterministically from `(artifact.authorship, VerifiedQuote::verify())`.
   - AI outputs can **never** be laundered into `UserRecorded` decisions. Speculative thoughts (*"Maybe we should..."*) are treated as hypotheses, never recorded decisions.

3. **Sub-5ms Cold Return & Zero-Guilt Resumption:**
   - Level 1 Quick Briefs ($\le 75$ words) and Level 2 Reviews are precomputed and cached in `derived_briefs` by cryptographic `basis_fingerprint`. Cold returns execute in $< 5\text{ms}$.
   - **No-Pressure Guarantee:** Copywriting is strictly neutral and informative. Guilt-inducing language (*"you haven't"*, *"overdue"*, *"behind schedule"*, or *"streak"*) is programmatically banned and enforced via automated linter gates.

4. **Isolated Derived Memory Mirrors:**
   - Human- and LLM-readable Markdown mirrors are exported to `.sthira_meta/memory_mirrors/` with the header `<!-- STHIRA_DERIVED_MEMORY: DO_NOT_INGEST -->`.
   - Vault reconciliation strictly ignores `.sthira_meta/` to ensure AI summaries are never re-ingested into user notes.

5. **Provider-Centric OmniRouter Compute Aggregator:**
   - Connect any local or cloud LLM provider (Ollama, OpenAI, Anthropic, Groq, OpenRouter, DeepSeek, or custom OpenAI-compatible endpoints) without hardcoded models.
   - Dynamically discovers and syncs live models from provider endpoints (`/v1/models` and `/api/tags`).
   - Inline model switcher chip in the Universal Input card and Settings modal.

6. **Pluggable Privacy-First Web Research:**
   - Modular `SearchProvider` trait supporting SearXNG (local/private), Brave Search, Tavily AI, and a headless DuckDuckGo HTML scraper.
   - Scrapes web pages into clean Markdown, caches results in `web_research_cache`, verifies quotes against scraped text, and offers 1-click `[Import to Vault]` as durable source artifacts.

7. **Adaptive Streaming & Conditional Generative UI:**
   - Rust progressive stream engine (`ProgressiveStreamChunk`) streaming over Tauri v2 IPC channels.
   - Fail-Safe Parser Invariant: broken JSON gracefully degrades to raw Markdown tokens without crashing.
   - Generative UI Widgets: `<MilestonePlanDagWidget>`, `<EvidenceComparisonMatrixWidget>`, `<InteractiveDecisionSelectorWidget>`, and a dual-mode toggle `[🎛️ Interactive Widgets]` ⟷ `[📄 Plain Markdown]`.

8. **Extensible Typed Object System:**
   - Structured entities (`Idea` 💡, `To-Do / Action` ☑️, `Decision` ⚖️, `Source / Reading` 🔗) alongside freeform notes.
   - Custom object type schema builder with support for property types (`text`, `number`, `select`, `date`, `relation`, `checkbox`).
   - Multi-view collections: sortable **Table / List Views** and drag-and-drop **Kanban Boards**.

9. **Interactive 2D Knowledge Graph:**
   - Force-directed physics graph visualizing Projects (hub nodes), Pages/Notes (Warm Stone cards), and Typed Objects.
   - Visualizes structural containment (`belongs_to`) and semantic connections (`supports`, `contradicts`, `extends`, `references`).
   - Interactive pan, zoom, search filtering, and node details inspection cards.

---

## Repository Architecture

```
.
├── Cargo.toml                         # Workspace manifest (resolver = "2")
├── crates/
│   ├── sthira-core/                   # Pure Rust engine library (SQLite DDL, Epistemic, Router, Search, Stream)
│   ├── sthira-desktop/                # Tauri v2 desktop application & IPC command handlers
│   │   ├── src/                       # Rust desktop backend (commands.rs, events.rs, lib.rs)
│   │   ├── ui/                        # React 19 + TypeScript + Tailwind frontend
│   │   │   ├── src/components/        # UI widgets, modals, layout components
│   │   │   ├── src/views/             # HomeView, FocusView, CanvasView, GraphView, ObjectListView, ObjectKanbanView
│   │   │   └── src/test/              # Vitest test suites (33 UI tests)
│   │   └── tests/                     # Rust integration tests for IPC bridge, graph, and actions
│   ├── sthira-cli/                    # Headless CLI binary interface
│   └── sthira-testkit/                # Deterministic test harness (FakeClock, DeterministicFakeModel)
├── xtask/                             # Automated phase gate runner & anti-cheating scanner
├── tests/
│   └── frozen/                        # Acceptance & metamorphic tests (Phases 1 to 7, Phase 9)
├── docs/                             # Architecture specs, implementation plans & progress logs
├── fixtures/golden/                   # Frozen test fixtures and forbidden literals
└── gate_reports/                      # JSON verification reports for all phases
```

---

## Prerequisites

- **Rust:** Stable toolchain (1.80+ or latest stable).
- **Node.js:** v18+ or v20+ with `npm`.
- **SQLite 3:** Included automatically via bundled `rusqlite` feature.
- **Webkit2GTK / Tauri prerequisites (Linux):** Standard desktop development libraries (`libwebkit2gtk-4.1-dev`, `build-essential`, `curl`, `wget`, `file`, `libssl-dev`, `libgtk-3-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`).

Verify your installation:
```bash
rustc --version
cargo --version
node --version
npm --version
```

---

## Building the Workspace

### 1. Build Rust Workspace
Compile all Rust crates (engine, desktop app, CLI, testkit, and xtask):
```bash
cargo build --workspace
```

For release builds:
```bash
cargo build --workspace --release
```

### 2. Build Frontend UI
Compile and bundle the React 19 desktop interface:
```bash
cd crates/sthira-desktop/ui
npm install
npm run build
cd ../../..
```

---

## Running the Desktop Application

### Option A: Tauri Development Mode (Hot Reload)
Runs both the Vite dev server and the Tauri v2 native desktop window with live hot reloading:
```bash
cd crates/sthira-desktop
npm run tauri dev --prefix ui # or: cargo tauri dev (if tauri-cli is installed)
```

### Option B: Run Compiled Desktop Binary
Once the frontend is built (`npm run build` in `crates/sthira-desktop/ui`), you can run the native desktop executable:
```bash
cargo run -p sthira-desktop
```

### Desktop UI Key Bindings & Controls
- **Universal Input:** On Home View, capture thoughts, type search queries, or select operational modes (`Capture`, `Research`, `Plan`, `Act`). Streams real LLM inference from active models without stubs.
- **Full-Height Workspace Canvas (`FocusView`):** Generous responsive document canvas (`max-w-4xl lg:max-w-5xl mx-auto px-12 py-10 min-h-full`) with a cleanly docked, collapsible Epistemic Margin right sidebar (`w-80`).
- **Notion-Style Slash Commands (`/`):** Type `/` at the beginning of a line or after a space to trigger an interactive block menu:
  - Formatting: `/h1`, `/h2`, `/h3`, `/todo`, `/bullet`, `/table`, `/code`, `/callout`
  - Epistemic & Objects: `/decision` (registers `UserRecorded` decision node), `/question`, `/idea`, `/task`
  - Inline AI: `/plan <goal>`, `/research <topic>`, `/expand`
- **Formatting Toolbar & Rich Preview Toggle:** Top formatting toolbar (`B`, `I`, `H1`, `H2`, `Checklist`, `Code`, `Table`, `/ Insert Block`) and seamless toggle between raw markdown editing and rich typography preview with interactive clickable checkboxes (`- [ ] ` $\rightarrow$ `- [x] `).
- **Sidebar Navigation:** Click `+ New Page` under any project or unassociated section. Drag to resize sidebar width.
- **3-Dots Settings Menu (`...`):** Pin notes/projects to top (`📌`), duplicate with `(Copy)` suffix, attach to other projects, or rename inline (`Enter` to save, `Escape` to cancel).
- **Auto-Save:** Typing is automatically saved after 600ms of inactivity or upon clicking away. Watch the live status pill (`Saving...` $\rightarrow$ `Saved` $\rightarrow$ `Auto-saved`).
- **Knowledge Graph:** Click `🌐 Knowledge Graph` in the sidebar to open the 2D force-directed physics graph. Pan, zoom, and filter by node types.
- **Web Search & Research Rail:** In Research mode or in the Right Mode Rail, toggle `[🌐 Include Web Search]` to scrape web sources and click `[Import to Vault]` to persist findings offline.
- **Adaptive Stream Switcher:** During progressive AI responses, toggle between `[ 🎛️ Interactive Widgets ]` and `[ 📄 Plain Markdown ]`.

---

## Running the Headless CLI

Sthira includes a headless CLI (`sthira-cli`) for terminal usage, scripting, and offline automation:

```bash
# 1. View Home State (threads, loose captures, pending proposals)
cargo run -p sthira-cli -- home

# 2. Capture a spontaneous thought
cargo run -p sthira-cli -- capture "Evaluate small quantized models for local test generation."

# 3. Route queries with zero mutations
cargo run -p sthira-cli -- route "What is speculative decoding?"
cargo run -p sthira-cli -- route "Resume local coding research"

# 4. Create and list project threads
cargo run -p sthira-cli -- project new "Local Coding Model Research" "Evaluate local models under 16GB memory"
cargo run -p sthira-cli -- project list

# 5. Resume a project (Level 1 Quick Brief or Level 2 Detailed Review)
cargo run -p sthira-cli -- resume <PROJECT_ID> 1
cargo run -p sthira-cli -- resume <PROJECT_ID> 2

# 6. View chronological project evolution and milestones
cargo run -p sthira-cli -- history <PROJECT_ID>

# 7. Reconcile vault filesystem with SQLite database
cargo run -p sthira-cli -- reconcile
```

---

## Comprehensive Testing & Verification Suite

Sthira is protected by comprehensive test suites across both Rust and React/TypeScript.

### 1. Run All Rust Workspace Tests (68 Tests)
Runs all unit, integration, and frozen acceptance tests across all crates:
```bash
cargo test --workspace
```

### 2. Run Desktop UI Test Suites (29 Vitest Tests)
Runs all frontend component and integration tests in `crates/sthira-desktop/ui`:
```bash
cd crates/sthira-desktop/ui
npm test
cd ../../..
```

### 3. Run Quality Gates via `xtask`
Run individual phase gates (Phases 1 through 7) verifying anti-cheating rules, forbidden literals, and banned wall-clock calls:
```bash
# Phase 1: Hybrid Storage & Vault Reconciliation
cargo run --package xtask -- gate 1

# Phase 2: Epistemic Extraction & Authorship Safety
cargo run --package xtask -- gate 2

# Phase 3: Read-Only Universal Input Router
cargo run --package xtask -- gate 3

# Phase 4: Fingerprint-Cached Resumption & Home State
cargo run --package xtask -- gate 4

# Phase 5: Project History & Segmented Evolution
cargo run --package xtask -- gate 5

# Phase 6: Cross-Project Discovery & Budgeted Proposals
cargo run --package xtask -- gate 6

# Phase 7: 14-Day Full Multi-Project Lifecycle Simulation
cargo run --package xtask -- gate 7
```

### 4. Code Quality & Formatting
Run Clippy with compiler warnings treated as errors:
```bash
cargo clippy --workspace -- -D warnings
```

Format all codebase files:
```bash
cargo fmt --all
```
