-- Rebuild both tables together so existing conversation history and edit decisions survive.
ALTER TABLE document_conversation_turns RENAME TO document_conversation_turns_v13;
ALTER TABLE document_conversations RENAME TO document_conversations_v13;
