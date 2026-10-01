/** TG-705: the app-wide admin client and whether the viewer is a system administrator. */
import { createStore } from 'zustand/vanilla'
import { authStore, createAdminApi, selectToken } from '@tg/core'
import { apiClient } from '../../app/client'

export const adminApi = createAdminApi(apiClient, () => selectToken(authStore.getState()) || null)

export const adminStore = createStore<{ isAdmin: boolean; checked: boolean }>()(() => ({
  isAdmin: false,
  checked: false,
}))

/** Asks once per session; the console entry appears only for administrators. */
export function checkAdmin(): void {
  if (adminStore.getState().checked) return
  adminStore.setState({ checked: true })
  adminApi.isAdmin().then(
    (isAdmin) => adminStore.setState({ isAdmin }),
    () => adminStore.setState({ checked: false }),
  )
}
