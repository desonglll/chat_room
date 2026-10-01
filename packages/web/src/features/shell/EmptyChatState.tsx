import { t } from '../../i18n/index'
/** The middle pane before any chat is chosen — one quiet pill, as Telegram does it. */
export function EmptyChatState() {
  return (
    <div className="tg-empty-chat">
      <p className="tg-empty-chat__pill">{t('w.shell.5557cf')}</p>
    </div>
  )
}
