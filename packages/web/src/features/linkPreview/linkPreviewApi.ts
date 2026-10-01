/** The app-wide TG-408 client. */
import { authStore, createLinkPreviewApi, selectToken } from '@tg/core'
import { apiClient } from '../../app/client'

export const linkPreviewApi = createLinkPreviewApi(apiClient, () => selectToken(authStore.getState()) || null)
