/**
 * TG-203 channel comments: link a channel's discussion group, read a post's comment thread,
 * and comment. Comments are replies in the discussion group; reading needs only the channel,
 * writing needs membership of the group (join it with `join`).
 */
import type { StoredMessage } from '../types'
import { encodePathSegment, type ApiClient } from './http'

export interface CommentThread {
  discussion_chat_id: string
  discussion_message_id: string
  count: number
  can_comment: boolean
  /** Oldest first. */
  messages: StoredMessage[]
}

export interface PostCommentInput {
  content: string
  /** A comment in this thread to answer; the post itself when omitted. */
  reply_to?: string
  client_message_id?: string
}

export interface DiscussionApi {
  /** Link `chatId` (a group you administer) as the channel's discussion group; `null` unlinks. */
  link(channelId: string, chatId: string | null): Promise<{ channel_id: string; linked_chat_id: string | null }>
  thread(channelId: string, postId: string): Promise<CommentThread>
  comment(channelId: string, postId: string, input: PostCommentInput): Promise<StoredMessage>
  /** Join the discussion group (open groups admit at once). */
  join(groupId: string): Promise<void>
}

export function createDiscussionApi(client: ApiClient, token: () => string | null): DiscussionApi {
  const auth = () => {
    const value = token()
    return value ? { token: value } : {}
  }
  const chat = (chatId: string, suffix: string) => `/api/chats/${encodePathSegment(chatId)}${suffix}`
  const post = (channelId: string, postId: string) => chat(channelId, `/posts/${encodePathSegment(postId)}/comments`)
  return {
    link: (channelId, chatId) =>
      client.json('PUT', chat(channelId, '/discussion'), { ...auth(), body: { chat_id: chatId } }),
    thread: (channelId, postId) => client.json<CommentThread>('GET', post(channelId, postId), auth()),
    comment: (channelId, postId, input) =>
      client.json<StoredMessage>('POST', post(channelId, postId), { ...auth(), body: input }),
    join: async (groupId) => {
      await client.request('POST', chat(groupId, '/join-requests'), { ...auth(), body: {} })
    },
  }
}
