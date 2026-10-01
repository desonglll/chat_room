/**
 * TG-510: the web app's `t`. Importing it registers the web catalogs (zh-CN source, English);
 * core registers its own. Components call `t()` while rendering, and the app root re-renders
 * on `localeStore` changes, so switching language needs no page reload.
 */
import { registerMessages, translate } from '@tg/core'
import { en } from './en/index'
import { zh } from './zh/index'

registerMessages('zh-CN', zh)
registerMessages('en', en)

export const t = translate
