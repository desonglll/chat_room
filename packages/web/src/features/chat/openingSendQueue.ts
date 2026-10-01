/**
 * TG-1208: the sends a viewer makes while a chat is still opening — the socket is connecting,
 * or it is online but the history replay has not finished. Sending those frames at once
 * either failed (socket not online: the composer silently swallowed Enter) or put the
 * optimistic row ABOVE the replayed history (the replay rows were appended after it).
 *
 * The session parks them here and sends them, in order, once the replay completes. Only the
 * first opening of a session queues: a connection that drops after being ready keeps the
 * old contract (the send is refused and the row is marked failed).
 */
import type { ClientFrame } from '@tg/core'

interface QueuedSend {
  clientMessageId: string
  frame: ClientFrame
}

export interface OpeningSendQueue {
  /** True until the first `ready()`; while true, sends are parked rather than refused. */
  opening(): boolean
  push(clientMessageId: string, frame: ClientFrame): void
  /** Leave the opening phase and hand every parked send to `send`, oldest first. */
  ready(send: (send: QueuedSend) => void): void
  /** The session can never open (auth failed / stopped): give back every parked send. */
  abandon(fail: (clientMessageId: string) => void): void
}

export function createOpeningSendQueue(): OpeningSendQueue {
  let opening = true
  let parked: QueuedSend[] = []

  function drain(handle: (send: QueuedSend) => void): void {
    const sends = parked
    parked = []
    for (const send of sends) handle(send)
  }

  return {
    opening: () => opening,
    push(clientMessageId, frame) {
      parked.push({ clientMessageId, frame })
    },
    ready(send) {
      opening = false
      drain(send)
    },
    abandon(fail) {
      opening = false
      drain((send) => fail(send.clientMessageId))
    },
  }
}
