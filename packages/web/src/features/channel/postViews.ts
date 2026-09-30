/** The app-wide view reporter: batches on-screen posts into `POST /message-views`. */
import { channelApi } from './channelApi'
import { channelStore } from './channelStore'
import { createViewReporter } from './viewReporter'

export const postViewReporter = createViewReporter({
  report: (chatId, ids) => channelApi.reportViews(chatId, ids),
  onCounts: (counts) => channelStore.getState().mergeViews(counts),
  setTimer: (callback, delayMs) => setTimeout(callback, delayMs),
})
