export function messageIdFromRoute(
  routeName: unknown,
  routeRoomId: unknown,
  messageQuery: unknown,
  currentRoomId: string,
): string {
  if (routeName !== 'room' || routeRoomId !== currentRoomId || typeof messageQuery !== 'string') return ''
  return messageQuery.trim()
}
