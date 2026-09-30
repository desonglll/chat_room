/**
 * `@tg/core/domain` — pure business logic, one concern per file. No platform access, no
 * transport; anything platform-shaped arrives as an injected interface from
 * `types/platform.ts`.
 */
export * from './messageView'
export * from './calculator'
export * from './markdown'
export * from './chatOptimistic'
export * from './chatIncoming'
export * from './messageReactions'
export * from './messageMotion'
export * from './messageViewportPolicy'
export * from './attachmentUploadProgress'
export * from './fileHash'
export * from './avatarColor'
export * from './randomUuid'
export * from './messageDeepLink'
export * from './searchPattern'
export * from './textFrameBatch'
export * from './draftSync'
