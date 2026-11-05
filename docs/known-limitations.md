# Verification limits and follow-ups

This file tracks unresolved work, not a whole-product pass/fail certificate. Update an item only with evidence from the execution layer it names. Permanent requirements live in [policies](policies/README.md).

- **Full native visual and lifecycle verification remains open.** Engine-reopen integration, mocked components, and browser spot checks do not establish a current page-by-page Tauri GUI pass, process restart, all responsive/theme/error states, or all eight connected journeys.
- **Live provider/web and cross-platform/assistive-tech execution remain unverified where unavailable.** Do not infer success from deterministic fake providers, test names, token tables, or another platform's browser.
- **Search keyboard/accessibility needs a complete pass.** Current source handles arrows/Enter on the input and restores an opener on dismissal, but Tab containment and activation of a directly focused result option are not established. Test stale async query/close results as well as happy-path input traversal.
- **Toolchain/performance checks must be reported honestly.** Clippy/rustfmt may require installed components. A frozen router test has a 50ms aggregate timing assertion that has been sensitive to concurrent build load; preserve failures/retests. The production bundle also has a size advisory; a successful build does not establish a startup performance measurement.

The temporary audit reports and screenshots were removed at the owner's request. Their prior completion claims are not carried forward. Approved visual targets, reproducible tests, and product/design contracts remain.
