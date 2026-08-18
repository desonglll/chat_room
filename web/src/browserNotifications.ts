import type { AccountMessageEvent } from './types'

export function createBrowserNotifier(onActivate: (roomId: string) => void) {
  let enabled = false
  let showDetails = true

  function configure(nextEnabled: boolean, nextShowDetails: boolean): void {
    enabled = nextEnabled
    showDetails = nextShowDetails
  }

  function notify(message: AccountMessageEvent): void {
    if (!enabled
      || document.visibilityState === 'visible'
      || typeof Notification === 'undefined'
      || Notification.permission !== 'granted') return

    const title = showDetails ? `${message.sender} · ${message.room_name}` : 'Chat Room'
    const body = showDetails
      ? message.content || (message.attachment_file_name ? `发送了附件：${message.attachment_file_name}` : '发来一条消息')
      : '你有一条新消息'
    const notification = new Notification(title, {
      body,
      icon: '/favicon.svg',
      tag: `${message.room_id}:${message.message_id}`,
    })
    notification.onclick = () => {
      window.focus()
      onActivate(message.room_id)
      notification.close()
    }
  }

  return { configure, notify }
}
