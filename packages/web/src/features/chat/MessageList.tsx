/**
 * Superseded by the virtual list in `features/messageList` (TG-101). Kept only as a
 * re-export because `test/markup.test.tsx` (outside TG-101's paths) still imports this
 * path; the integration lead can repoint that import and delete this file.
 */
export { MessageList } from '../messageList/MessageList'
