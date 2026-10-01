/**
 * TG-701: the bottom of a group or channel's info panel — edit its profile, invite someone by
 * username, leave it, or (owner) delete it. Destructive actions confirm first.
 */
import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import type { Chat } from '@tg/core'
import { Button, Modal, TextField } from '@tg/ui'
import { t } from '../../i18n/index'
import { lifecycleActions, profileError } from './lifecycleModel'
import { invite, leave, remove, saveProfile } from './lifecycleApi'

type Dialog = 'edit' | 'invite' | 'leave' | 'delete' | null

export function ChatActions({ chat }: { chat: Chat }) {
  const actions = lifecycleActions(chat.membership_role, chat.chat_type === 'private')
  const [dialog, setDialog] = useState<Dialog>(null)
  const [title, setTitle] = useState(chat.title)
  const [description, setDescription] = useState(chat.description)
  const [emoji, setEmoji] = useState(chat.avatar_emoji)
  const [username, setUsername] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [notice, setNotice] = useState('')
  const navigate = useNavigate()
  if (!actions.edit && !actions.invite && !actions.leave && !actions.delete) return null

  const run = async (work: () => Promise<unknown>, done: () => void) => {
    setBusy(true)
    setError('')
    try {
      await work()
      done()
    } catch {
      setError(t('w.lifecycle.failed'))
    } finally {
      setBusy(false)
    }
  }
  const close = () => {
    setDialog(null)
    setError('')
  }
  const profileProblem = profileError(title, description)
  const channel = chat.chat_type === 'channel'

  return (
    <section className="tg-lifecycle" aria-label={t('w.lifecycle.section')}>
      {actions.edit ? (
        <Button variant="text" fullWidth onClick={() => setDialog('edit')}>
          {t('w.lifecycle.edit')}
        </Button>
      ) : null}
      {actions.invite ? (
        <Button variant="text" fullWidth onClick={() => setDialog('invite')}>
          {t('w.lifecycle.invite')}
        </Button>
      ) : null}
      {actions.leave ? (
        <Button variant="danger" fullWidth onClick={() => setDialog('leave')}>
          {channel ? t('w.lifecycle.leaveChannel') : t('w.lifecycle.leaveGroup')}
        </Button>
      ) : null}
      {actions.delete ? (
        <Button variant="danger" fullWidth onClick={() => setDialog('delete')}>
          {channel ? t('w.lifecycle.deleteChannel') : t('w.lifecycle.deleteGroup')}
        </Button>
      ) : null}
      {notice ? <p role="status">{notice}</p> : null}

      <Modal open={dialog === 'edit'} onClose={close} title={t('w.lifecycle.edit')}>
        <form
          className="tg-lifecycle__form"
          onSubmit={(event) => {
            event.preventDefault()
            if (!profileProblem)
              void run(() => saveProfile(chat.id, { title: title.trim(), description, avatar_emoji: emoji }), close)
          }}
        >
          <TextField
            label={t('w.lifecycle.emoji')}
            value={emoji}
            maxLength={8}
            onChange={(e) => setEmoji(e.target.value)}
          />
          <TextField
            label={t('w.lifecycle.title')}
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            error={profileProblem && title ? t(profileProblem) : undefined}
          />
          <TextField
            label={t('w.lifecycle.description')}
            multiline
            rows={3}
            value={description}
            onChange={(e) => setDescription(e.target.value)}
          />
          {error ? <p role="alert">{error}</p> : null}
          <Button type="submit" disabled={Boolean(profileProblem)} loading={busy}>
            {t('w.lifecycle.save')}
          </Button>
        </form>
      </Modal>

      <Modal open={dialog === 'invite'} onClose={close} title={t('w.lifecycle.invite')}>
        <form
          className="tg-lifecycle__form"
          onSubmit={(event) => {
            event.preventDefault()
            if (username.trim())
              void run(
                () => invite(chat.id, username),
                () => {
                  setNotice(t('w.lifecycle.invited', username.trim()))
                  setUsername('')
                  close()
                },
              )
          }}
        >
          <TextField label={t('w.lifecycle.username')} value={username} onChange={(e) => setUsername(e.target.value)} />
          {error ? <p role="alert">{error}</p> : null}
          <Button type="submit" disabled={!username.trim()} loading={busy}>
            {t('w.lifecycle.sendInvite')}
          </Button>
        </form>
      </Modal>

      <Modal
        open={dialog === 'leave' || dialog === 'delete'}
        onClose={close}
        title={
          dialog === 'delete' ? t('w.lifecycle.confirmDelete', chat.title) : t('w.lifecycle.confirmLeave', chat.title)
        }
      >
        <p>{dialog === 'delete' ? t('w.lifecycle.deleteWarning') : t('w.lifecycle.leaveWarning')}</p>
        {error ? <p role="alert">{error}</p> : null}
        <div className="tg-lifecycle__row">
          <Button variant="text" onClick={close}>
            {t('w.lifecycle.cancel')}
          </Button>
          <Button
            variant="danger"
            loading={busy}
            onClick={() =>
              void run(
                () => (dialog === 'delete' ? remove(chat.id) : leave(chat.id)),
                () => {
                  close()
                  void navigate('/')
                },
              )
            }
          >
            {dialog === 'delete' ? t('w.lifecycle.delete') : t('w.lifecycle.leave')}
          </Button>
        </div>
      </Modal>
    </section>
  )
}
