/**
 * TG-106 public surface. The shell mounts `<InfoPane />` (features/shell/InfoPane.tsx),
 * which renders `ChatInfoPanel` for `uiStore.activeChatId`. See docs/devlog/TG-106.md
 * "Frozen interface".
 */
export { ChatInfoPanel } from './ChatInfoPanel'
export type { ChatInfoPanelProps } from './ChatInfoPanel'
export { infoVariant, selectInfoHeader } from './chatInfoModel'
export type { InfoHeaderModel, InfoVariant } from './chatInfoModel'
export type { SharedTabId } from './sharedSources'
export { holdMessageListAnchor, watchPanelAnchor } from './panelAnchor'
