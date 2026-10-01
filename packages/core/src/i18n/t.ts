/** TG-510: core's `t`, with core's catalogs registered on first import. */
import { en } from './en'
import { registerMessages, translate } from './runtime'
import { zh } from './zh'

registerMessages('zh-CN', zh)
registerMessages('en', en)

export const t = translate
