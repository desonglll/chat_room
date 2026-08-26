DROP TRIGGER IF EXISTS messages_graph_after_insert ON messages;
DROP TRIGGER IF EXISTS messages_graph_after_update ON messages;
DROP TRIGGER IF EXISTS messages_graph_after_delete ON messages;
DROP FUNCTION IF EXISTS enqueue_message_graph_change();
DROP TABLE IF EXISTS message_graph_outbox;
