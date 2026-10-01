/**
 * TG-1204: user-visible copy goes through `t()`. A Chinese literal outside the zh catalogs
 * renders untranslated in the English UI, so this guard fails on any CJK left in source code
 * (comments excluded). Dev-only galleries, fixtures and benches are allowlisted explicitly.
 */
import { afterEach, describe, expect, test } from 'bun:test'
import { joinList, labelled, setLocale, speaker } from '@tg/core'
import { readdirSync, readFileSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'
import { loadLocale, t } from './index'

const PACKAGES = join(import.meta.dir, '../../..')
const ROOTS = ['web/src', 'core/src', 'ui/src']
const ALLOWED = [
  /\/i18n\/zh(\/|\.ts$)/,
  /\.test\.tsx?$/,
  /\/test\//,
  /\/fixtures\//,
  /\/bench\//,
]
/**
 * Lines that may keep CJK, as `[file, substring]`. INTENTIONAL entries parse user input or name a
 * language in itself. PENDING entries are real English-UI defects in paths other owners hold
 * (reported in docs/devlog/TG-1204.md); delete an entry when its owner fixes the line — the
 * staleness test below fails if an entry no longer matches anything.
 */
const INTENTIONAL: readonly (readonly [string, string])[] = [
  ['core/src/i18n/runtime.ts', "label: '简体中文'"],
  ['core/src/api/linkPreviews.ts', 'replace(/[.,;:!?)'],
  ['core/src/domain/calculator.ts', "replaceAll('×', '*')"],
  ['web/src/features/composer/Composer.tsx', 'const ARITHMETIC'],
  ['web/src/features/message/content/linkify.ts', 'const TRAILING_PUNCTUATION'],
]
const PENDING: readonly (readonly [string, string])[] = [
  ['web/src/features/videoNote/VideoNoteBody.tsx', '${label}，'],
  ['web/src/features/composer/ComposerBar.tsx', '`「${extras.quote.text}」` :'],
  ['web/src/features/composer/ComposerBar.tsx', 'body: `「${extras.quote.text}」`,'],
  ['web/src/features/composer/ComposerBar.tsx', '${message.sender}：'],
  ['web/src/features/composer/PendingDialog.tsx', '${file.name}：'],
  ['web/src/features/composer/Composer.tsx', '${file.name}：'],
  ['web/src/features/message/UploadBubble.tsx', '`：${upload.error}`'],
  ['web/src/features/album/AlbumContent.tsx', '（${index + 1}'],
  ['web/src/features/location/LocationMap.tsx', ".join('，')"],
  ['web/src/features/chatInfo/sharedItems.tsx', "t('w.chatInfo.48e9e5')}："],
  ['web/src/features/chatInfo/sharedItems.tsx', '`GIF：'],
  ['web/src/features/messageList/DefaultMessage.tsx', '{message.reply_to.sender}：'],
]
const EXEMPT = [...INTENTIONAL, ...PENDING]

const CJK = /[㐀-鿿＀-￯　-〿]/

function sourceFiles(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name)
    if (statSync(path).isDirectory()) return sourceFiles(path)
    return /\.tsx?$/.test(name) ? [path] : []
  })
}

/** Drops block comments (incl. JSX `{/* … *\/}`) and line comments, keeping line numbers. */
export function stripComments(source: string): string {
  return source
    .replace(/\/\*[\s\S]*?\*\//g, (block) => block.replace(/[^\n]/g, ' '))
    .replace(/(^|[\s;{}(),])\/\/.*$/gm, '$1')
}

describe('TG-1204 no hard-coded CJK copy', () => {
  test('comment stripping keeps code and drops comments', () => {
    expect(CJK.test(stripComments('const a = 1 // 中文\n{/* 注释 */}'))).toBe(false)
    expect(CJK.test(stripComments("const url = 'https://x'; const s = '中文'"))).toBe(true)
  })

  test('every CJK literal lives in a zh catalog', () => {
    const offenders: string[] = []
    const used = new Set<readonly [string, string]>()
    for (const root of ROOTS) {
      for (const file of sourceFiles(join(PACKAGES, root))) {
        const rel = relative(PACKAGES, file)
        if (ALLOWED.some((pattern) => pattern.test(`/${rel}`))) continue
        stripComments(readFileSync(file, 'utf8'))
          .split('\n')
          .forEach((line, index) => {
            if (!CJK.test(line)) return
            const exempt = EXEMPT.find(([path, needle]) => path === rel && line.includes(needle))
            if (exempt) used.add(exempt)
            else offenders.push(`${rel}:${index + 1}: ${line.trim()}`)
          })
      }
    }
    expect(offenders).toEqual([])
    expect(EXEMPT.filter((entry) => !used.has(entry))).toEqual([])
  })
})

describe('TG-1204 English copy regressions', () => {
  afterEach(() => setLocale('zh-CN'))

  test('“coming soon” quotes the section name with matching marks', async () => {
    await loadLocale('en')
    setLocale('en')
    expect(t('w.settings.76d626', 'Privacy')).toBe('“Privacy” is still in development.')
    setLocale('zh-CN')
    expect(t('w.settings.76d626', '隐私')).toBe('「隐私」还在开发中。')
  })

  test('core punctuation follows the locale', () => {
    expect(speaker('Ann')).toBe('Ann：')
    expect(joinList(['a', 'b'])).toBe('a，b')
    setLocale('en')
    expect(speaker('Ann')).toBe('Ann: ')
    expect(labelled('GIF', 'x.gif')).toBe('GIF: x.gif')
    expect(joinList(['a', 'b'])).toBe('a, b')
  })
})
