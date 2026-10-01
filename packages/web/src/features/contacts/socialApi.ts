/** TG-702: the app-wide contacts client. */
import { authStore, createSocialApi, selectToken } from '@tg/core'
import { apiClient } from '../../app/client'

export const socialApi = createSocialApi(apiClient, () => selectToken(authStore.getState()) || null)
