# Codex Implementation Task — Sthira Visual Fidelity, Phase 1

You are acting as a senior React/TypeScript engineer with strong product UI, visual QA, and accessibility skills. You are **editing the existing repository**, not generating a new product or merely giving design advice.

## Context and objective

The product is **Sthira**, an AI-native, local-first knowledge workspace built with **Tauri v2, React, TypeScript, Tailwind CSS, and a Rust core**.

A prior agent implemented some colors and isolated style changes from a design kit, but the running application still looks substantially different from the approved visual mockups. The likely issue is **layout/composition and visual hierarchy**, not simply palette selection.

**Your objective:** make the **existing Sthira UI visually close to the approved mockups**, starting with the **application shell and Home screen**. Make actual code changes. Keep all current behavior and user data intact.

Project location supplied by owner:
`/home/aman/Projects/SthiraNotes`

Known existing frontend CSS location:
`/home/aman/Projects/SthiraNotes/crates/sthira-desktop/ui/src/styles/sthira-theme-tokens.css`

The above is a path hint, not evidence that any file exists. Inspect the real repository before changing paths. Operate in the active frontend under `crates/sthira-desktop/ui` if verified. Do **not** create a second `src` tree at the repository root.

## Mandatory reference material — inspect before editing

Find and read (use actual locations if these differ):

1. `docs/design/sthira-design-system-v1.md` — UI design blueprint and semantic design tokens.
2. `docs/design/references/home.png` — **approved visual target for Home**.
3. `docs/design/references/document-workspace.png` — subsequent Focus screen visual target.
4. `docs/design/references/all-files.png` — subsequent directory visual target.
5. `docs/design/references/knowledge-graph.png` — subsequent graph visual target.
6. `docs/design/references/settings.png` — subsequent settings visual target.
7. `crates/sthira-desktop/ui/src/styles/sthira-theme-tokens.css` — existing visual token implementation.
8. `sthira_product_understanding.md`, `docs/sthira_product_understanding.md`, or the actual product-understanding document in this repository — product/behavior authority.
9. Existing `AGENTS.md` files, `README`, `package.json`, Tailwind config, global CSS, entry point, component tree, frontend test setup, and Tauri configuration.
10. Existing implementation progress notes, if any.

Do not claim to have seen a reference image you cannot open; if screenshots are missing, record that limitation and use the available design specification, without inventing imagery. Do not blindly overwrite prior agent changes.

### Priority of sources

1. **Real application behavior and durable-data safety** (must not break).
2. **Product-understanding document and existing functional requirements**.
3. **Approved reference images for visual composition**, plus semantic design tokens.
4. Other sketches or prior agent suggestions.

Mockup text and imagery can be illustrative. Never fabricate data or status to achieve a visual match.

## Explicit non-goals / safeguards

- Do **not** rewrite or redesign the Rust backend, IPC contracts, DB schema, vault storage, provider adapters, model routing, or AI behavior.
- Do **not** replace actual projects, notes, research, sources, graph edges, model statuses, or resume summaries with demo/mock/hardcoded data.
- Do **not** remove features just to simplify appearance.
- Do **not** introduce a new UI framework or component library without a documented technical necessity.
- Do **not** use external font CDNs, unlicensed assets, or network-dependent UI resources. Preserve offline startup.
- Do **not** modify secret values, API keys, or local user data. Never `git reset --hard`, force-clean, or discard existing changes.
- Do **not** fake native window chrome shown in generated images; respect actual Tauri/Linux window behavior.
- Do **not** use one-off `!important` overrides or absolute positioning as the main way to align with an image.
- Do **not** claim a visual match if you did not inspect screenshots of the running implementation.

## Step A — baseline audit (short, evidence-led)

1. Inspect `git status` and repository instructions; preserve all existing work.
2. Determine the **actual** frontend package root, entry point, CSS import order, Tailwind version, theme selectors, and shared UI components.
3. Identify the real files responsible for:
   - app shell, sidebar, global header, navigation;
   - Home hero, universal input and mode controls;
   - project cards and "Pick up where you left off";
   - model selector, theme toggle and settings entry points.
4. Verify `sthira-theme-tokens.css` is loaded in the running UI **exactly once**, and look for conflicting hardcoded colors, global styles or Tailwind utilities.
5. Run existing frontend validation scripts that can execute in the environment (identify them from `package.json`). Record any existing failures without hiding them.
6. Launch the running frontend/Tauri app if feasible and capture a **baseline Home screenshot** at the same viewport as the target image (prefer target image native dimensions, and also test 1366×768). If GUI/screenshot tools are unavailable, report the limitation clearly.
7. Create `docs/design/visual-audit/home-baseline.md` with:
   - reference image path and actual screenshot path, if captured;
   - top 8–12 mismatches, ranked by visual impact;
   - corresponding React/CSS owners;
   - whether each is geometry, typography, density, component structure, color, or decoration.

Keep this diagnostic **concise**, and **proceed to implementation**. Do not stop after writing the audit.

## Step B — reconstruct composition, not just colors

Work from the reference's **large-scale spatial hierarchy first**:

**1. Shell / sidebar**
- Target sidebar ~264px default, with the existing resize/collapse behavior intact (if present).
- A stable, understated header ~56px; avoid duplicate global search interfaces and extra competing toolbars.
- Selected nav item must reflect the actual active screen; same geometry, icon sizing, and hover/focus behavior throughout.
- Present projects, notes/pages and custom objects as clear sidebar groups. Respect any cap/pinning behavior from existing product docs.
- Fix visual crowding, truncation, horizontal alignment, inconsistent icon treatment, and redundant metadata.

**2. Home hero**
- Recreate the mockup's generous top rhythm and intentional content container; target content max width about 1120–1240px, responsive.
- Editorial serif only for the main hero/section headings; readable sans-serif for controls and body text.
- Hero ~40–44px in ordinary desktop view (scale appropriately at 1366×768). Proper heading/subheading spacing.
- If the decorative landscape/wash asset isn't present, don't approximate it with a giant gradient behind content. Leave the top right quietly empty or use a subtle CSS-only accent.

**3. Universal input card**
- This is Home's visual center. It should read as **one cohesive card**, not a pile of pills.
- Recompose entry area, action row, mode controls (Capture, Research, Plan, Act), web search and model selector into meaningful groups with sensible alignment and hierarchy.
- Preserve actual submission, model selection, web search capability logic, keyboard shortcuts, focus and validation.
- Responsive handling: controls wrap or collapse logically, without overlap or clipping.

**4. Your projects**
- Build consistently sized project cards with clear title, compact context/metadata, useful short description if real data is available, quiet overflow actions, hover/focus states, and consistent 20–24px padding.
- Typical radius ~16px; use subtle borders, not heavy shadows.
- Don't show fake project cards, invented item counts, or artificial cover art.
- Make cards functional and keyboard-usable; preserve existing navigation.

**5. Resume-first ("Pick up where you left off")**
- This section is a priority. Use a concise row/card layout with item icon, title, grounded continuation detail where available, neutral metadata, and obvious Continue/Open action.
- Don't fabricate "next steps" if the knowledge base has insufficient evidence; use a factual fallback.
- Preserve Level 1 brief (~75 words maximum), Level 2 expanded review, provenance, and navigation.
- A visually elegant mockup is **not** permission to convert AI guesses into recorded progress.

**6. Main surface styles**
- Use semantic CSS variables rather than scattering hardcoded hex values.
- Consistent 4/8/12/16/24/32/48px spacing rhythm; 8px inputs/chips, 12px menus, 16px cards, 20px prominent input.
- App background `#F5F3EC`, canvas `#FAF8F3`, white surfaces, primary sage `#49583D`, charcoal `#1E201C`, border `#E8E5DC`, accent amber `#BE8B41`.
- UI typography: local Inter or system sans. Editorial headings: local Fraunces/Source Serif 4 if already licensed and present, otherwise Georgia/serif fallback.
- Keep color mostly neutral; green only for important actions/selection, amber sparingly.

Use component re-composition/refactoring when necessary. Reusing old container structure at all costs is **not** the objective. Preserve state/hook/IPC behavior while improving presentational structure.

## Step C — make shared components reusable

Create or refine existing shared primitives as appropriate:
- buttons (primary, secondary, ghost, destructive);
- cards and surfaces;
- text inputs and search;
- nav rows;
- segmented controls and filter chips;
- badges/type labels vs interactive controls;
- menus/dialogs.

Avoid duplicate design tokens, duplicate CSS imports, excessive wrappers, and conflicting active states. Keep light/dark/system themes. Prefer the current component organization; don't invent a large design-system package just for this.

## Step D — visual QA loop (mandatory when screenshot capability exists)

1. Capture the implemented Home at the **same viewport and display scale** as the reference/baseline.
2. Compare screenshot to approved reference **for geometry and composition**:
   - sidebar percentage of window;
   - main left/right margins and top offset;
   - hero line wrapping, baseline and vertical gap;
   - universal input width/height and row layout;
   - project-card heights/columns;
   - resume-section top position and row density;
   - typography family, size, weight, line-height;
   - borders, contrast, radii and shadows.
3. Make a ranked discrepancy list of remaining issues.
4. Correct the largest discrepancies and recapture at least once.
5. Also visually test 1366×768, dark theme and long titles; avoid overflow or nonfunctional off-screen controls.
6. Record actual screenshots and notes under `docs/design/visual-audit/`. If you cannot capture screenshots, state explicitly that visual parity is **unverified**.

Do not use arbitrary numeric "95% fidelity" claims; report concrete observed differences instead.

## Step E — functional verification and acceptance

Preserve and test:
- Home routing and sidebar active state;
- universal input Capture / Research / Plan / Act;
- keyboard shortcut behavior and mode switches;
- project/card click navigation;
- continue/resume navigation;
- model/provider selector, web search controls;
- theme Light, Dark, System;
- focus-visible, keyboard navigation, interactive 40px+ hit targets;
- states when no projects/notes/models are configured, and loading/error/offline states.

Run relevant available lint, type-check, test and build scripts without changing their meaning. Report command results. Distinguish pre-existing errors from errors introduced by this change. No unrelated refactors.

## Phase boundary — STOP AFTER HOME

**Implement the shared shell and Home screen in this run, then stop.**

Do **not** launch a broad rewrite of Focus, All Files, Graph, Settings or Explore & Plan in the same run. Their mockups are provided to establish design consistency, not to expand this phase's scope.

Once shell/Home is validated, the subsequent phases can be:
1. Focus/document workspace + resume brief/history/margin.
2. All Files + projects/custom objects.
3. Knowledge Graph + Canvas.
4. Settings/providers + Explore & Plan.
5. Final responsive/accessibility/dark-mode polish.

## Deliverables in your final report

Provide:
1. Concise summary of implemented UI improvements.
2. Exact modified file paths and key components.
3. Baseline and final screenshot paths (if captured) and concrete visual differences still remaining.
4. Tests/build commands with PASS/FAIL results.
5. Known pre-existing issues and any blockers.
6. Recommended next design area.

**Success definition:** Home should exhibit the approved reference's visual **composition and quality**, not merely its color palette; all existing behavior remains functional; code is maintainable; visual verification is evidence-backed.

Now inspect the real repository and **make the implementation changes**, starting with the audit and proceeding to shell/Home.
