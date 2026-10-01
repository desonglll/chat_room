/** TG-407 side-effect module, imported once by `main.tsx`: location bubbles. */
import { registerMessageContent } from '../message'
import { LocationContent } from './LocationContent'
import './location.css'

registerMessageContent('location', LocationContent, {
  match: (message) => message.location !== undefined && message.recalled_at === null,
  priority: 30,
})
