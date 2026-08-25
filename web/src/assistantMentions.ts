export interface MentionableConversation {
  roomId: string
  title: string
}

export interface ParsedAssistantPrompt {
  roomId: string
  question: string
}

export interface ConversationMentionRange {
  start: number
  end: number
  query: string
}

export interface InsertedConversationMention {
  value: string
  caret: number
}

function compactWhitespace(value: string): string {
  return value.replace(/\s+/g, ' ').trim()
}

export function parseAssistantPrompt(
  prompt: string,
  conversations: MentionableConversation[],
  selectedRoomId = '',
): ParsedAssistantPrompt {
  let question = prompt.replace(/@AI助手(?=\s|$)/gi, ' ')
  let roomId = selectedRoomId
  const selected = conversations.find((conversation) => conversation.roomId === selectedRoomId)
  const match =
    selected && question.includes(`@${selected.title}`)
      ? selected
      : [...conversations]
          .filter((conversation) => conversation.title.trim())
          .sort((left, right) => right.title.length - left.title.length)
          .find((conversation) => question.includes(`@${conversation.title}`))
  if (match) {
    question = question.replace(`@${match.title}`, ' ')
    roomId = match.roomId
  }
  return { roomId, question: compactWhitespace(question) }
}

export function activeConversationMention(
  value: string,
  caret: number,
  conversations: MentionableConversation[],
): ConversationMentionRange | null {
  const safeCaret = Math.max(0, Math.min(caret, value.length))
  const start = value.lastIndexOf('@', safeCaret - 1)
  if (start < 0) return null
  const query = value.slice(start + 1, safeCaret)
  if (query.includes('\n')) return null
  const exactSelection = conversations.some((conversation) => {
    if (!query.startsWith(conversation.title)) return false
    const boundary = query[conversation.title.length]
    return boundary !== undefined && /\s/.test(boundary)
  })
  return exactSelection ? null : { start, end: safeCaret, query }
}

export function conversationMentionCandidates(
  range: ConversationMentionRange | null,
  conversations: MentionableConversation[],
): MentionableConversation[] {
  if (!range) return []
  const query = compactWhitespace(range.query).toLocaleLowerCase()
  return conversations
    .filter((conversation) => !query || conversation.title.toLocaleLowerCase().includes(query))
    .sort((left, right) => left.title.localeCompare(right.title, 'zh-CN'))
}

export function insertConversationMention(
  value: string,
  range: ConversationMentionRange,
  conversation: MentionableConversation,
): InsertedConversationMention {
  const mention = `@${conversation.title} `
  const nextValue = `${value.slice(0, range.start)}${mention}${value.slice(range.end)}`
  return { value: nextValue, caret: range.start + mention.length }
}
