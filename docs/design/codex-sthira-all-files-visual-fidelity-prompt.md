Implement Sthira’s next visual-fidelity phase: **All Files, project browsing, and custom-object browsing**.

This is an implementation task in an existing Tauri + React + TypeScript application. Make the changes, run the application, inspect screenshots, correct discrepancies, and complete validation. Do not stop after an audit or plan.

## Repository and current state

Active macOS repository:
`/Volumes/Aman Ext/Projects/sthira`

Original Linux repository location:
`/home/aman/Projects/SthiraNotes`

Use the existing checkout available in your environment. Verify the actual location before acting.

Frontend:
`crates/sthira-desktop/ui`

Home/application-shell and Focus/document-workspace correction phases have already been implemented. Their changes may still be uncommitted. Preserve them and all other existing work.

Do not reset, clean, overwrite, or recreate the frontend.

## Mandatory inspection before editing

Read:

- Applicable `AGENTS.md` instructions.
- `docs/design/codex-sthira-visual-fidelity-prompt.md`
- `docs/design/sthira-design-system-v1.md`, especially All Files.
- `docs/design/references/all-files.png` — open the image; this is the primary visual target.
- `docs/sthira_product_understanding.md`
- `docs/design/implementation-progress.md`
- Existing reports under `docs/design/visual-audit/`, including Home and Focus validation reports.
- `crates/sthira-desktop/ui/src/styles/sthira-theme-tokens.css`
- Existing `home-shell.css`, `focus-workspace.css`, global CSS, stylesheet import order, Tailwind configuration, package scripts, and test setup.

The earlier implementation prompt limits its original run to Home. This request explicitly authorizes the subsequent All Files/projects/custom-objects phase. All its data-safety and verification safeguards still apply.

Inspect the actual implementation, including:

- `src/views/WorkspaceIndexView.tsx`
- `src/components/LeftSidebar.tsx`
- `src/components/PageActionMenu.tsx`
- `src/components/AddToProjectModal.tsx`
- `src/components/ProjectDeletionModal.tsx`
- `src/components/objects/ObjectCollectionContainer.tsx`
- `src/components/objects/ObjectTableView.tsx`
- `src/components/objects/ObjectKanbanView.tsx`
- `src/components/objects/ObjectTypeManagerModal.tsx`
- Relevant object builders, property renderers, and template controls.
- Routing/callbacks in `src/App.tsx`
- Relevant IPC wrappers and TypeScript types.

Paths above are relative to the frontend package unless otherwise stated. Verify them; do not assume their contents.

## Scope

Primary:
1. All Files / All Items directory.
2. Projects category.
3. Notes & Pages category.
4. Custom Objects category and type filtering.
5. Object collection table/board presentation and its existing creation/edit controls.
6. Manage Types presentation where needed for visual consistency.

Fix directly related shared-sidebar selection contrast or navigation defects if found.

Do not redesign Home, Focus, Graph, Canvas, Settings, or Explore & Plan. Preserve their completed work. Do not expand into an object-schema/backend redesign.

## Non-negotiable safeguards

Preserve:

- Existing React state, routing, callbacks, keyboard shortcuts, and refresh events.
- Tauri IPC commands, argument shapes, Rust behavior, database and vault formats.
- Real projects, notes, object types, properties, templates, provenance, relationships, and timestamps.
- Existing pinning, sorting, filtering, navigation, attachment, and page actions.
- Project detach versus deletion semantics, especially artifacts shared by multiple projects.
- Object property editing, persistence, board grouping/movement, creation defaults, project attachment, and template behavior wherever implemented.
- Light, Dark, and System theme support.
- Offline startup.

Do not fabricate items, counts, statuses, descriptions, relationships, or classifications to match the image.

A generic note must remain a Note unless real application data identifies another type. The mockup’s Source/Idea/Task labels are illustrative, not permission to reclassify data.

Do not add a framework, dependency, external font CDN, network-dependent asset, or new palette without demonstrated necessity.

Reuse existing semantic theme tokens, imported exactly once. Avoid scattered hardcoded colors, `!important`, and absolute positioning for the main layout.

Do not modify secrets, provider settings, or owner data. Use an isolated QA vault for mutation tests. A previous temporary QA vault/app may exist; verify its isolation before reusing it.

## Step 1 — establish a real baseline

Before modifying code:

1. Inspect `git status` and distinguish existing changes from this phase.
2. Run the available baseline checks and record results.
3. Launch or connect to the actual application.
4. Open All Files and inspect all category tabs and existing object-collection views.
5. Capture baseline screenshots, preferably at:
   - 1672×941, the approved image dimensions.
   - 1366×768.
6. Inspect existing functionality through the UI, not source alone.
7. Record a concise ranked audit in:
   `docs/design/visual-audit/all-files-baseline.md`

Identify 8–12 important differences with their React/CSS owners. Cover composition, proportions, typography, density, alignment, spacing, and hierarchy—not just colors.

If native tooling is unavailable, use the real frontend in a browser and clearly distinguish browser fallback behavior from verified Tauri behavior.

## Step 2 — reproduce the approved composition

Use `all-files.png` as the visual composition authority, with product behavior and real data taking precedence.

### Overall hierarchy

Reproduce the image’s structure:

- Existing understated application sidebar.
- Small Workspace breadcrumb/eyebrow.
- Prominent editorial “All Files” heading and readable subtitle.
- Local directory search and Manage Types action near the upper right.
- A distinct category-tab group.
- A quieter secondary filter/sort row.
- One coherent directory surface with aligned columns and restrained borders.

Use the existing ~264px sidebar default. Do not enlarge it merely to imitate the image’s illustrated proportions.

Use a generous, responsive content container. Determine its dimensions from screenshot comparison rather than retaining the old wrappers by default.

If the landscape artwork is unavailable, leave the header quietly open. Do not replace it with a large generic gradient.

### Categories versus filters

Visually distinguish:

- Category tabs: All Items, Projects, Notes & Pages, Custom Objects.
- Scope controls: All / In Projects / Standalone, where applicable.
- Actual registered object-type filters.
- Sort controls.

Do not make every control look like an interchangeable pill.

Preserve existing filter semantics. Inspect which controls currently apply to which categories. Do not silently broaden or remove behavior to imitate the mockup.

Counts must come from real data. Clearly distinguish workspace counts, filtered-result counts, and visible-page counts.

### Directory rows

Default All Items to the approved dense but breathable list/table composition:

- Title and real description/snippet.
- Actual type.
- Project association where applicable.
- Modified timestamp.
- Quiet overflow/open actions.

Use consistent column alignment, row padding, icon containers, separators, and readable metadata. Keep the title visually dominant.

Reuse genuine snippets already supplied by the application. Do not invent summaries or display raw Markdown noise when an existing safe text-preview approach is available.

Use actual object types and registered icons. Do not hardcode the mockup’s four example types as the complete system.

Preserve project/card or collection modes where they provide existing functionality.

### Actions and interaction

Keep row opening and nested actions independent:

- Clicking a pin, project link, overflow menu, attachment control, or type link must not accidentally open the row.
- Essential actions must be available on keyboard focus, not only pointer hover.
- Rows must support appropriate keyboard navigation and activation.
- Preserve existing PageActionMenu behavior and mutation refreshes.
- Keep visible focus and approximately 40px interactive targets.

Do not add decorative selection boxes, sort arrows, pagination, or view switches that do nothing.

If adding client-side pagination to reproduce the reference, implement it completely: real filtered totals, correct page ranges, working controls, and page reset/clamping after search, filter, or deletion. Do not change backend contracts for pagination in this phase.

Only add bulk-selection controls if they support an actual, authorized workflow. Otherwise omit the mockup’s unsupported affordance and document that difference.

### Projects and custom objects

Carry the same typography, spacing, surfaces, metadata hierarchy, menus, and empty states into the Projects and Custom Objects categories.

For object collections, preserve existing table/board capabilities and property editors. Improve their visual presentation without changing schema interpretation or persistence.

Manage Types must continue opening the existing manager. Preserve type creation/editing and template workflows. Limit changes to the presentation and directly necessary interaction fixes.

### Responsive and theme behavior

At narrower desktop widths:

- Collapse or reposition optional metadata before destroying title readability.
- Wrap filter groups intentionally.
- Keep menus within the viewport.
- Keep essential actions reachable.
- Allow intentional inner scrolling for genuinely wide property tables; avoid accidental page-wide overflow.

Check both themes using semantic tokens. Verify selected text, badges, icons, hover states, disabled controls, and menus—not just backgrounds.

## Step 3 — preserve and verify behavior

Build a behavior checklist from the current application and tests.

At minimum verify:

- Sidebar All Files navigation and correct active destination.
- Sidebar View all links opening the intended category.
- Category switching and externally supplied initial-tab changes.
- Search and clearing search.
- Notes scope filters and all existing sort modes.
- Real counts and empty/no-results states.
- Project, note, and object opening with correct IDs/context.
- Project association navigation and standalone-note attachment.
- Pin/unpin persistence and ordering.
- Page actions, including rename/duplicate/link/detach where implemented.
- Correct shared-artifact deletion semantics through existing workflows.
- Custom-type filtering and collection navigation.
- Object creation, property editing/persistence, and existing board behavior.
- Manage Types and templates.
- Loading, failed requests, retry, and mutation feedback.
- Keyboard access, menu dismissal/focus restoration, and theme switching.

Use isolated QA data for mutations. Do not delete owner records to test a visual change.

Do not claim a feature works merely because its button remains in the JSX.

## Step 4 — mandatory screenshot correction loop

After the first implementation:

1. Run the application and capture screenshots.
2. Compare them directly against `all-files.png` and the baseline.
3. Rank remaining discrepancies by visual impact.
4. Correct the largest discrepancies.
5. Recapture at least once after those corrections.

Compare concrete geometry:

- Sidebar/content proportions.
- Header top offset and title size.
- Search and Manage Types placement.
- Category-tab dimensions and separation from filters.
- Filter/sort alignment.
- Directory width and top position.
- Column widths, title truncation, row height and density.
- Icons, badges, borders, radii and metadata contrast.
- Menus, footer/pagination if implemented, and scrolling.

Also inspect:

- 1366×768.
- A smaller desktop window around 900px wide.
- Dark mode.
- Long titles and project names.
- Empty and filtered-no-results states.
- Multiple custom types and property-heavy collections where QA data permits.

Save original screenshots under `docs/design/visual-audit/` with clear before/pass1/final names. Do not edit screenshots to conceal defects.

Report tooling limits honestly. A screenshot file existing is not proof that you visually inspected it. Do not claim native parity from browser-only captures.

## Step 5 — validation and completion

Discover and run the repository’s relevant scripts.

Expected frontend commands, if unchanged:

From `crates/sthira-desktop/ui`:
- `npm test`
- `npm run build` — includes TypeScript checking.

From repository root:
- `cargo test --workspace --offline`, if dependencies permit.

Run `git diff --check`.

Do not invent a lint script or alter check configuration to obtain a pass. Distinguish a frontend build from a packaged Tauri release build.

Inspect existing tests for workspace organization, page actions, custom objects, type management, deletion, and themes. Add meaningful behavioral regression coverage for changed interactions. Do not replace assertions with weaker ones or rely only on snapshots/class-name tests.

Record actual final test counts and failures. Earlier phase counts are historical evidence, not a target to copy.

Create:
`docs/design/visual-audit/all-files-validation.md`

Include:

1. Implemented changes and exact changed-file manifest.
2. Before, first-pass, and final screenshot paths and dimensions.
3. Concrete screenshot comparisons and second-pass corrections.
4. Functional checks, distinguishing native/manual checks from mocked tests.
5. Commands and actual PASS/FAIL results.
6. Remaining visual differences and unverified behavior.
7. Existing warnings and tooling limitations.
8. Confirmation that previous Home/Focus work and backend/data behavior were preserved.

In your final response provide a concise implementation summary, validation results, links to the report and before/after screenshots, and at least one displayed final screenshot if supported.

Do not report an invented fidelity percentage. Do not claim completion based on colors, code compilation, or test success alone.

Complete this scoped implementation phase. Stop before beginning Graph/Canvas or another major screen.
