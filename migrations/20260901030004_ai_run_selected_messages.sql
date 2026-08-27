CREATE TABLE ai_run_selected_messages (
    run_id TEXT NOT NULL REFERENCES ai_runs (id) ON DELETE CASCADE,
    message_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    PRIMARY KEY (run_id, message_id),
    UNIQUE (run_id, ordinal)
);

CREATE INDEX ai_run_selected_messages_message_idx
    ON ai_run_selected_messages (message_id);
