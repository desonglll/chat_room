import { describe, expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import type { ChatFolder } from '@tg/core'
import { EMPTY_FOLDER, folderDraftError } from './FolderEditor'
import type { ConversationSummary } from '@tg/core'
import { selectChatListView } from '../chatList/chatListFilters'
import { FolderStrip } from './FolderTabs'

const chat = (room_id: string, group: boolean, unread: number, archived = false) =>
  ({
    room_id,
    kind: group ? 'group' : 'direct',
    title: room_id,
    alias: '',
    group: group ? { chat_type: 'group' } : null,
    peer: null,
    unread_count: unread,
    preferences: { is_pinned: false, is_archived: archived, notification_level: 'all', muted_until: null },
    last_message: null,
    last_activity_at: '',
    created_at: '',
  }) as unknown as ConversationSummary
const chats = [chat('dm', false, 2), chat('g', true, 3), chat('archived', true, 1, true)]

const folder = (extra: Partial<ChatFolder>): ChatFolder => ({ ...EMPTY_FOLDER, id: 'f1', title: '工作', ...extra })

describe('TG-501 chat folders (web)', () => {
  test('the editor rejects an empty or over-long title and an empty folder, like the server', () => {
    expect(folderDraftError(EMPTY_FOLDER)).toBe('请输入文件夹名称')
    expect(
      folderDraftError({ ...EMPTY_FOLDER, title: '一二三四五六七八九十一二三', include_types: ['groups'] }),
    ).toContain('12')
    expect(folderDraftError({ ...EMPTY_FOLDER, title: '工作' })).toContain('至少')
    expect(folderDraftError({ ...EMPTY_FOLDER, title: '工作', include_chat_ids: ['c1'] })).toBe('')
  })

  test('the strip is hidden without folders and shows «全部» plus each folder with its unread count', () => {
    const strip = (folders: ChatFolder[], layout: 'top' | 'left' = 'top') =>
      renderToStaticMarkup(
        <FolderStrip
          folders={folders}
          activeId="f1"
          layout={layout}
          conversations={chats}
          now={0}
          onSelect={() => {}}
        />,
      )
    expect(strip([])).toBe('')
    const html = strip([folder({ emoji: '💼', include_types: ['groups'] })])
    expect(html).toContain('全部')
    expect(html).toContain('工作')
    expect(html).toContain('aria-selected="true"')
    expect(html).toContain('data-layout="top"')
    expect(html).toContain('<span class="tg-folders__badge">4</span>')
    expect(strip([folder({ include_types: ['groups'] })], 'left')).toContain('data-layout="left"')
  })

  test('the chat list view narrows the main list to the active folder, archived chats included', () => {
    const view = selectChatListView(chats, 'main', '', { folder: folder({ include_types: ['groups'] }), now: 0 })
    expect(view.rows.map((chat) => chat.room_id).sort()).toEqual(['archived', 'g'])
    expect(
      selectChatListView(chats, 'main', '')
        .rows.map((chat) => chat.room_id)
        .sort(),
    ).toEqual(['dm', 'g'])
  })
})
