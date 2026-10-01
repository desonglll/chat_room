/** The app-wide TG-203 discussion client: the shared `ApiClient` with the live session's token. */
import { authStore, createDiscussionApi, selectToken } from '@tg/core'
import { apiClient } from '../../../app/client'

export const discussionApi = createDiscussionApi(apiClient, () => selectToken(authStore.getState()) || null)
