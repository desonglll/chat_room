/**
 * WebSocket connection management: reconnect backoff, subscriptions, frame codec, and the
 * catch-up fetch that runs after a dropped connection.
 *
 * The socket itself arrives through an injected `createWebSocket()` factory so this layer can
 * be tested with a fake socket and can run outside a browser.
 *
 * Owner of the contents: TG-011. This barrel is the frozen entry point `@tg/core/realtime`.
 */
export {}
