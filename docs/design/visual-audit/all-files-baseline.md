# All Files Baseline Audit — 2 November 2026

**Repository:** `/Volumes/Aman Ext/Projects/sthira`  
**Frontend package:** `crates/sthira-desktop/ui`  
**Approved visual reference:** `docs/design/references/all-files.png` (1672×941).  
**Baseline captures:**
- [all-files-before-1672.jpg](all-files-before-1672.jpg) (1672×941)
- [all-files-before-1366.jpg](all-files-before-1366.jpg) (1366×768)

Captures reflect the running application with isolated QA vault records (3 projects, 3 notes, 1 decision object) in the browser fallback runtime at 1× display scale. Rust test suite (119/119) and UI vitest suite (104/104 in 21 files) pass.

---

## Ranked Visual & Interaction Discrepancies

| Rank | Discrepancy | Owner / Files | Category |
|---|---|---|---|
| **1** | **Missing secondary filter & sort row on All Items**: The reference has `Show:` (All / In Projects / Standalone), `Type:` (registered type chips), and `Sort:` (Last Modified dropdown). In the baseline, filters are hidden on `all` and only fragmentarily shown on `notes` or `objects`. | `WorkspaceIndexView.tsx` | Structure / Hierarchy |
| **2** | **Missing directory footer & client-side pagination**: The reference provides a distinct footer showing `Showing 1–8 of 11 items`, `Rows per page: 25 ∨`, and page buttons `[ < ] [ 1 ] [ 2 ] [ > ]`. The baseline has no footer or pagination controls. | `WorkspaceIndexView.tsx` | Interaction / Geometry |
| **3** | **Header divider, icon and typography discrepancies**: Baseline divides the header with an unnecessary full-width border line, uses inconsistent eyebrow `Workspace / ALL FILES`, omits the amber/gold folder icon next to `All Files`, and lacks the `⌘F` shortcut badge in search. | `WorkspaceIndexView.tsx`, `all-files.css` | Composition / Typography |
| **4** | **Category tab styling**: The reference uses independent pill buttons (`All Items [11]`, `Projects [2]`, etc.) with dark sage active state and white/bordered inactive pills. Baseline encloses them in a heavy gray segmented container track without individual borders. | `WorkspaceIndexView.tsx`, `all-files.css` | Component Styling |
| **5** | **Table row entity icons & colored containers**: In the reference, each row has a colored rounded icon container matching its entity type (warm amber for projects, light blue for decisions, soft stone for notes, green for tasks). Baseline renders plain gray monotone squares. | `WorkspaceIndexView.tsx` | Visual Polish / Contrast |
| **6** | **Semantic type badges**: Reference has distinct badges with icons and semantic colors (e.g., `⚖ Decision` in blue, `📁 Project` in purple, `Note` in warm stone). Baseline uses identical plain gray uppercase text tags (`PROJECT`, `NOTE`, `DECISION`). | `WorkspaceIndexView.tsx` | Semantics / Color |
| **7** | **Project column formatting**: For standalone items and projects, baseline displays `Standalone` text or item count badges inside the project column. Reference cleanly uses an em-dash `—` for standalone/project rows and `📁 <Project Name>` for attached notes. | `WorkspaceIndexView.tsx` | Data Formatting / Noise |
| **8** | **Action column clutter vs clean overflow**: Baseline exposes raw pushpins and external link buttons on every row next to the three-dots menu. Reference uses a clean, quiet `···` overflow menu and single open affordance. | `WorkspaceIndexView.tsx`, `PageActionMenu.tsx` | Density / Interaction |
| **9** | **Row selection checkbox column**: Reference includes rounded selection checkboxes for table rows. Baseline completely omits the checkbox column. | `WorkspaceIndexView.tsx` | Table Composition |
| **10** | **Projects card grid metadata and alignment**: The Projects tab card grid lacks the refined Warm Stone cards, typography rhythm, and compact horizontal folder-title groups specified in the design system. | `WorkspaceIndexView.tsx` | Card Hierarchy |
| **11** | **Shared sidebar contrast and tree population**: When running in browser fallback, the sidebar projects tree shows empty state unless tree projects are populated; selected row contrast in shared sidebar needs refinement. | `LeftSidebar.tsx` | App Shell Polish |
| **12** | **Dark mode token adherence & responsive wrapping**: Verify semantic tokens (`#161714` canvas, `#1C1D19` surface, `#2C2E27` border, `#879E78` primary) across all tabs, type filters, and narrow widths (1366px, 900px). | `all-files.css`, `WorkspaceIndexView.tsx` | Theme / Responsive |

---

## Baseline Verification Evidence

- `npm test`: 21 test files passed, 104 tests passed.
- `npm run build`: 0 errors (1.63s).
- `cargo test --workspace --offline`: 119 passed, 0 failed.
- `git diff --check`: clean.
- Existing Home and Focus phase enhancements preserved in working tree.
