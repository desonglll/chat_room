/** The app-wide forum topics client: the shared `ApiClient` with the live session's token. */
import { authStore, createTopicsApi, selectToken } from '@tg/core'
import { apiClient } from '../../app/client'

export const topicsApi = createTopicsApi(apiClient, () => selectToken(authStore.getState()) || null)
