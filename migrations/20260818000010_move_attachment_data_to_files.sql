-- Attachment bytes are exported to the configured filesystem store before the
-- migrator reaches this file. SQLite retains only searchable metadata.
ALTER TABLE attachments DROP COLUMN data;
