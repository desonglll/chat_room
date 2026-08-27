import type { MenuItem } from 'primevue/menuitem'
import { isConversationMuted } from './conversationState'
import type { ConversationNotificationLevel, ConversationPreferencesPatch } from './conversationPreferencesApi'
import type { ConversationSummary } from './types'

type SavePreference = (patch: ConversationPreferencesPatch, success: string) => void

const NOTIFICATION_LEVELS: Array<[ConversationNotificationLevel, string]> = [
  ['all', '全部消息'],
  ['mentions', '仅提及与回复'],
  ['none', '不通知'],
]

const MUTE_DURATIONS: Array<[string, number]> = [
  ['1 小时', 1],
  ['8 小时', 8],
  ['1 天', 24],
]

export function conversationPreferenceMenuItems(
  conversation: ConversationSummary,
  disabled: boolean,
  save: SavePreference,
): MenuItem[] {
  return [
    {
      label: conversation.preferences.is_pinned ? '取消置顶' : '置顶会话',
      disabled,
      command: () =>
        save(
          { is_pinned: !conversation.preferences.is_pinned },
          conversation.preferences.is_pinned ? '已取消置顶' : '已置顶会话',
        ),
    },
    {
      label: conversation.preferences.is_archived ? '移出归档' : '归档会话',
      disabled,
      command: () =>
        save(
          { is_archived: !conversation.preferences.is_archived },
          conversation.preferences.is_archived ? '已移出归档' : '已归档会话',
        ),
    },
    {
      label: '通知设置',
      items: NOTIFICATION_LEVELS.map(([level, label]) => ({
        label: conversation.preferences.notification_level === level ? `${label}（当前）` : label,
        disabled: conversation.preferences.notification_level === level,
        command: () => save({ notification_level: level }, `通知已设为${label}`),
      })),
    },
    {
      label: isConversationMuted(conversation) ? '静音（已开启）' : '静音时长',
      items: [
        ...MUTE_DURATIONS.map(([label, hours]) => ({
          label,
          command: () =>
            save({ muted_until: new Date(Date.now() + hours * 60 * 60 * 1000).toISOString() }, `已静音${label}`),
        })),
        {
          label: '取消静音',
          disabled: !conversation.preferences.muted_until,
          command: () => save({ muted_until: null }, '已取消静音'),
        },
      ],
    },
  ]
}
