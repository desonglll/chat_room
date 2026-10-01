/**
 * TG-1101 end-to-end walkthrough of forum topics and two-step verification against a REAL
 * server (two accounts). Not part of `bun test`. Seed with the TG-804 seed script or any data
 * containing two friends in one group, then:
 *   BASE_URL=http://127.0.0.1:3921 SEED=/path/seed.json node packages/web/test/e2e/forum2fa.e2e.mjs
 * Also guards the header bell's first-paint styles (TG-1003 regression).
 */
import { mkdirSync } from 'node:fs'
const { chromium } = await import(process.env.PLAYWRIGHT ?? '/tmp/pw/node_modules/playwright/index.mjs')
function mk(BASE) {
  async function api(token, method, path, body) {
    const r = await fetch(`${BASE}${path}`, {
      method,
      headers: { 'content-type': 'application/json', ...(token ? { authorization: `Bearer ${token}` } : {}) },
      body: body === undefined ? undefined : JSON.stringify(body),
    })
    if (!r.ok) throw new Error(`${method} ${path} → ${r.status} ${await r.text()}`)
    const text = await r.text()
    return text ? JSON.parse(text) : null
  }
  async function account(name) {
    const username = `${name}_${Date.now().toString(36)}${Math.floor(Math.random() * 1000)}`
    const a = await api(null, 'POST', '/api/users/register', { username, password: 'correct-horse-9' })
    return { ...a, username, password: 'correct-horse-9' }
  }
  async function login(page, acct, width = 1440, height = 900) {
    await page.setViewportSize({ width, height })
    await page.goto(`${BASE}/login`)
    await page.locator('input[name="username"]').fill(acct.username)
    await page.locator('input[name="password"]').fill(acct.password)
    await page.getByRole('button', { name: '登录', exact: true }).click()
    await page.getByRole('button', { name: '主菜单' }).first().waitFor()
  }
  return { api, account, login }
}

import { readFileSync } from 'node:fs'
const BASE = process.env.BASE_URL
const seed = JSON.parse(readFileSync(process.env.SEED, 'utf8'))
const { login } = mk(BASE)
const OUT = process.env.SHOTS ?? '/tmp/tg-shots/TG-1101'
mkdirSync(OUT, { recursive: true })
const browser = await chromium.launch()
const errors = [],
  bad = []
const page = async (who) => {
  const p = await (await browser.newContext({ viewport: { width: 1440, height: 900 } })).newPage()
  p.on('pageerror', (e) => errors.push(String(e)))
  p.on(
    'response',
    (r) =>
      r.status() >= 400 &&
      !r.url().includes('/two-factor') &&
      bad.push(`${r.status()} ${r.request().method()} ${new URL(r.url()).pathname}`),
  )
  await login(p, who)
  return p
}
let n = 0
const step = async (p, name, body) => {
  n++
  try {
    await body()
    await p.waitForTimeout(900)
    await p.screenshot({ path: `${OUT}/${String(n).padStart(2, '0')}-${name}.png` })
    console.log('ok', name)
  } catch (e) {
    console.log('FAIL', name, String(e).split('\n')[0])
    await p.screenshot({ path: `${OUT}/${String(n).padStart(2, '0')}-${name}-FAIL.png` }).catch(() => {})
  }
}
const a = await page(seed.alice),
  b = await page(seed.bob)
await step(a, 'bell-styled', async () => {
  const style = await a
    .locator('.tg-bell')
    .first()
    .evaluate((e) => getComputedStyle(e).borderTopStyle + '/' + getComputedStyle(e).backgroundColor)
  if (!style.startsWith('none')) throw new Error(`bell has a native border: ${style}`)
})
// ---- forum
await step(a, 'enable-topics', async () => {
  await a.goto(`${BASE}/chat/${seed.group}`)
  await a.locator('.tg-message').first().waitFor()
  await a.getByRole('button', { name: '查看会话信息' }).click()
  await a.waitForTimeout(800)
  await a.getByText('管理群组').first().click()
  await a.waitForTimeout(800)
  await a.getByRole('switch', { name: /话题/ }).first().click()
  await a.waitForTimeout(1200)
})
await step(a, 'topic-list', async () => {
  await a.keyboard.press('Escape')
  await a.goto(`${BASE}/chat/${seed.group}`)
  await a
    .getByText(/General/)
    .first()
    .waitFor({ timeout: 6000 })
})
await step(a, 'create-topic', async () => {
  await a.getByRole('button', { name: '新建话题' }).first().click()
  await a.getByRole('textbox', { name: '话题名称' }).fill('发布计划')
  await a.getByRole('button', { name: '创建', exact: true }).click()
  await a.locator('.tg-topicrow', { hasText: '发布计划' }).waitFor({ timeout: 5000 })
})
await step(a, 'post-in-topic', async () => {
  await a.locator('.tg-topicrow', { hasText: '发布计划' }).click()
  await a.locator('.tg-compose__input').fill('周五发布 v2')
  await a.keyboard.press('Enter')
  await a.locator('.tg-message', { hasText: '周五发布 v2' }).waitFor({ timeout: 5000 })
})
await step(b, 'member-sees-topic', async () => {
  await b.goto(`${BASE}/chat/${seed.group}`)
  await b.locator('.tg-topicrow', { hasText: '发布计划' }).click()
  await b.locator('.tg-message', { hasText: '周五发布 v2' }).waitFor({ timeout: 6000 })
})
await step(a, 'close-topic', async () => {
  await a.goto(`${BASE}/chat/${seed.group}`)
  const row = a.locator('.tg-topicrow', { hasText: '发布计划' })
  await row
    .getByRole('button', { name: '话题操作' })
    .click()
    .catch(async () => {
      await row.click({ button: 'right' })
    })
  await a.getByRole('menuitem', { name: '关闭话题' }).click()
  await row.getByRole('img', { name: '已关闭' }).waitFor({ timeout: 5000 })
})
await step(b, 'closed-topic-locks-member', async () => {
  await b.goto(`${BASE}/chat/${seed.group}`)
  await b.locator('.tg-topicrow', { hasText: '发布计划' }).click()
  await b.waitForTimeout(1200)
  const composer = await b.locator('.tg-compose__input').count()
  if (composer > 0 && (await b.locator('.tg-compose__input').isEditable()))
    throw new Error('member can still write in a closed topic')
})
// ---- 2FA
const settings = async (p) => {
  await p.goto(`${BASE}/`)
  await p.getByRole('button', { name: '主菜单' }).first().click()
  await p.getByRole('menuitem', { name: '设置' }).click()
  await p.waitForTimeout(700)
  await p.locator('.tg-settings__row', { hasText: '隐私与安全' }).first().click()
  await p.waitForTimeout(1000)
  await p.locator('.tg-settings__row', { hasText: '两步验证' }).click()
  await p.waitForTimeout(900)
}
await step(a, '2fa-enable', async () => {
  await settings(a)
  await a.getByRole('button', { name: '设置两步验证密码' }).click()
  await a.getByLabel('账号密码').fill(seed.alice.password)
  await a.getByLabel('两步验证密码', { exact: true }).fill('second-factor-1')
  await a.getByLabel('再次输入两步验证密码').fill('second-factor-1')
  await a.getByLabel('密码提示（可选）').fill('猫的名字')
  await a.getByRole('button', { name: '开启两步验证' }).click()
  await a.getByText('两步验证已开启').first().waitFor({ timeout: 6000 })
  const headings = await a.getByRole('heading', { name: '两步验证' }).count()
  if (headings !== 1) throw new Error(`«两步验证» heading shown ${headings} times`)
})
const fresh = await (await browser.newContext({ viewport: { width: 1440, height: 900 } })).newPage()
fresh.on('pageerror', (e) => errors.push(String(e)))
await step(fresh, '2fa-login', async () => {
  await fresh.goto(`${BASE}/login`)
  await fresh.locator('input[name="username"]').fill(seed.alice.username)
  await fresh.locator('input[name="password"]').fill(seed.alice.password)
  await fresh.getByRole('button', { name: '登录', exact: true }).click()
  await fresh.getByText('提示：猫的名字').waitFor({ timeout: 6000 })
  await fresh.getByLabel('两步验证密码').fill('wrong-one')
  await fresh.getByRole('button', { name: '下一步' }).click()
  await fresh.waitForTimeout(1000)
  if (await fresh.getByRole('button', { name: '主菜单' }).count()) throw new Error('wrong 2FA password logged in')
  await fresh.getByLabel('两步验证密码').fill('second-factor-1')
  await fresh.getByRole('button', { name: '下一步' }).click()
  await fresh.getByRole('button', { name: '主菜单' }).first().waitFor({ timeout: 6000 })
})
await step(fresh, '2fa-disable', async () => {
  await settings(fresh)
  await fresh.getByRole('button', { name: '关闭两步验证' }).first().click()
  await fresh.getByLabel('当前两步验证密码').fill('second-factor-1')
  await fresh.getByRole('button', { name: '关闭两步验证' }).last().click()
  await fresh.getByText('两步验证已关闭').first().waitFor({ timeout: 6000 })
})
console.log('errors', errors, 'bad', [...new Set(bad)])
if (errors.length || bad.length) process.exitCode = 1
await browser.close()
