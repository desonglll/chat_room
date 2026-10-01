/** TG-410: the translation of one message, for the viewer who asked. */
import { Modal, Spinner } from '@tg/ui'
import { useStore } from 'zustand/react'
import { translationStore } from './translation'
import { t } from '../../i18n/index'

export function TranslationDialog() {
  const state = useStore(translationStore)
  return (
    <Modal
      open={state.open !== null}
      onClose={() => translationStore.setState({ open: null })}
      title={t('w.contact.231413')}
      size="sm"
    >
      <div className="tg-translation">
        <p className="tg-translation__original">{state.open?.original}</p>
        {state.loading ? <Spinner label={t('w.contact.8ccb0c')} /> : null}
        {state.text ? <p className="tg-translation__text">{state.text}</p> : null}
        {state.error ? (
          <p className="tg-translation__original" role="alert">
            {state.error}
          </p>
        ) : null}
      </div>
    </Modal>
  )
}
