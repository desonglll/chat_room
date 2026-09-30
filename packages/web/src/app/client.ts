/**
 * The app-wide API singletons: one `ApiClient` (injected browser fetch, same-origin
 * base) and one `DraftsApi` whose bearer token tracks the live session. Feature code
 * imports these; only tests construct their own with fakes.
 */
import { authStore, createApiClient, createDraftsApi, selectToken } from '@tg/core'
import { browserFetch } from './platform'

export const apiClient = createApiClient({ fetchImpl: browserFetch, baseUrl: '' })

export const draftsApi = createDraftsApi(browserFetch, {
  token: () => selectToken(authStore.getState()) || null,
})
