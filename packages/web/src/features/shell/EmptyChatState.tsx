/** The middle pane before any chat is chosen — one quiet pill, as Telegram does it. */
export function EmptyChatState() {
  return (
    <div className="tg-empty-chat">
      <p className="tg-empty-chat__pill">选择一个会话开始聊天</p>
    </div>
  )
}
