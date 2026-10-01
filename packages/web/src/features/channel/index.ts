/**
 * TG-202 public surface: channels. Mount points are listed in docs/devlog/TG-202.md
 * "Integration patch list".
 */

export { channelApi } from './channelApi'
export { ChannelFooter, ChannelFooterView } from './ChannelFooter'
export type { ChannelFooterViewProps } from './ChannelFooter'
export { ChannelPostMeta } from './ChannelPostMeta'
export { ChannelSubtitle } from './ChannelSubtitle'
export { CreateChannelDialog, createChannelError } from './CreateChannelDialog'
export type { CreateChannelDialogProps } from './CreateChannelDialog'
export { applyViewsFrame, channelStore, createChannelStore, effectiveViews } from './channelStore'
export type { ChannelStore, ChannelStoreState } from './channelStore'
export { canPublish, channelPostLabel, channelPostOf, formatViews, subscriberLine } from './channelModel'
export type { ChannelPostParts } from './channelModel'
export { createViewReporter } from './viewReporter'
export type { ViewReporter, ViewReporterOptions } from './viewReporter'
export { useChannelPublisher } from './useChannelAccess'
export { CommentsPanel } from './comments/CommentsPanel'
export { DiscussionLinkEditor } from './comments/DiscussionLinkEditor'
export { PostCommentsEntry } from './comments/PostCommentsEntry'
