/**
 * TG-410 side-effect module, imported once by `main.tsx`: contact cards render through the
 * TG-103 content registry, and «翻译» joins the message menu when the server can translate.
 */
import { registerMessageContent, registerMessageMenuItem } from '../message'
import { ContactContent } from './ContactContent'
import { checkTranslationAvailability, translateMessage, translationStore } from './translation'
import './contact.css'

registerMessageContent('contact', ContactContent, {
  match: (message) => message.contact !== undefined && message.recalled_at === null,
  priority: 30,
})

checkTranslationAvailability()
registerMessageMenuItem('translate', (message) =>
  translationStore.getState().available && message.content.trim() !== '' && !message.message_id.startsWith('pending:')
    ? { id: 'translate', label: '翻译', onSelect: () => translateMessage(message.message_id, message.content) }
    : null,
)
