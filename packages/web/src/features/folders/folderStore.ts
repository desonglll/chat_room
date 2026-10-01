/**
 * TG-501: the viewer's folders and which one the chat list shows (`null` = «全部»). Loaded once
 * per session; the settings page writes through the API and refreshes this store.
 */
import { createStore } from 'zustand/vanilla'
import type { ChatFolder } from '@tg/core'
import { authStore, createFoldersApi, selectToken } from '@tg/core'
import { apiClient } from '../../app/client'

export const foldersApi = createFoldersApi(apiClient, () => selectToken(authStore.getState()) || null)

export interface FolderState {
  folders: ChatFolder[]
  activeId: string | null
  loaded: boolean
}

const EMPTY: FolderState = { folders: [], activeId: null, loaded: false }

export const folderStore = createStore<FolderState>()(() => EMPTY)

// TG-1204: folders are per account. Without this, signing out and into another account in the
// same tab kept the previous account's folder tabs (`loaded` stayed true, so nothing refetched).
authStore.subscribe((next, previous) => {
  if (selectToken(next) !== selectToken(previous)) folderStore.setState(EMPTY)
})

export function loadFolders(api = foldersApi): Promise<void> {
  return api.list().then(
    (folders) =>
      folderStore.setState((state) => ({
        folders,
        loaded: true,
        // A deleted active folder falls back to «全部».
        activeId: state.activeId && folders.some((folder) => folder.id === state.activeId) ? state.activeId : null,
      })),
    () => folderStore.setState({ loaded: true }),
  )
}

export function selectFolder(id: string | null): void {
  folderStore.setState({ activeId: id })
}
