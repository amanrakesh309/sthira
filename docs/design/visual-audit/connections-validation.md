# Workspace connections validation

5 November 2026. This is a new feature following Sthira's existing design language; no separate approved reference image was supplied.

## Automated verification

- `npm test`: **28 files, 154 tests passed**.
- `npm run build`: TypeScript and Vite passed. Vite reports the existing large-bundle advisory.
- `cargo test --workspace --offline`: passed across the workspace, including the six new connection integration tests.
- After the final graph filtering adjustment, `cargo test -p sthira-desktop --test workspace_connections_test --test deletion_and_graph_test --offline`: **10 passed**.
- `git diff --check`: passed.
- `rustfmt` was unavailable in the installed Rust toolchain; no formatter execution is claimed.

New frontend coverage (`workspace_connections.test.tsx`, eight tests):

- Object/project preview, live property values, backlinks, nested Back navigation and keyboard return.
- Search, self-exclusion, existing-link state and selected reference insertion.
- Unlink and Undo.
- Missing reference recovery and ignoring stale responses after navigation.
- Original editor identity and dirty-text preservation during preview.
- Internal stable-ID links and normal external-link behavior.
- Caret insertion and preserving text when creating a connection fails.
- Project-owned references independently of the open page.

New backend coverage (`workspace_connections_test.rs`, six tests):

- Artifact/object/project endpoint combinations, backlinks, graph edges and membership independence.
- Restart persistence, current title resolution, idempotent create and content-preserving unlink.
- Self-link, missing target and soft-deleted target rejection.
- Shared-page detach versus permanent deletion cleanup.
- Project preview members and cascade deletion cleanup.
- Additive migration from version 11 and repeated reopening.

## Running browser verification

The real Vite frontend was exercised through the in-app browser with manually created QA pages, using its existing browser-session storage. No owner vault was opened or modified. Checked page creation, explicit linking, incoming backlinks, nested preview and Back, picker insertion, rendered inline-link opening, Escape/close, themes and responsive layout.

| Screenshot | Viewport | Observed state |
|---|---|---|
| [preview-light-1672.png](connections/preview-light-1672.png) | 1672 × 941 | Main page in Preview with clickable internal reference; referenced document and backlink beside it. |
| [preview-dark-1672.png](connections/preview-dark-1672.png) | 1672 × 941 | Dark reference preview with original editor retained and sidebar collapsed. |
| [rail-light-1366.png](connections/rail-light-1366.png) | 1366 × 768 | 320px connection rail; branching reference card, outgoing count and backlink section. |
| [preview-light-900.png](connections/preview-light-900.png) | 900 × 768 | 420px reference drawer with close/back controls and independently scrollable content. |

DOM measurements confirmed document width equals viewport width at 1366 and 900px; no horizontal document overflow. At 1672px, the document workspace split into approximately 760px for the original page and 648px for the preview with a 264px application sidebar. The original page stayed mounted during reference navigation. Temporary viewport overrides were reset.

## Verification limits

Browser screenshots verify frontend composition and interaction, not native IPC or desktop persistence. Persistence, migrations, object properties, project membership independence and deletion semantics were verified against real temporary SQLite vaults by Rust integration tests. A native Tauri GUI smoke test and packaged release build were not performed. Native keyboard focus, OS scaling and non-macOS platform behavior remain unverified.
