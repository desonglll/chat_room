import type { BroadcastMessage } from './types'

export function createBrowserNotifier(
  roomName: () => string,
  currentUserId: () => string,
) {
  let readyAt = 0
  let enabled = false
  let showDetails = true

  function configure(nextEnabled: boolean, nextShowDetails: boolean): void {
    enabled = nextEnabled
    showDetails = nextShowDetails
  }

  function arm(): void {
    readyAt = performance.now() + 1200
  }

  function notify(message: BroadcastMessage): void {
    if (!enabled
      || message.sender_id === currentUserId()
      || document.visibilityState === 'visible'
      || performance.now() < readyAt
      || typeof Notification === 'undefined'
      || Notification.permission !== 'granted') return

    const title = showDetails ? `${message.sender} · ${roomName()}` : 'Chat Room'
    const body = showDetails
      ? message.content || (message.attachment ? `发送了附件：${message.attachment.file_name}` : '发来一条消息')
      : '你有一条新消息'
    const notification = new Notification(title, {
      body,
      icon: '/favicon.svg',
      tag: message.message_id,
    })
    notification.onclick = () => {
      window.focus()
      notification.close()
    }
  }

  return { arm, configure, notify }
}
