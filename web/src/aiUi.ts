import type { AiThreadMessage } from './types'

export interface AiUiMessage extends AiThreadMessage {
  streaming?: boolean
  phase?: 'connecting' | 'reasoning' | 'answering'
}
