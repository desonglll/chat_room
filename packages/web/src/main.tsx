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
