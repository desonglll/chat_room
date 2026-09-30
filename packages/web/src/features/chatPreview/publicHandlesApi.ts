import { authStore, createPublicHandlesApi, selectToken } from '@tg/core'
import { apiClient } from '../../app/client'

export const publicHandlesApi = createPublicHandlesApi(apiClient, () => selectToken(authStore.getState()) || null)
