-- The browser computes this before sending chunks. It lets the server verify
-- resumed content and reuse a healthy object previously uploaded by this user.
ALTER TABLE attachment_uploads ADD COLUMN content_hash TEXT;

