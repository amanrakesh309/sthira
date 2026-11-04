# Project overview validation — 5 November 2026

## Behavior

Project navigation now opens a dedicated contents overview. It reads saved state without auto-opening a document, creating content, or generating a resume brief. See [feature behavior](../project-overview.md).

Verified project metadata, member/reference/backlink deduplication, custom object types, canvas inclusion, soft-deleted/detached exclusion, shared artifact context, empty/missing projects, filtering, sorting, pagination, explicit creation, add-existing success/failure, live connection updates, and keyboard search. App routing tests cover project entry, external document context, saving before returning, retained browse state, saved canvas navigation, and return to the Projects directory.

## Automated checks

- Frontend: `npm test` — **183 tests passed across 31 files**.
- Rust: `cargo test --workspace` — **131 tests passed**, including five project overview SQLite integration tests.
- Production: `npm run build` — passed TypeScript and Vite compilation. Existing bundle-size advisory remains.
- `git diff --check` — passed.

## Browser review

Used browser-session records created through the UI, starting from an empty preview: Knowledge platform, two member pages, Shared guidelines, and Research library. Linked the separate page and project, attached the shared page to Research library without copying it, and opened a filtered reference then returned with search, relationship filter, and name sorting preserved. Clicking the sidebar project root opens the overview without creating a page.

Reviewed a populated light list at 1672px and the normal 1150px viewport, a dark grid at 1366px, and a compact light list and connection drawer at 900px. At 1366px and 900px, document width equaled viewport width; no horizontal document overflow. Compact rows retain type, relationship, and edited metadata beneath the title. Restored the normal viewport and light theme after review.

- [Light desktop list](project-overview/list-light-1672.png)
- [Light normal viewport](project-overview/list-light-default.png)
- [Dark grid](project-overview/grid-dark-1366.png)
- [Compact list](project-overview/list-light-900.png)
- [Compact connections drawer](project-overview/connections-light-900.png)

## Limits

Visual review used the browser preview, not a running native Tauri window. Browser records are session-only, and some pre-existing editor resume APIs still use demo content; those are not evidence of native project facts. The SQLite integration tests validate native data selection and isolation in temporary vaults. The owner's vault was not modified. Browser visuals exercised pages and related projects; native canvas/custom-object inclusion is covered by backend tests and their routing by frontend tests.
