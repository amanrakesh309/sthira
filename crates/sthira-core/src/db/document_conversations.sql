CREATE TABLE IF NOT EXISTS document_conversations (
    artifact_id TEXT PRIMARY KEY REFERENCES artifacts(id) ON DELETE CASCADE,
    mode TEXT NOT NULL CHECK(mode IN ('plan', 'research', 'document')),
    initial_prompt TEXT NOT NULL DEFAULT '',
    model_id TEXT,
    model_name TEXT
);
CREATE TABLE IF NOT EXISTS document_conversation_turns (
    id TEXT PRIMARY KEY,
    artifact_id TEXT NOT NULL REFERENCES document_conversations(artifact_id) ON DELETE CASCADE,
    user_text TEXT NOT NULL,
    reply TEXT,
    error TEXT,
    basis_content_md TEXT NOT NULL,
    proposed_content_md TEXT,
    proposal_state TEXT NOT NULL DEFAULT 'pending' CHECK(proposal_state IN ('pending', 'applied', 'dismissed', 'reverted')),
    change_set_id TEXT REFERENCES change_sets(id),
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_document_turns ON document_conversation_turns(artifact_id, created_at);
