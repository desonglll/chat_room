/**
 * Extract the target message id from a deep-link route. Migrated from
 * `web/src/messageDeepLink.ts` (TG-011); the expected route name is now a parameter
 * defaulting to `'chat'` (the old client hardcoded its Vue route record name `'room'`,
 * which was never a server contract — pass `'room'` to get the old behaviour).
 */
export function messageIdFromRoute(
  routeName: unknown,
  routeChatId: unknown,
  messageQuery: unknown,
  currentChatId: string,
  expectedRouteName = 'chat',
): string {
  if (routeName !== expectedRouteName || routeChatId !== currentChatId || typeof messageQuery !== 'string') return ''
  return messageQuery.trim()
}
