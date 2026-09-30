/**
 * TG-104 test fixture: one fake cloud-draft server shared by any number of "devices",
 * each a real `createChatSession` (features/chat) over a fake socket + fake clock, with
 * a `ComposerController` on top. A PUT on one device is pushed to every other device
 * as a `draft_updated` frame — what the real server does (TG-008). Not a test file.
 */
import type { ChatDraft, ClientFrame, DraftsApi } from '@tg/core'
import {
  createChatActionSender,
  createChatListStore,
  createComposerStore,
  createMessageStore,
  createPresenceStore,
} from '@tg/core'
import { createChatSession } from '../../chat/chatSession'
import { AUTH_OK, FakeClock, FakeSocket, ME, settle } from '../../../../test/chatSessionHarness'
import { createComposerController } from '../composerController'

export { FakeClock, FakeSocket, ME, settle }

export class FakeDraftServer {
  drafts = new Map<string, ChatDraft>()
  puts: Array<{ chatId: string; text: string; reply: string | null }> = []
  private seq = 0
  private listeners: Array<{ chatId: string; socket: FakeSocket }> = []

  api(): DraftsApi {
    return {
      get: async (chatId) => this.drafts.get(chatId) ?? null,
      put: async (chatId, draft) => {
        this.seq += 1
        const stored: ChatDraft = {
          user_id: ME,
          text: draft.text,
          reply_to_message_id: draft.reply_to_message_id ?? null,
          topic_id: draft.topic_id ?? null,
          updated_at: `2026-10-01T10:00:${String(this.seq).padStart(2, '0')}Z`,
        }
        this.puts.push({ chatId, text: stored.text, reply: stored.reply_to_message_id })
        if (stored.text === '' && stored.reply_to_message_id === null) this.drafts.delete(chatId)
        else this.drafts.set(chatId, stored)
        for (const listener of this.listeners) {
          if (listener.chatId === chatId && !listener.socket.closed) {
            listener.socket.receive({ type: 'draft_updated', ...stored })
          }
        }
        return stored
      },
    }
  }

  attach(chatId: string, socket: FakeSocket): void {
    this.listeners.push({ chatId, socket })
  }
}

export function createDevice(server: FakeDraftServer, clock = new FakeClock()) {
  const stores = {
    message: createMessageStore(),
    presence: createPresenceStore(),
    composer: createComposerStore(),
    chatList: createChatListStore(),
  }
  const forwards: Array<{ ids: readonly string[]; target: string }> = []
  const sockets: FakeSocket[] = []

  async function open(chatId: string) {
    let socket: FakeSocket | null = null
    const session = createChatSession({
      chatId,
      token: 't',
      currentUserId: ME,
      socketUrl: `ws://test/ws/${chatId}`,
      createSocket: () => {
        socket = new FakeSocket()
        sockets.push(socket)
        server.attach(chatId, socket)
        return socket
      },
      clock,
      client: { json: async () => [], request: async () => new Response() } as never,
      draftsApi: server.api(),
      stores,
    })
    session.start()
    await settle()
    const live = socket! as FakeSocket
    live.open()
    live.receive(AUTH_OK)
    live.receive({ type: 'history_complete' })
    await settle()
    // The integration patch adds `sendFrame` to the session; the harness supplies it directly.
    const framesViaComposer: ClientFrame[] = []
    const sendFrame = (frame: ClientFrame) => {
      framesViaComposer.push(frame)
      live.send(JSON.stringify(frame))
      return true
    }
    const actions = createChatActionSender({ clock, send: (_id, frame) => sendFrame(frame) })
    const controller = createComposerController({
      chatId,
      session: { sendMessage: session.sendMessage, setDraftText: session.setDraftText, sendFrame },
      composer: stores.composer,
      messages: stores.message,
      actions,
      forward: async (ids, target) => {
        forwards.push({ ids, target })
      },
    })
    return { session, socket: live, controller, actions, framesViaComposer }
  }

  return { stores, clock, open, forwards, sockets }
}
