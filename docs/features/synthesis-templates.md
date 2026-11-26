# Synthesis templates and agent Research/Plan

Synthesis templates are reusable starting structures for Research, Plan, and optional Capture notes. They are separate from object-type templates. The contract lives in [the product policy](../policies/product.md); this page describes usage and current limits.

## Managing templates

Open **Settings → Templates**. The manager can:

- filter by Research, Plan, Capture, or All and show archived entries;
- preview the rendered Markdown with an example title;
- create and edit custom templates (name, workflow, description, structure, optional guidance);
- duplicate built-in or custom templates and customize the copy;
- set or clear at most one default per Research/Plan workflow;
- archive, restore, or delete custom templates with an explicit confirmation.

Built-ins are read-only. Startup seeding only inserts missing built-ins; it never overwrites owner edits, resets a chosen default, or unarchives a built-in. Deleting a custom template never deletes pages or saved runs.

Validation is enforced in `sthira-core`: supported workflow, nonblank/bounded text, compatible defaults, and stale-revision rejection. Only `{{title}}` (and legacy `{Title}`) are substituted; other placeholders remain visible. Templates cannot execute code, read files, fetch URLs, or grant tools.

## Selecting a template

Research and Plan pages offer three choices:

- **Inherit default** — use the workflow default, if one is configured;
- **this template** — an explicit compatible template for this run;
- **No template** — send no structure.

A **Manage** link opens the Settings manager. The Home composer does not add a template selector; Home-initiated Research/Plan may inherit configured defaults. Turning templates off for a workflow sends no structure, with a distinct disabled decision in execution history.

Direct streamed Plan captures a receipt at dispatch containing the effective template mode, immutable ID/revision/name/structure/guidance, and producing model/provider. Saving uses that receipt even if the template is edited, deleted, or replaced as the default while generation is running. A stable generation key makes repeated save callbacks idempotent. Explicit no-template and disabled decisions are also recorded; failed generations do not create successful execution records. SQLite v30 adds this receipt metadata and durable agent progress.

Capture structures are model-free. From the page editor choose **Structure**, preview a Capture template, and insert it at the cursor. Existing text, selection, and Undo are preserved; headings are not expanded into AI prose.

## Agent Research and Plan

The workspace agent can list/read compatible templates, request web research, and propose a reviewable Research or Plan page. Advanced controls in the Agent composer expose:

- **Allow web research** (default off) — persists with the run and is inherited, not widened, by follow-ups;
- Research/Plan template modes — inherit, none, or a specific template; explicit owner choices cannot be replaced by the model.

Workflow gates are rechecked at dispatch: Agent, Research, Plan, template enablement, and the web grant. Web access requires the workflow, the run grant, an enabled search provider, and the model’s explicit `use_web` request. Project scope filters local retrieval before model context. Workspace memory is a separate opt-in grant.

While Research or Plan is running, the Agent view polls durable progress for real context, template, retrieval, fetching, extraction, and generation stages as applicable. The current stage is persisted with the run, with up to fifty stage events retained. Cancellation is checked between search, fetch, extraction, and synthesis boundaries; it records a terminal stopped state and prevents saving a proposal from that run.

Web Research sends only bounded, relevance-selected passages to quote extraction and synthesis, so facts later in a long article can contribute even when the page opens with navigation or boilerplate. Exact-quote verification still uses the complete fetched document. Each source keeps one stable number through synthesis, findings, the proposal packet, and review. Quote extraction has a per-source deadline, and a failed or timed-out source is reported as partial evidence while usable sources continue. If synthesis itself fails, the run returns an error and creates no proposal.

Generation is non-publishing. Before review there is no page, project change, shared memory, imported source, or accepted claim. Review shows operation/destination, proposed rich text, source identities and versions, producing model/provider, template snapshot, search grounding, warnings, and open questions. Source titles are decoded as text. Valid inline `[Source N]` markers, groups, and ranges link to the numbered bibliography; invalid references remain inert. The review distinguishes web-backed proposals from a permitted search that contributed zero external sources. Apply creates the page once, marks it AI-authored, records template execution history, and preserves derivation lineage for plan-from-research. Dismiss leaves knowledge unchanged.

Research-to-plan follows: accept research first, then ask for a follow-up plan from that saved page. An unaccepted research proposal has no artifact identity.

## Acceptance checklist

The current acceptance pass uses local fixtures with web access off for agent journeys. Scripted model responses exercise the app's actions and persistence in an isolated vault. Loopback fixtures can separately exercise web retrieval and citation handling without a live provider.

- Verify the agent can read permitted project material, propose Research, and show the destination, evidence, and template in review. Dismiss one proposal and confirm that it created neither a page nor a completed execution receipt.
- Accept a separate local Research proposal, reopen the vault, and generate and review a follow-up Plan using the accepted page. Verify the saved Plan's lineage, template receipt, and source checks. Exercise cancellation, unavailable or malformed model responses, and stale or out-of-scope sources.
- Verify citation rendering and keyboard navigation with fixture data. Record native review at normal and 125% scaling and interactive screen-reader speech as separate checks.
- **Deferred live checks.** Rerun ordinary-language source selection and Plan generation with Inkling 2 Mini when its network connection is stable. Test Agent and direct Research using a normally configured live search provider, freshly fetched articles, and verified citations through review and reopen. Keep these checks open when fixture tests pass.

## Deferred live checks

- Ordinary-language source selection and Plan generation with Inkling 2 Mini once that connection is stable. Fixture coverage now exercises the captured `{"name":"search","args":{...}}` wrapper and the accepted Research → Plan lineage.
- Native Agent Research with a normally configured live search provider and freshly fetched article bodies. Verify source links, displayed quotes, citation attribution, acceptance, and reopen. The NASA adapter/cached-body runs remain historical diagnostic evidence, not a current configured-provider pass.
- Successful direct web Research from the dedicated Research screen, including saved sources and template history. The prior direct web attempts timed out; the fixture pass verifies deadline reservation and local direct/template behavior only.

## Current limits

- Deterministic loopback fixtures and isolated SQLite vaults cover unit/engine behavior; mocked UI tests and browser fixtures cover presentation separately. Native template preview/duplicate/edit/default/archive/restore and relaunch persistence, a direct local-model Plan receipt, active Agent cancellation, live synthesis progress, and loopback-fixture Agent Research/Plan review (at 100% and 125% scaling with citation focus and Escape restoration) have been observed in an isolated Linux QA vault. General live model/template quality, interactive screen-reader speech, deferred live-provider/web checks, and non-Linux support remain open.
- A native Linux QA run with Mac-hosted Inkling 2 Mini and web access off generated two separate Four Lenses Research proposals from the same synthetic project. Native review showed the template and vault sources. Dismissing one created no page or execution receipt. The other was accepted into the project, and a follow-up that explicitly named its saved page ID created a reviewed Editorial Launch Plan with persisted Research lineage. In the later reliability batch, a natural-language “turn that saved research into a plan” follow-up failed twice before proposing anything: Inkling returned an unreadable agent action. That natural-language lineage check remains open; no internal tool name or artifact ID was supplied in those attempts.
- Agent Research validates individual citations, grouped numbers, and ranges against the supplied source index. The synthesis request pairs each verified finding and quote with its source number. A native Linux run with Mac-hosted Inkling 2 Mini and live NASA site search through a temporary SearXNG-compatible adapter reached review with two sources. Bounded, relevance-selected passages let the draft distinguish the July 11 5 p.m. White House preview from the July 12 10:30 a.m. EDT broadcast and cite the NASA release supporting each. Review exposed navigable citation links and decoded source titles. Dismissing that proposal left artifacts and template history unchanged. A second accepted proposal persisted its page and explicit Four Lenses receipt across relaunch. DuckDuckGo challenged the separate configured-path QA run, and two-source retries can still consume the three-minute Agent deadline during fetch plus synthesis. A template-backed direct Research run succeeded only from the local vault after web-backed direct attempts hit the 180-second model timeout; it recorded four local sources and a completed Four Lenses receipt. Synthesis can still omit or conflate facts present in the retrieved passages; citation validity is not factual sufficiency.
- Live progress reflects pipeline stages, not token-by-token model output. Stage visibility depends on the UI polling interval; very short stages can pass between refreshes while remaining in durable history.
- Fetched-article URLs are restricted to http/https, bounded redirects, size/content limits, and public destinations unless the exact origin is a configured local service endpoint. The transport resolves each destination once, rejects mixed public/private answers, pins validated addresses into the HTTP client, disables implicit proxies, and repeats validation/pinning for redirects. IPv4-mapped IPv6 addresses receive the same destination checks. A configured service origin does not authorize other private origins or ports.
