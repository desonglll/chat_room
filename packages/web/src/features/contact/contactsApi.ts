import { authStore, createContactsApi, selectToken } from '@tg/core'
import { apiClient } from '../../app/client'

export const contactsApi = createContactsApi(apiClient, () => selectToken(authStore.getState()) || null)
