export interface MentionableConversation {
  roomId: string
  title: string
}

export interface ParsedAssistantPrompt {
  roomId: string
  question: string
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
  const match = [...conversations]
    .filter((conversation) => conversation.title.trim())
    .sort((left, right) => right.title.length - left.title.length)
    .find((conversation) => question.includes(`@${conversation.title}`))
  if (match) {
    question = question.replace(`@${match.title}`, ' ')
    roomId = match.roomId
  }
  return { roomId, question: compactWhitespace(question) }
}
