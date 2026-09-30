/** The app-wide privacy client: the shared `ApiClient` with the live session's token. */
import { authStore, createPrivacyApi, selectToken } from '@tg/core'
import { apiClient } from '../../../app/client'

export const privacyApi = createPrivacyApi(apiClient, () => selectToken(authStore.getState()) || null)
