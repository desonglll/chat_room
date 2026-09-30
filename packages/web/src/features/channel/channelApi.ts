/** The app-wide channel client: the shared `ApiClient` with the live session's token. */
import { authStore, createChannelApi, selectToken } from '@tg/core'
import { apiClient } from '../../app/client'

export const channelApi = createChannelApi(apiClient, () => selectToken(authStore.getState()) || null)
