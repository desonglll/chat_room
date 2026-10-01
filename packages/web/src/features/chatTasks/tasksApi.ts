/** TG-706: the app-wide tasks / audit client. */
import { authStore, createTasksApi, selectToken } from '@tg/core'
import { apiClient } from '../../app/client'

export const tasksApi = createTasksApi(apiClient, () => selectToken(authStore.getState()) || null)
