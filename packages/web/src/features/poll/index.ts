/**
 * TG-406 public surface. `register.ts` plugs the bubble body in (side-effect import);
 * `PollCreateDialog` is what the composer's «投票» attach entry opens; `applyPollFrame` is
 * what the chat session calls for every `poll_updated` frame.
 */
export { PollCreateDialog } from './PollCreateDialog'
export type { PollCreateDialogProps } from './PollCreateDialog'
export { PollContent, PollBody } from './PollContent'
export { applyPollFrame, createPollStore, effectivePoll, pollStore } from './pollStore'
export type { PollStore, PollStoreState } from './pollStore'
export { describePoll, mergePollState, pollPercentages } from './pollView'
export type { PollOptionView, PollView } from './pollView'
