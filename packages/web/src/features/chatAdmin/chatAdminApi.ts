/** The app-wide chat administration client: the shared `ApiClient` with the live session's token. */
import { authStore, createChatAdminApi, selectToken } from '@tg/core'
import { apiClient } from '../../app/client'

export const chatAdminApi = createChatAdminApi(apiClient, () => selectToken(authStore.getState()) || null)
