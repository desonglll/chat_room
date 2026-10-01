import '@tg/ui/styles.css'
import './styles/index.css'
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { authStore, settingsStore } from '@tg/core'
import { App } from './App'
import './features/customEmoji/register'
import { apiClient } from './app/client'
import { browserStorage } from './app/platform'
import { hydrateSession, revalidateSession } from './app/session'
import { bindTheme } from './app/theme'
// TG-406: registers the poll bubble body with the message content registry.
import './features/poll/register'
// TG-303: registers the sticker message body with the message content registry.
import './features/sticker/register'
// TG-401: registers the voice bubble body with the message content registry.
import './features/voice/register'
// TG-402: registers the round video bubble body with the message content registry.
import './features/videoNote/register'
// TG-305: GIF bubble body, the media panel's GIF tab, and the "保存 GIF" menu row.
import './features/gif/register'
// TG-509: 设置 › 数据与存储.
import './features/settings/storage/register'
// TG-501: 设置 › 聊天文件夹.
import './features/folders/register'
// TG-508: 设置 › 通知与声音.
import './features/settings/notifications/register'
// TG-410: contact card bubbles and «翻译».
import './features/contact/register'
// TG-408: link cards.
import './features/linkPreview/register'
// TG-407: location bubbles.
import './features/location/register'
// TG-511: «头像» and «我的二维码» in settings.
import './features/profile/register'
// TG-503: «保存到收藏夹» in the message menu.
import './features/savedMessages/register'
// TG-403: the album mosaic body for collapsed album rows.
import './features/album/register'

// Boot order matters: settings → theme attribute → session → render. The session is
// restored synchronously so the router's first pass already knows whether `/` or
// `/login` applies; revalidation is fire-and-forget (a revoked token clears itself).
settingsStore.getState().hydrate(browserStorage)
bindTheme(settingsStore)
hydrateSession({ storage: browserStorage, store: authStore }, Date.now())
void revalidateSession({ client: apiClient, storage: browserStorage, store: authStore })

const container = document.getElementById('root')
if (!container) throw new Error('index.html is missing the #root mount point')

createRoot(container).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
