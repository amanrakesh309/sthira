# Working on Sthira

Read [the permanent policies](docs/policies/README.md) before product, UI, data, or verification work. Follow the product contract, design/interaction rules, and verification/evidence rules linked there. Latest explicit owner instructions take priority.

- Preserve the owner's vault and unrelated working changes. Test mutations in an isolated QA vault.
- Keep project overview navigation, the single rich editor, provenance, reference/membership/lineage separation, and reversible actions intact.
- Use the approved images in `docs/design/references/` as design targets. They are not proof of current execution.
- Capture fresh evidence for the current build; never copy old screenshots or reports into a new run and call them current verification.
- Put generated evidence in ignored `tmp/verification/<unique-run>/` or an external temporary directory. Do not recreate committed audit albums, dated reports, completion tables, or agent task prompts.
- Describe mocked tests, engine integration, browser preview, native GUI, and live-provider checks separately. Unexecuted work remains unverified.
- Update the relevant permanent policy when the owner changes a product/design decision. Keep concrete outstanding issues in `docs/known-limitations.md` until verified closed.
- Default test fixtures to temporary storage. Retained QA seeding must be explicit and must not overwrite an existing vault.
