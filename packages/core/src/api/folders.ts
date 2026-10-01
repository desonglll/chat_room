/** TG-501 chat folders: the caller's folders and their rules (membership is evaluated client-side). */
import type { ChatFolder } from '../domain/chatFolders'
import type { ApiClient } from './http'
import { encodePathSegment } from './http'

export type ChatFolderWrite = Omit<ChatFolder, 'id'>

export interface FoldersApi {
  list(): Promise<ChatFolder[]>
  create(folder: ChatFolderWrite): Promise<ChatFolder>
  update(id: string, folder: ChatFolderWrite): Promise<ChatFolder>
  remove(id: string): Promise<void>
  reorder(ids: readonly string[]): Promise<ChatFolder[]>
}

export function createFoldersApi(client: ApiClient, token: () => string | null): FoldersApi {
  const auth = () => {
    const value = token()
    return value ? { token: value } : {}
  }
  return {
    list: () => client.json<ChatFolder[]>('GET', '/api/users/me/folders', auth()),
    create: (folder) => client.json<ChatFolder>('POST', '/api/users/me/folders', { ...auth(), body: folder }),
    update: (id, folder) =>
      client.json<ChatFolder>('PUT', `/api/users/me/folders/${encodePathSegment(id)}`, { ...auth(), body: folder }),
    remove: async (id) => {
      await client.request('DELETE', `/api/users/me/folders/${encodePathSegment(id)}`, auth())
    },
    reorder: (ids) =>
      client.json<ChatFolder[]>('PUT', '/api/users/me/folders/order', { ...auth(), body: { folder_ids: [...ids] } }),
  }
}
