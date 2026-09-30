/**
 * The @mention suggestion list that floats above the input. Keyboard (↑/↓, Enter/Tab,
 * Esc) is owned by the textarea's keydown in `Composer.tsx` so focus never leaves the
 * input; this renders the ARIA listbox the textarea points at via aria-activedescendant.
 */
import type { ChatMember } from '@tg/core'
import { Avatar } from '@tg/ui'

export interface MentionPopupProps {
  id: string
  candidates: readonly ChatMember[]
  activeIndex: number
  onPick(member: ChatMember): void
  onHover(index: number): void
}

export const mentionOptionId = (listId: string, index: number) => `${listId}-opt-${index}`

export function MentionPopup({ id, candidates, activeIndex, onPick, onHover }: MentionPopupProps) {
  if (candidates.length === 0) return null
  return (
    <ul className="tg-compose__mentions" id={id} role="listbox" aria-label="提及成员">
      {candidates.map((member, index) => (
        <li
          key={member.user_id}
          id={mentionOptionId(id, index)}
          role="option"
          aria-selected={index === activeIndex}
          className="tg-compose__mention"
          // mousedown, not click: picking must not blur the textarea first.
          onMouseDown={(event) => {
            event.preventDefault()
            onPick(member)
          }}
          onMouseEnter={() => onHover(index)}
        >
          <Avatar label={member.username} initials={member.avatar_emoji || undefined} size="sm" />
          <span className="tg-compose__mention-name">{member.username}</span>
          <span className="tg-compose__mention-handle">@{member.username}</span>
        </li>
      ))}
    </ul>
  )
}
