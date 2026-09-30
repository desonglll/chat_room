/** TG-410: the translation of one message, for the viewer who asked. */
import { Modal, Spinner } from '@tg/ui'
import { useStore } from 'zustand/react'
import { translationStore } from './translation'

export function TranslationDialog() {
  const state = useStore(translationStore)
  return (
    <Modal open={state.open !== null} onClose={() => translationStore.setState({ open: null })} title="翻译" size="sm">
      <div className="tg-translation">
        <p className="tg-translation__original">{state.open?.original}</p>
        {state.loading ? <Spinner label="正在翻译" /> : null}
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
