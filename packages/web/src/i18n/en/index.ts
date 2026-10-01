/** TG-510: English messages; keys match ../zh. One file per feature area. */
import type { Catalog } from '@tg/core'
import { admin } from './admin'
import { album } from './album'
import { auth } from './auth'
import { channel } from './channel'
import { chat } from './chat'
import { chatAdmin } from './chatAdmin'
import { chatInfo } from './chatInfo'
import { chatList } from './chatList'
import { chatPreview } from './chatPreview'
import { composer } from './composer'
import { contact } from './contact'
import { customEmoji } from './customEmoji'
import { folders } from './folders'
import { forum } from './forum'
import { gif } from './gif'
import { inviteLinks } from './inviteLinks'
import { contactsPage } from './contactsPage'
import { lifecycle } from './lifecycle'
import { notificationsPage } from './notificationsPage'
import { password } from './password'
import { tasks } from './tasks'
import { linkPreview } from './linkPreview'
import { location } from './location'
import { mediaViewer } from './mediaViewer'
import { message } from './message'
import { messageList } from './messageList'
import { poll } from './poll'
import { presence } from './presence'
import { profile } from './profile'
import { pwa } from './pwa'
import { savedMessages } from './savedMessages'
import { scheduled } from './scheduled'
import { search } from './search'
import { settings } from './settings'
import { shell } from './shell'
import { sticker } from './sticker'
import { videoNote } from './videoNote'
import { voice } from './voice'

export const en: Catalog = {
  ...admin,
  ...album,
  ...auth,
  ...channel,
  ...chat,
  ...chatAdmin,
  ...chatInfo,
  ...chatList,
  ...chatPreview,
  ...composer,
  ...contact,
  ...customEmoji,
  ...folders,
  ...forum,
  ...gif,
  ...inviteLinks,
  ...contactsPage,
  ...lifecycle,
  ...notificationsPage,
  ...password,
  ...tasks,
  ...linkPreview,
  ...location,
  ...mediaViewer,
  ...message,
  ...messageList,
  ...poll,
  ...presence,
  ...profile,
  ...pwa,
  ...savedMessages,
  ...scheduled,
  ...search,
  ...settings,
  ...shell,
  ...sticker,
  ...videoNote,
  ...voice,
}
