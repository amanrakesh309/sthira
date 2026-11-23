use super::*;
use sthira_core::workspace_agent::{
    external::{ExternalBatch, ExternalGrant},
    handoffs::*,
};
fn external_grant(e: &Engine, project: Option<String>, propose: bool) -> (String, String) {
    let (c, t) = e
        .grant_external_access(ExternalGrant {
            name: "Contributor".into(),
            whole_workspace: project.is_none(),
            project_id: project,
            workspace_memory: false,
            can_propose: propose,
            model: "Declared model".into(),
            provider: "Declared provider".into(),
        })
        .unwrap();
    (c.id, t)
}
fn origin(e: &Engine, project: Option<&str>) -> AgentRun {
    e.db().writer().execute("INSERT INTO agent_runs(id,goal,project_id,workspace_memory,model_id,model_name,provider_id,provider_name,status,summary,handoff,created_at,updated_at,thread_id,allow_edits) VALUES('origin','Preserve the unresolved release question.',?1,0,'fixture-model','Native fixture model','fixture','Fixture provider','completed','The date remains unconfirmed.','Inspect evidence before deciding.',1000,1000,'origin',1)",[project]).unwrap();
    e.agent_run("origin").unwrap()
}
fn source_page(e: &Engine) -> sthira_core::Artifact {
    e.create_artifact(
        ArtifactType::Note,
        Authorship::User,
        Some("Open question"),
        "Original uncertainty.",
        None,
    )
    .unwrap()
}
fn draft(run: &AgentRun, recipient: &str, page: &str) -> HandoffDraft {
    HandoffDraft {
        request_id: "save-1".into(),
        run_id: run.id.clone(),
        expected_run_revision: run.revision,
        recipient_connection_id: recipient.into(),
        title: "Release evidence handoff".into(),
        next_steps: "Inspect the unresolved source, then propose a revision.".into(),
        open_questions: "When is evidence sufficient?".into(),
        artifact_ids: vec![page.into()],
        memory_ids: vec![],
    }
}
fn contribution(id: &str, revision: i64, page: &str) -> ExternalContribution {
    ExternalContribution {
        handoff_id: id.into(),
        expected_revision: revision,
        request_id: "contribution-1".into(),
        summary: "Prepared a revision; the release date is still unknown.".into(),
        next_steps: "Review the proposed wording.".into(),
        open_questions: "The evidence remains incomplete.".into(),
        actions: vec![
            AgentAction::ReadPage {
                artifact_id: page.into(),
            },
            AgentAction::ProposeRefine {
                artifact_id: page.into(),
                content_md: "The date is unconfirmed; evidence still needs review.".into(),
            },
        ],
    }
}
fn continuation(v: &HandoffView, key: &str) -> ContinueHandoff {
    ContinueHandoff {
        id: v.handoff.id.clone(),
        request_id: key.into(),
        expected_context_version: v.context_version.clone(),
        text: "Continue from the reviewed contribution using current sources.".into(),
        model_id: "fixture-model".into(),
    }
}
#[tokio::test]
async fn native_external_native_roundtrip_keeps_memory_identity_review_and_current_context_after_reopen(
) {
    let tmp = tempfile::tempdir().unwrap();
    let e = open(tmp.path());
    let p = e.create_project("Release", None).unwrap();
    let page = source_page(&e);
    e.attach_artifact(&p.id, &page.id, "user_explicit", None)
        .unwrap();
    let MemoryResult::Entry(memory) = e
        .execute_memory_action(
            &MemoryAccess::owner(),
            MemoryAction::Create {
                draft: MemoryDraft {
                    scope: MemoryScope::Project(p.id.clone()),
                    title: "Unresolved evidence".into(),
                    body: "Do not assume a release date.".into(),
                    source_ids: vec![],
                    pinned: true,
                },
            },
        )
        .unwrap()
    else {
        panic!()
    };
    let finish = json!({"type":"finish","summary":"The release date remains uncertain.","handoff":"Read evidence before deciding."});
    let (url, server) = fixture(vec![
        json!({"type":"read_page","artifact_id":page.id}),
        finish.clone(),
        json!({"type":"read_page","artifact_id":page.id}),
        finish,
    ])
    .await;
    configure(&e, &url);
    let first = e.create_agent_run(goal(Some(&p.id))).unwrap();
    let first = e
        .execute_agent_run(&first.id, &CancellationToken::new())
        .await
        .unwrap();
    let (recipient, token) = external_grant(&e, Some(p.id.clone()), true);
    let mut d = draft(&first, &recipient, &page.id);
    d.memory_ids = vec![memory.id.clone()];
    let saved = e.save_agent_handoff(d).unwrap();
    let c = contribution(&saved.handoff.id, saved.handoff.revision, &page.id);
    let external = e
        .external_handoff(
            &token,
            ExternalHandoffAction::Contribute {
                contribution: c.clone(),
            },
        )
        .unwrap();
    let run: AgentRun = serde_json::from_value(external["run"].clone()).unwrap();
    assert_eq!(run.parent_run_id.as_deref(), Some(first.id.as_str()));
    assert_eq!(run.thread_id, first.thread_id);
    assert_eq!(
        e.get_artifact(&page.id).unwrap().content_md,
        page.content_md
    );
    assert_eq!(run.steps[1].state, "pending");
    assert_eq!(
        e.external_handoff(
            &token,
            ExternalHandoffAction::Contribute { contribution: c }
        )
        .unwrap()["replayed"],
        true
    );
    e.review_agent_step(&run.id, &run.steps[1].id, true)
        .unwrap();
    let packet = e.agent_handoff(&saved.handoff.id).unwrap();
    assert!(packet.sources_changed && packet.sources_available);
    assert_eq!(
        packet.contributions[0]["review"]["proposals"][0]["state"],
        "applied"
    );
    let request = continuation(&packet, "continue-1");
    let native = e.continue_agent_handoff(request.clone()).unwrap();
    assert_eq!(e.continue_agent_handoff(request).unwrap().id, native.id);
    let finished = e
        .execute_agent_run(&native.id, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(finished.status, "completed");
    let requests = server.await.unwrap();
    let text = requests[2]["messages"][1]["content"].as_str().unwrap();
    let prompt: Value = serde_json::from_str(text).unwrap();
    assert_eq!(prompt["saved_handoff"]["handoff_id"], saved.handoff.id);
    assert!(prompt["saved_handoff"].to_string().contains(&memory.id));
    assert_eq!(
        prompt["saved_handoff"]["contributions"][0]["current_review"]["proposals"][0]["state"],
        "applied"
    );
    assert_eq!(
        prompt["saved_handoff"]["current_sources"]["sources"][0]["version"],
        2
    );
    assert_eq!(prompt["saved_handoff"]["content_is_untrusted"], true);
    assert_eq!(
        e.get_artifact(&page.id).unwrap().authorship,
        Authorship::User
    );
    let handoff_id = saved.handoff.id;
    drop(e);
    let e = open(tmp.path());
    assert_eq!(e.agent_handoff(&handoff_id).unwrap().contributions.len(), 1);
    assert_eq!(
        e.list_agent_handoffs(Some(&native.id)).unwrap()[0].id,
        handoff_id
    );
    assert_eq!(e.get_artifact(&page.id).unwrap().id, page.id);
}
#[test]
fn handoff_reads_do_not_duplicate_artifacts_and_wrong_recipients_scopes_or_revocations_fail() {
    let tmp = tempfile::tempdir().unwrap();
    let e = open(tmp.path());
    let p = e.create_project("Scoped", None).unwrap();
    let page = source_page(&e);
    e.attach_artifact(&p.id, &page.id, "user_explicit", None)
        .unwrap();
    let run = origin(&e, Some(&p.id));
    let (id, token) = external_grant(&e, Some(p.id.clone()), true);
    let saved = e.save_agent_handoff(draft(&run, &id, &page.id)).unwrap();
    let (_, other) = external_grant(&e, Some(p.id.clone()), true);
    let before = count(&e, "artifacts");
    for _ in 0..2 {
        assert!(e
            .external_handoff(
                &token,
                ExternalHandoffAction::Read {
                    id: saved.handoff.id.clone()
                }
            )
            .is_ok());
    }
    assert_eq!(count(&e, "artifacts"), before);
    assert!(e
        .external_handoff(
            &other,
            ExternalHandoffAction::Read {
                id: saved.handoff.id.clone()
            }
        )
        .is_err());
    e.detach_artifact(&p.id, &page.id).unwrap();
    assert!(e
        .external_handoff(
            &token,
            ExternalHandoffAction::Read {
                id: saved.handoff.id.clone()
            }
        )
        .is_err());
    assert!(e
        .external_handoff(&token, ExternalHandoffAction::List)
        .unwrap()["handoffs"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(
        !e.agent_handoff(&saved.handoff.id)
            .unwrap()
            .sources_available
    );
    e.revoke_external_access(&id).unwrap();
    assert!(e
        .external_handoff(&token, ExternalHandoffAction::List)
        .is_err());
}
#[test]
fn stale_source_change_and_restore_blocks_external_contribution_and_stale_owner_preview() {
    let tmp = tempfile::tempdir().unwrap();
    let e = open(tmp.path());
    let page = source_page(&e);
    let run = origin(&e, None);
    let (id, token) = external_grant(&e, None, true);
    let saved = e.save_agent_handoff(draft(&run, &id, &page.id)).unwrap();
    e.db()
        .writer()
        .execute(
            "UPDATE artifacts SET version=version+2 WHERE id=?1",
            [&page.id],
        )
        .unwrap();
    let packet = e.agent_handoff(&saved.handoff.id).unwrap();
    assert!(packet.sources_changed);
    assert_ne!(packet.context_version, saved.context_version);
    assert!(e
        .external_handoff(
            &token,
            ExternalHandoffAction::Contribute {
                contribution: contribution(&saved.handoff.id, saved.handoff.revision, &page.id)
            }
        )
        .is_err());
    assert_eq!(count(&e, "agent_handoff_contributions"), 0);
}
#[test]
fn contribution_conflicts_close_and_readonly_grants_cannot_replay_or_self_approve() {
    let tmp = tempfile::tempdir().unwrap();
    let e = open(tmp.path());
    let page = source_page(&e);
    let run = origin(&e, None);
    let (id, token) = external_grant(&e, None, true);
    let saved = e.save_agent_handoff(draft(&run, &id, &page.id)).unwrap();
    let c = contribution(&saved.handoff.id, saved.handoff.revision, &page.id);
    let result = e
        .external_handoff(
            &token,
            ExternalHandoffAction::Contribute {
                contribution: c.clone(),
            },
        )
        .unwrap();
    let external: AgentRun = serde_json::from_value(result["run"].clone()).unwrap();
    let mut different = c;
    different.request_id = "competing".into();
    assert!(e
        .external_handoff(
            &token,
            ExternalHandoffAction::Contribute {
                contribution: different
            }
        )
        .is_err());
    assert_eq!(count(&e, "agent_handoff_contributions"), 1);
    let current = e.agent_handoff(&saved.handoff.id).unwrap();
    e.close_agent_handoff(&saved.handoff.id, current.handoff.revision)
        .unwrap();
    assert!(e
        .external_handoff(
            &token,
            ExternalHandoffAction::Read {
                id: saved.handoff.id.clone()
            }
        )
        .is_err());
    assert!(e
        .review_agent_step(&external.id, &external.steps[1].id, true)
        .is_err());
    assert!(e
        .review_agent_step(&external.id, &external.steps[1].id, false)
        .is_ok());
    let (readonly, readonly_token) = external_grant(&e, None, false);
    let updated = e.agent_run(&run.id).unwrap();
    let mut d = draft(&updated, &readonly, &page.id);
    d.request_id = "readonly-save".into();
    let ro = e.save_agent_handoff(d).unwrap();
    assert!(e
        .external_handoff(
            &readonly_token,
            ExternalHandoffAction::Read {
                id: ro.handoff.id.clone()
            }
        )
        .is_ok());
    assert!(e
        .external_handoff(
            &readonly_token,
            ExternalHandoffAction::Contribute {
                contribution: contribution(&ro.handoff.id, ro.handoff.revision, &page.id)
            }
        )
        .is_err());
}
#[tokio::test]
async fn native_continuation_rechecks_preview_and_queued_sources_without_dispatching_on_conflict() {
    let tmp = tempfile::tempdir().unwrap();
    let e = open(tmp.path());
    configure(&e, "http://127.0.0.1:1/v1");
    let page = source_page(&e);
    let run = origin(&e, None);
    let (id, _) = external_grant(&e, None, true);
    let saved = e.save_agent_handoff(draft(&run, &id, &page.id)).unwrap();
    e.db()
        .writer()
        .execute(
            "UPDATE artifacts SET version=version+1 WHERE id=?1",
            [&page.id],
        )
        .unwrap();
    assert!(e
        .continue_agent_handoff(continuation(&saved, "stale"))
        .is_err());
    assert_eq!(count(&e, "agent_handoff_continuations"), 0);
    let current = e.agent_handoff(&saved.handoff.id).unwrap();
    let native = e
        .continue_agent_handoff(continuation(&current, "queued"))
        .unwrap();
    e.db()
        .writer()
        .execute(
            "UPDATE artifacts SET version=version+1 WHERE id=?1",
            [&page.id],
        )
        .unwrap();
    let failed = e
        .execute_agent_run(&native.id, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(failed.status, "failed");
    assert!(failed.error.unwrap().contains("Handoff sources changed"));
    assert!(e.agent_run(&native.id).unwrap().steps.is_empty());
}
#[test]
fn source_and_memory_scope_grants_cannot_be_expanded_and_handoffs_are_retry_safe() {
    let tmp = tempfile::tempdir().unwrap();
    let e = open(tmp.path());
    let p = e.create_project("One", None).unwrap();
    let q = e.create_project("Other", None).unwrap();
    let page = source_page(&e);
    e.attach_artifact(&p.id, &page.id, "user_explicit", None)
        .unwrap();
    let run = origin(&e, Some(&p.id));
    let (wrong, _) = external_grant(&e, Some(q.id.clone()), true);
    assert!(e.save_agent_handoff(draft(&run, &wrong, &page.id)).is_err());
    let (id, _) = external_grant(&e, Some(p.id), true);
    let d = draft(&run, &id, &page.id);
    let saved = e.save_agent_handoff(d.clone()).unwrap();
    assert_eq!(
        e.save_agent_handoff(d).unwrap().handoff.id,
        saved.handoff.id
    );
    assert_eq!(count(&e, "agent_handoffs"), 1);
    let extra = source_page(&e);
    let current = e.agent_run(&run.id).unwrap();
    let mut d = draft(&current, &id, &extra.id);
    d.request_id = "out-of-scope".into();
    assert!(e.save_agent_handoff(d).is_err());
}
#[test]
fn failed_handoff_and_contribution_receipts_roll_back_links_revisions_and_runs() {
    let tmp = tempfile::tempdir().unwrap();
    let e = open(tmp.path());
    let page = source_page(&e);
    let run = origin(&e, None);
    let (id, token) = external_grant(&e, None, true);
    e.db().writer().execute_batch("CREATE TRIGGER fail_handoff BEFORE UPDATE OF revision ON agent_runs BEGIN SELECT RAISE(ABORT,'receipt fixture'); END").unwrap();
    assert!(e.save_agent_handoff(draft(&run, &id, &page.id)).is_err());
    assert_eq!(count(&e, "agent_handoffs"), 0);
    e.db()
        .writer()
        .execute_batch("DROP TRIGGER fail_handoff")
        .unwrap();
    let saved = e.save_agent_handoff(draft(&run, &id, &page.id)).unwrap();
    let before = count(&e, "agent_runs");
    e.db().writer().execute_batch("CREATE TRIGGER fail_contribution BEFORE INSERT ON agent_handoff_contributions BEGIN SELECT RAISE(ABORT,'receipt fixture'); END").unwrap();
    assert!(e
        .external_handoff(
            &token,
            ExternalHandoffAction::Contribute {
                contribution: contribution(&saved.handoff.id, saved.handoff.revision, &page.id)
            }
        )
        .is_err());
    assert_eq!(count(&e, "agent_runs"), before);
    assert_eq!(count(&e, "agent_handoff_contributions"), 0);
    assert_eq!(
        e.agent_handoff(&saved.handoff.id).unwrap().handoff.revision,
        saved.handoff.revision
    );
}
#[test]
fn v25_upgrade_preserves_external_receipts_and_handoff_context() {
    let tmp = tempfile::tempdir().unwrap();
    let e = open(tmp.path());
    let page = source_page(&e);
    let run = origin(&e, None);
    let (id, token) = external_grant(&e, None, true);
    let batch = ExternalBatch {
        request_id: "legacy".into(),
        goal: "Inspect project names.".into(),
        actions: vec![AgentAction::SearchProjects { query: "".into() }],
    };
    e.external_workspace(&token, batch.clone()).unwrap();
    let current = e.agent_run(&run.id).unwrap();
    let saved = e
        .save_agent_handoff(draft(&current, &id, &page.id))
        .unwrap();
    e.db()
        .writer()
        .execute_batch("PRAGMA user_version=25")
        .unwrap();
    drop(e);
    let e = open(tmp.path());
    assert_eq!(
        e.external_workspace(&token, batch).unwrap()["replayed"],
        true
    );
    assert_eq!(
        e.agent_handoff(&saved.handoff.id).unwrap().handoff.revision,
        1
    );
    assert_eq!(
        e.db()
            .reader()
            .unwrap()
            .query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        27
    );
}
#[test]
fn selected_memory_revisions_and_failed_native_receipts_preserve_the_reviewed_snapshot() {
    let tmp = tempfile::tempdir().unwrap();
    let e = open(tmp.path());
    configure(&e, "http://127.0.0.1:1/v1");
    let p = e.create_project("Memory scope", None).unwrap();
    let page = source_page(&e);
    e.attach_artifact(&p.id, &page.id, "user_explicit", None)
        .unwrap();
    let run = origin(&e, Some(&p.id));
    let (id, token) = external_grant(&e, Some(p.id.clone()), true);
    let MemoryResult::Entry(memory) = e
        .execute_memory_action(
            &MemoryAccess::owner(),
            MemoryAction::Create {
                draft: MemoryDraft {
                    scope: MemoryScope::Project(p.id),
                    title: "Unresolved question".into(),
                    body: "Keep uncertainty.".into(),
                    source_ids: vec![],
                    pinned: false,
                },
            },
        )
        .unwrap()
    else {
        panic!()
    };
    let mut d = draft(&run, &id, &page.id);
    d.memory_ids = vec![memory.id.clone()];
    let saved = e.save_agent_handoff(d).unwrap();
    e.db().writer().execute_batch("CREATE TRIGGER fail_continuation BEFORE INSERT ON agent_handoff_continuations BEGIN SELECT RAISE(ABORT,'continuation receipt fixture'); END").unwrap();
    let before = count(&e, "agent_runs");
    assert!(e
        .continue_agent_handoff(continuation(&saved, "failed-native"))
        .is_err());
    assert_eq!(count(&e, "agent_runs"), before);
    assert_eq!(
        e.agent_handoff(&saved.handoff.id).unwrap().handoff.revision,
        saved.handoff.revision
    );
    e.db()
        .writer()
        .execute_batch("DROP TRIGGER fail_continuation")
        .unwrap();
    e.execute_memory_action(
        &MemoryAccess::owner(),
        MemoryAction::Update {
            id: memory.id,
            expected_version: memory.version,
            title: "Unresolved question".into(),
            body: "Additional uncertainty remains.".into(),
            pinned: false,
        },
    )
    .unwrap();
    assert!(e.agent_handoff(&saved.handoff.id).unwrap().sources_changed);
    assert!(e
        .external_handoff(
            &token,
            ExternalHandoffAction::Contribute {
                contribution: contribution(&saved.handoff.id, saved.handoff.revision, &page.id)
            }
        )
        .is_err());
}
#[test]
fn concurrent_external_contributors_cannot_consume_one_handoff_revision_twice() {
    let tmp = tempfile::tempdir().unwrap();
    let e = Arc::new(open(tmp.path()));
    let page = source_page(&e);
    let run = origin(&e, None);
    let (id, token) = external_grant(&e, None, true);
    let saved = e.save_agent_handoff(draft(&run, &id, &page.id)).unwrap();
    let c = contribution(&saved.handoff.id, saved.handoff.revision, &page.id);
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let handles = (0..2)
        .map(|n| {
            let e = e.clone();
            let token = token.clone();
            let barrier = barrier.clone();
            let mut c = c.clone();
            c.request_id = format!("concurrent-{n}");
            std::thread::spawn(move || {
                barrier.wait();
                e.external_handoff(
                    &token,
                    ExternalHandoffAction::Contribute { contribution: c },
                )
                .is_ok()
            })
        })
        .collect::<Vec<_>>();
    let accepted = handles
        .into_iter()
        .map(|h| h.join().unwrap())
        .filter(|ok| *ok)
        .count();
    assert_eq!(accepted, 1);
    assert_eq!(count(&e, "agent_handoff_contributions"), 1);
    assert_eq!(count(&e, "agent_runs"), 2);
}
#[test]
fn migrated_sources_work_with_memory_external_proposals_and_handoff_conflicts() {
    use sthira_core::migration::{ImportDraft, ImportFile};
    let tmp = tempfile::tempdir().unwrap();
    let e = open(tmp.path());
    let p = e.create_project("Imported research", None).unwrap();
    let preview = e
        .preview_migration(ImportDraft {
            request_id: "migration".into(),
            source: "obsidian".into(),
            collection: "research".into(),
            project_id: Some(p.id.clone()),
            files: vec![ImportFile {
                path: "Question.md".into(),
                content: Some("# Open question\nThe release date is uncertain.".into()),
                problem: None,
            }],
        })
        .unwrap();
    let imported = e
        .apply_migration(&preview.id, preview.revision, vec!["Question.md".into()])
        .unwrap();
    let page = &imported.plan.items[0].artifact_id;
    let MemoryResult::Entry(memory) = e
        .execute_memory_action(
            &MemoryAccess::owner(),
            MemoryAction::Create {
                draft: MemoryDraft {
                    scope: MemoryScope::Project(p.id.clone()),
                    title: "Imported uncertainty".into(),
                    body: "The date remains unconfirmed.".into(),
                    source_ids: vec![page.clone()],
                    pinned: true,
                },
            },
        )
        .unwrap()
    else {
        panic!()
    };
    let native = origin(&e, Some(&p.id));
    let (recipient, token) = external_grant(&e, Some(p.id), true);
    let mut d = draft(&native, &recipient, page);
    d.memory_ids = vec![memory.id];
    let handoff = e.save_agent_handoff(d).unwrap();
    let result = e
        .external_handoff(
            &token,
            ExternalHandoffAction::Contribute {
                contribution: contribution(&handoff.handoff.id, handoff.handoff.revision, page),
            },
        )
        .unwrap();
    let run: AgentRun = serde_json::from_value(result["run"].clone()).unwrap();
    e.review_agent_step(&run.id, &run.steps[1].id, true)
        .unwrap();
    assert_eq!(
        e.get_artifact(page).unwrap().authorship,
        Authorship::External
    );
    assert!(
        !e.agent_handoff(&handoff.handoff.id)
            .unwrap()
            .sources_available
    );
    assert!(e
        .rollback_migration(&imported.id, imported.revision)
        .is_err());
}
