# Antigravity Master Prompt — Sthira UI/UX Redesign

## Your role

You are the principal product designer, senior React/TypeScript engineer, and accessibility reviewer working on **Sthira**, an existing **AI-native, local-first knowledge workspace**. Your assignment is to transform the existing functional UI into a polished, mature, calm, coherent desktop application **without rewriting the product or breaking behavior**.

**Product character:** Steady, trustworthy, warm, readable, minimal, capable, exploratory. Sthira is a serious tool for thinking, researching, organizing knowledge, and resuming work after interruption—not a chat-first dashboard or a decorative landing page.

## Reference files: READ BEFORE CODING

Find the following files in the current repository/workspace (the accompanying README describes suggested placement):

1. `docs/design/sthira-design-system-v1.md` — **visual source of truth**: semantic colors, typography, components, spacing, per-screen behavior, acceptance criteria.
2. `/home/aman/Projects/SthiraNotes/crates/sthira-desktop/ui/src/styles/sthira-theme-tokens.css` — **implementation-ready design tokens**. Preserve and extend these tokens instead of scattering hard-coded colors.
3. `docs/design/references/home.png` — target visual direction for Home.
4. `docs/design/references/document-workspace.png` — target for Focus/document workspace.
5. `docs/design/references/all-files.png` — target for All Files.
6. `docs/design/references/knowledge-graph.png` — target for Knowledge Graph.
7. `docs/design/references/settings.png` — target for provider/settings UI.
8. `sthira_product_understanding.md` or the project's actual canonical product-understanding file — **functional/product behavior authority**.
9. `plan(1).md` or the repository's actual active implementation plan — architecture and implementation background, NOT permission to rebuild.
10. Repository instructions (`AGENTS.md`, README, package scripts, Tailwind configuration, test configuration, and existing UI conventions).

If any file/path differs, **locate its real path** and document it. Do not invent file paths or assert you inspected files you have not opened. If references are absent, continue using the available design system and actual UI; report the omission rather than hallucinating images.

**Priority on contradictions:** working code and accepted product behavior / product-understanding document > design-system functional constraints > visual reference mockups. A mockup is an aesthetic target, not proof of data, workflow, model capability, graph edges, or research facts.

---

## Absolute non-negotiable constraints

- **Visual and usability refactor, not feature rewrite.** Reuse existing React components, hooks, state, routing, IPC adapters, Tauri commands, Rust engine and vault/database semantics. Do not change Rust code or database schema for visual purposes.
- Preserve Capture / Research / Plan / Act and their exact real behavior; retain provider configuration, web search capability semantics, model selection, research streaming, trackers, history, source citations, epistemic provenance, typed objects, templates, reversible AI changes, slash commands, Markdown editing/preview, graph and canvas interactions.
- No fabricated project cards, document rows, model capabilities, successful connection labels, graph edges, findings, activities, summaries, or citations. Render only real application state; for zero records, create a polished empty state.
- A source-backed statement, a user-recorded decision, and an AI inference must stay visibly and semantically distinct. Do not decorate AI interpretations as established user decisions.
- **Resume first:** a concise grounded Level 1 brief (max ~75 visible words) and optional Level 2 expanded review. Opening a document returns the user to actual work. The right Epistemic Margin remains collapsed by default unless existing cold-return logic opens it as documented; do not make an ever-present AI sidebar.
- Preserve the editor's actual **Markdown Edit vs Rich Preview** modes; do not silently replace the editor with WYSIWYG just because a mockup looks like one.
- Support **offline startup**. Avoid network-loaded fonts, mandatory online images, CDN UI assets, or any UI render dependency on inference providers.
- Do not add dependencies or alter build tooling merely for visual parity if existing Tailwind, React and Lucide can cover it. If one dependency is genuinely necessary, document why and alternatives first.
- Fix incidental UI issues directly connected to the redesign, but do not expand into unrelated backend bugfixes. Record unrelated pre-existing issues in a separate backlog.
- Never delete user data, migrate a live vault, or change credentials/provider values. Do not commit sensitive settings or API keys.
- Do not replace working endpoints with mocks, sample data, or static generated copy.
- Do not arbitrarily replace app shell/window chrome with mockup OS decorations; respect actual Tauri window controls and Linux/Windows behavior.
- Use small, reviewable changes; keep existing tests green and add focused UI tests where useful.

---

## Design target: Warm Stone + Forest Sage

Implement **the token values in `src/styles/sthira-theme-tokens.css`** as canonical, not a separately invented palette:

### Light
- App: `#F5F3EC`
- Canvas: `#FAF8F3`
- Surface: `#FFFFFF`
- Raised surface: `#FDFBF7`
- Subtle surface: `#F1EEE6`
- Text: `#1E201C`
- Muted text: `#62665D`
- Border: `#E8E5DC`
- Primary: `#49583D`; hover `#39472F`
- Primary tint: `#EEF1E8`
- Amber accent: `#BE8B41`

### Dark
- App: `#121310`
- Canvas: `#161714`
- Surface: `#1C1D19`
- Raised: `#23251F`
- Text: `#EDEAE2`
- Muted: `#B1B5AB`
- Border: `#2C2E27`
- Primary action: `#879E78`
- Primary tint: `#283427`

### Typography, rhythm, interaction
- UI: Inter if already available locally; otherwise system sans. Optional editorial headings: locally available Fraunces/Source Serif 4 or Georgia fallback. **Do not pull fonts from the network**, and do not add font files from unknown licensing sources.
- Hero ~40–44px (responsive), page title ~30–34px, section ~22–24px, controls/body ~13–16px. No essential labels under 12px.
- Spacing rhythm 4 / 8 / 12 / 16 / 24 / 32 / 48px.
- Sidebar ~264px default, resizable if existing resizing is supported. Header ~56px. Reading column ~720–820px. Inputs/buttons 40–44px high. Card radius 16px; input radius 8px; popover radius 12px; hero input radius ~20px.
- Prioritize warm neutrals and readable hierarchy. Use green for primary actions/selection and restrained amber editorial accents. Do not apply large illustrations, shadows, and colored chips everywhere.
- Prefer subtle borders to box shadows. Illustrations/landscape textures are **optional and localized** to low-information header regions. Never behind document text, graphs, data, or forms.
- All interactive elements need hover, keyboard-visible focus, disabled, loading, success/error and reduced-motion-aware states. Aim for WCAG AA contrast; do not rely on color alone for provenance and status.

---

## Step 0 — Inspect, document baseline, and protect functionality

Before editing:

1. Inspect the frontend structure, React entry point (`src/main.tsx`, `src/App.tsx`, or equivalent), existing global CSS, Tailwind version/configuration, current theme toggling mechanism, shared components, routing, styles and provider settings UI.
2. Identify each relevant real component and map it to the visual redesign sections: app shell, sidebar, Home, Focus/editor, All Files, typed objects, graph, canvas, Explore & Plan, provider/settings modal.
3. Identify existing npm/pnpm/yarn scripts for lint/typecheck/test/build and Tauri dev commands rather than guessing commands. Run the available baseline checks and capture their results; do not fix unrelated failures silently.
4. Check `git status`; preserve uncommitted user work. Prefer an isolated branch or checkpoint if appropriate, but never discard existing changes.
5. Capture baseline screenshots or describe why screenshot automation is unavailable. At minimum record Home, document/editor, All Files, Graph, Settings under light and dark modes if possible.
6. Make a short baseline report in `docs/design/implementation-progress.md`: actual file paths, proposed edit sequence, existing UI tests, pre-existing issues, and acceptance checks.

Do not over-plan for hours: after recording the baseline, proceed with the implementation. Do not stop to ask for permission between safe, scoped visual changes.

## Step 1 — Integrate tokens correctly

1. Put `sthira-theme-tokens.css` **in the active frontend source tree**, preferably `src/styles/sthira-theme-tokens.css` relative to its package root. If the real frontend is nested, use the equivalent actual path; do not create an unused second frontend directory.
2. Ensure it is imported exactly **once** from the React entry point or global stylesheet, after or in coordination with existing Tailwind/global CSS. Example for a Vite frontend:
   - `import './index.css';`
   - `import './styles/sthira-theme-tokens.css';`
3. Confirm the actual theme selector. The token file supports `html.dark` and `html[data-theme="dark"]`; adapt its selectors to the code's existing mechanism if needed, without breaking Light / Dark / System. System must respond to OS preference according to existing product behavior.
4. Keep Tailwind installed. If Tailwind supports semantic theme definitions in the current version, integrate the `--sthira-*` variables idiomatically. Otherwise use classes such as `bg-[var(--sthira-canvas)]`, `text-[var(--sthira-text)]`, or scoped CSS primitives. Do not rewrite the Tailwind build configuration without need.
5. Use `.sthira-shell` on the actual top-level app shell (or equivalent), and apply `.sthira-canvas` and `.sthira-surface` where appropriate. The CSS import alone does **not** apply the theme to existing components.
6. Migrate repeated hard-coded warm colors incrementally. Avoid duplicative CSS overrides and global resets that destabilize the editor, graph library, rich markdown renderer, or Tauri chrome.
7. Test light, dark, and system theme and check nested popovers, modals, graph controls, and rich content.

## Step 2 — Shared primitives and app shell

Create or refine shared primitives *only after inspecting whether equivalents already exist*: `Button`, `IconButton`, `Input`, `TextArea`, `Select`, `Tabs/SegmentedControl`, `Badge/Status`, `Surface/Card`, `Dialog`, `Popover`, `Tooltip`, `Toolbar`, `TableRow`, `EmptyState`, `Skeleton`, `InlineAlert`.

Follow these rules:
- Maintain existing public props and callbacks wherever possible; use backwards-compatible variants if needed.
- Separate interactive filter chips from read-only type chips and provenance/status labels.
- One active sidebar navigation state derived from the actual route/view; no "Home active" bug while on Graph/Files.
- Avoid two competing global search boxes; distinguish global command palette from page-local search/filter.
- Cap immediate sidebar item lists, preserve View all links and expandable projects, pinning, hover actions, context menus, and resizable/collapsible behavior if present.
- Preserve contextual menus, keyboard navigation, focus trap, Escape, overlay dismissal, and click targets.
- Do not add more than one visually dominant CTA per area.
- Avoid unnecessary animation and use `prefers-reduced-motion`.

At the end of this step, the shell should look coherent even when screens still use older content components.

## Step 3 — Home: the first full-screen vertical slice

Use `references/home.png` as visual inspiration, not an exact copy. Implement:
- Calm top identity and concise title "Make room to think." only where suitable; do not let decorative hero content consume the usable viewport.
- Prominent universal input for Capture/Research/Plan/Act, preserving mode selection, Enter/keyboard behavior, web-search controls, selected provider, error states, and actual routing.
- Show **up to three** existing active projects with real names, real descriptions if recorded, and trustworthy metadata. Do not invent example projects.
- "Pick up where you left off" displays up to three meaningful recent continuation points ordered by the product's substantive-work logic, not raw last-open time when better data exists. Show grounded continuation text when available, otherwise neutral fallbacks.
- Quiet suggestion/proposal affordance only for real pending proposals. No fake "Add 2 sources" suggestion.
- Excellent empty states for fresh installation, no models connected, no projects, no history, offline, and errors; avoid treating "no provider" as the application being unusable.
- Screen works at 1366×768, 1600×900, 1920×1080, narrow/resized sidebar, and 125% scaling.

**First quality gate:** capture screenshots, run relevant tests, review visual parity and behavior, and fix regressions before moving on.

## Step 4 — Focus/editor + contextual resume + history

Use `references/document-workspace.png`. Preserve actual editor behavior and data:
- Strong document title and readable content column; appropriate editor toolbar and rich Preview distinction.
- Level 1 resume brief remains compact (purpose, grounded progress, next continuation point), capped at ~75 visible words. Expanded Level 2 review is on demand and source-linked.
- Do not fabricate grounded findings or next steps when project evidence is sparse.
- Keep right margin collapsed by default except documented cold-return/contradiction behavior. When opened, distinguish recorded facts, external sources, inferred suggestions and actions.
- Keep autosave status truthful and bound to actual save lifecycle (saving/saved/error).
- Preserve the slash menu, Markdown content fidelity, tables, Mermaid, checklists, typed properties, Refine/Derive, rollback, citations, lineage chips, History timeline, Canvas and keyboard shortcuts.
- Opening the project goes to actual work, not a mandatory summary interstitial.

## Step 5 — All Files, projects and typed objects

Use `references/all-files.png`.
- Create clear hierarchy: All Items / Projects / Notes & Pages / Custom Objects, with accurate real counts.
- Existing scope (in-project/standalone), type, search, sorting and action behavior remains intact. Distinguish filter controls from metadata badges.
- Align table columns and row actions; support long titles, long summaries, no project association, missing dates, narrow windows and dense data.
- Preserve safe detach vs delete semantics, multi-project reuse, conversion, templates and types. Never visually imply that editing in one project creates a separate copy.
- Use consistent list/table and card patterns, with meaningful keyboard focus and empty states.

## Step 6 — Knowledge Graph + Canvas

Use `references/knowledge-graph.png`.
- Improve graph node appearance, labels, selected states, legend, search, filtering, zoom, pan, centering and controls without changing the graph's actual edges, semantic relationship rules or physics behavior.
- Preserve distinction between containment/project links and semantic supports/contradicts/extends/references edges.
- No invented edges or decorative relationships! Layout/proximity in Canvas must never create a semantic link.
- Design for performance and readability with much larger graphs, not just the 11-node screenshot.
- Preserve pan/zoom/drag/accessibility interactions and reduce visual over-decoration.

## Step 7 — Settings + Explore & Plan

Use `references/settings.png`.
- Rename **consumer-facing** "OmniRouter Compute Fabric" entrypoints to "Models & Providers" or "AI & Model Settings" if this does not change internal API keys; keep technical engine identity in About/Advanced where appropriate.
- Use clear groups for Models, Web Search, Appearance, Storage & Privacy and Advanced. Preserve current endpoints, credentials, provider selection and test features.
- Show truthful provider states (Connected, Testing, Needs Setup, Unavailable) based on real checks; never label a provider connected solely because a URL field has a value.
- Separate active model from default model and web-search provider availability from a model's tool/search capability. Labels should be understandable to non-engineers with technical detail on demand.
- Avoid multiple nested scrollable panels in the settings dialog; handle narrow windows responsibly.
- Preserve Explore & Plan's Research, Plan, Trackers, Digests, streamed progress, templates, source grounding, failure fallback, and controls.
- Keep errors useful: tell the user what went wrong and what they can do.

## Step 8 — QA, cleanup and handoff

1. Run the actual available project typecheck, lint, unit/UI tests, build, and if feasible Tauri smoke test. Report exact commands and pass/fail results; never fabricate "all passed".
2. Test **light, dark, system**, offline/no-model, slow model, no web provider, zero results, long titles, empty lists, and error states.
3. Verify keyboard-only navigation, visible focus, proper labels, accessible dialogs, Escape/outside click behavior where appropriate, contrast, 125% scaling, and reduced motion.
4. Check that research citations, source-backed vs user-recorded vs AI-inferred labeling, history semantics, reversible changes, provider credentials and vault persistence have not regressed.
5. Remove conflicting obsolete style declarations only after verifying they are unused. Don't mass-delete component CSS.
6. Capture before-and-after images for each screen and list any deviations from the reference designs (especially where usability required deviation).
7. Update `docs/design/implementation-progress.md` with actual changed files, test commands/results, screenshots, known unresolved pre-existing bugs and deliberate tradeoffs.
8. Provide an end-of-run summary: Completed, Deferred, Verified, Known Issues, and Next Recommended Work. If a required tool/environment is unavailable, do the highest-value feasible portions and report exactly what could not be checked.

---

## Execution strategy and checkpoints

- Work in this order: **baseline → tokens → primitives/shell → Home → Focus → All Files/Objects → Graph/Canvas → Settings/Explore → QA**.
- Finish each screen's functional + visual acceptance gate before moving to the next. Prefer frequent small commits/checkpoints when the worktree is safe; do not commit without checking repository conventions.
- Do not pause after every straightforward edit to ask the user what to do; follow the plan within scope. Only surface genuine blockers (e.g. missing required credentials or destructive migration).
- If time/context is limited, prioritize a **fully integrated tokens + shell + Home vertical slice** over half-finished CSS changes scattered across all screens. Record exact continuation tasks.
- Validate appearance in the **running app** using real existing data. The generated illustrations are targets, not a substitute for screenshots from the real implementation.

### Acceptance criteria: definition of done

- [ ] App runs and existing behavior is preserved.
- [ ] Tokens apply to actual UI with no conflicting theme regressions.
- [ ] Light / Dark / System themes are usable.
- [ ] Unified design vocabulary for navigation, cards, buttons, form inputs, tables, dialogs, status, badges and typography.
- [ ] Home is calm and resume-first, with real data and no invented summaries.
- [ ] Focus/editor preserves all important document functions and provenance distinctions.
- [ ] All Files + objects filters/actions work.
- [ ] Graph represents only actual data relationships; Canvas spatial placement remains separate.
- [ ] Settings are understandable without reducing advanced functionality.
- [ ] Empty, loading, saving, error, provider-offline and research-failure states are legible.
- [ ] Keyboard, focus, responsive and reduced-motion checks complete.
- [ ] Tests/build results and screenshot comparisons documented.

**Begin by examining the actual repository now. Then proceed through the phases with real code edits and validation. Do not respond with another design proposal instead of implementing it.**
