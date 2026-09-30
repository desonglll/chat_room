import { authStore, createFavoritesApi, selectToken } from '@tg/core'
import { apiClient } from '../../app/client'

export const favoritesApi = createFavoritesApi(apiClient, () => selectToken(authStore.getState()) || null)
