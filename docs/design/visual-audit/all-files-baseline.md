# All Files correction baseline — 2–3 November 2026

This audit supersedes the previous model's report, archived in `all-files-baseline-previous-model.md`. Checkout began clean at `458d920`; existing Home and Focus corrections were preserved. The approved composition is `../references/all-files.png` (1672×941).

The actual macOS Tauri QA app was inspected before edits, using `/private/tmp/sthira-visual-qa-vault`. Native baseline: `all-files-correction-before-native.jpg` (1920×960), three real projects and three real notes. These are persisted QA records, not browser fixtures. Baseline `npm test`: 111 tests / 22 files passed. The preceding review also passed the build and 119 Rust tests, while five focused behavioral checks exposed regressions.

## Ranked differences and owners

| Priority | Finding | Owner |
|---|---|---|
| 1 | Object collection table, board, property fields and creation form retained light-only surfaces and compact controls; the requested custom-object phase was incomplete. | `components/objects/*` |
| 2 | Directory surface was about 1176px wide at 1920px, beginning around x504. It left too much unused content space compared with the approved composition. | `all-files.css`, `WorkspaceIndexView.tsx` |
| 3 | Categories were detached capsules rather than a coherent outer group, with inadequate distinction from scope/type chips. | `all-files.css` |
| 4 | Table typography used small uppercase column labels; description and metadata hierarchy were too compressed. | `all-files.css` |
| 5 | Search and Manage Types were visually small; native rows-per-page select rendered roughly 20px high. Controls needed actual native inspection, not CSS assumptions. | `all-files.css` |
| 6 | Real snippets repeated H1 titles and exposed Markdown markers. | `WorkspaceIndexView.tsx` |
| 7 | Type badges hardcoded built-in names/icons/colors instead of honoring registered customization. | `WorkspaceIndexView.tsx` |
| 8 | Project and attachment menus lacked consistent keyboard dismissal/focus and viewport protection; nested Enter/Space events could open the row. | `WorkspaceIndexView.tsx`, `PageActionMenu.tsx` |
| 9 | Sidebar active-note metadata used muted ink on the selected surface. | `LeftSidebar.tsx` |
| 10 | Pagination displayed a clamped page but retained stale state; Previous could require multiple clicks after results shrank. | `WorkspaceIndexView.tsx` |
| 11 | Failed loads/mutations appeared empty or silent, and late search responses could replace newer results. | `WorkspaceIndexView.tsx`, `ObjectCollectionContainer.tsx` |
| 12 | Production browser IPC fallback contained fabricated QA records; its populated screenshots could not establish native behavior. | `api/ipc.ts` |

The header artwork and wider illustrated sidebar are deliberate exceptions: the specification retains the existing 264px default sidebar and permits a quiet header when approved art is unavailable. Real classifications and counts take precedence over illustrative mockup content.
