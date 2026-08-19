-- Content-addressed storage: dedupe identical uploads and mark (not delete)
-- attachments once their last live reference is gone. `storage_key` is NULL
-- for attachments that predate this migration — their physical files stay
-- exactly where they are (keyed by `id`), never moved or merged.
ALTER TABLE attachments ADD COLUMN content_hash TEXT;
ALTER TABLE attachments ADD COLUMN storage_key TEXT;
ALTER TABLE attachments ADD COLUMN orphaned_at TIMESTAMPTZ;

CREATE INDEX attachments_content_hash_idx ON attachments (content_hash);
CREATE INDEX attachments_storage_key_idx ON attachments (storage_key);
