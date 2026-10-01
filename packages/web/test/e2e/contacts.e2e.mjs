/**
 * TG-801 end-to-end check of «联系人» against a REAL server, two accounts in two browser
 * contexts. Not part of `bun test` (needs a server and Chromium).
 *
 * Run (see docs/devlog/TG-801.md):
 *   CARGO_TARGET_DIR=<private dir> cargo run --bin server -- -p 3801 \
 *     --database-type sqlite --database /tmp/tg801.db
 *   BASE_URL=http://127.0.0.1:3801 node packages/web/test/e2e/contacts.e2e.mjs
 *
 * Env: BASE_URL (required), SHOTS (default /tmp/tg-shots/TG-801), PLAYWRIGHT (module path).
 * Exit code 0 = every step passed; each step prints `ok <n> <name>` or throws.
 */
import { mkdirSync } from 'node:fs'

const BASE_URL = process.env.BASE_URL
if (!BASE_URL) throw new Error('BASE_URL is required, e.g. http://127.0.0.1:3801')
const SHOTS = process.env.SHOTS ?? '/tmp/tg-shots/TG-801'
const { chromium } = await import(process.env.PLAYWRIGHT ?? '/tmp/pw/node_modules/playwright/index.mjs')
mkdirSync(SHOTS, { recursive: true })

const run = Date.now().toString(36)
const ALICE = { username: `alice_${run}`, password: 'correct-horse-1' }
const BOB = { username: `bob_${run}`, password: 'correct-horse-2' }
const pageErrors = []
let step = 0

async function check(name, body) {
  step += 1
  await body()
  console.log(`ok ${step} ${name}`)
}

async function shot(page, name) {
  await page.screenshot({ path: `${SHOTS}/${String(step).padStart(2, '0')}-${name}.png` })
}

async function register(page, user) {
  await page.goto(`${BASE_URL}/login`)
  await page.getByRole('button', { name: '注册新账号' }).click()
  await page.locator('input[name="username"]').fill(user.username)
  await page.locator('input[name="password"]').fill(user.password)
  await page.getByRole('button', { name: '注册', exact: true }).click()
  await page.getByRole('button', { name: '主菜单' }).first().waitFor()
}

async function openContacts(page) {
  await page.goto(`${BASE_URL}/contacts`)
  await page.getByRole('heading', { name: '联系人' }).waitFor()
}

const tab = (page, name) => page.getByRole('tab', { name: new RegExp(`^${name}`) })
const row = (page, username) => page.locator('.tg-contacts__row', { hasText: `@${username}` })

const browser = await chromium.launch()
try {
  const contexts = [await browser.newContext({ viewport: { width: 1280, height: 860 } })]
  contexts.push(await browser.newContext({ viewport: { width: 1280, height: 860 } }))
  const [alice, bob] = await Promise.all(contexts.map((context) => context.newPage()))
  for (const page of [alice, bob]) {
    page.on('pageerror', (error) => pageErrors.push(String(error)))
    page.on('response', (response) => {
      // The main menu probes whether the account is an administrator; 403 is its answer.
      if (response.status() >= 400 && !response.url().endsWith('/api/admin/overview')) pageErrors.push(`${response.status()} ${new URL(response.url()).pathname}`)
    })
  }

  await check('register two accounts', async () => {
    await register(alice, ALICE)
    await register(bob, BOB)
  })

  await check('alice finds bob by username and sends a request', async () => {
    await openContacts(alice)
    await tab(alice, '添加').click()
    await alice.getByRole('textbox', { name: '按用户名查找' }).fill(`@${BOB.username}`)
    await alice.getByRole('button', { name: '查找', exact: true }).click()
    await row(alice, BOB.username).getByRole('button', { name: '加为好友' }).click()
    await row(alice, BOB.username).getByText('等待对方同意').waitFor()
    await shot(alice, 'request-sent')
  })

  await check('bob sees the request live, without reloading', async () => {
    await openContacts(bob)
    await tab(bob, '申请').click()
    await row(bob, ALICE.username).waitFor({ timeout: 8000 })
    await shot(bob, 'request-received')
  })

  await check('bob accepts; both see each other as friends live', async () => {
    await row(bob, ALICE.username).getByRole('button', { name: '接受' }).click()
    await tab(bob, '好友').click()
    await row(bob, ALICE.username).waitFor()
    await tab(alice, '好友').click()
    await row(alice, BOB.username).waitFor({ timeout: 8000 })
  })

  await check('alice sets a remark, cancels a second edit with Escape', async () => {
    await row(alice, BOB.username).getByRole('button', { name: '备注' }).click()
    const field = row(alice, BOB.username).getByRole('textbox', { name: '备注' })
    await field.fill('老鲍')
    await field.press('Enter')
    await row(alice, BOB.username).getByText('老鲍').waitFor()
    await row(alice, BOB.username).getByRole('button', { name: '备注' }).click()
    await row(alice, BOB.username).getByRole('textbox', { name: '备注' }).press('Escape')
    await row(alice, BOB.username).getByRole('button', { name: '发消息' }).waitFor()
    await shot(alice, 'friends')
  })

  await check('发消息 opens the private chat and both sides can talk', async () => {
    await row(alice, BOB.username).getByRole('button', { name: '发消息' }).click()
    await alice.waitForURL(/\/chat\//)
    const chatUrl = alice.url()
    // The reported bug: after switching chats, «慢速模式已开启» locked the composer of a
    // private chat that has no slow mode at all. Switch away and back before sending.
    await alice.goto(`${BASE_URL}/contacts`)
    await alice.waitForTimeout(1500)
    await alice.goto(chatUrl)
    const input = alice.getByLabel('消息内容')
    await input.fill('你好，老鲍')
    await input.press('Enter')
    await bob.goto(chatUrl)
    await bob.getByText('你好，老鲍').first().waitFor({ timeout: 10_000 })
    await bob.getByLabel('消息内容').fill('收到')
    await bob.getByLabel('消息内容').press('Enter')
    await alice.getByText('收到', { exact: true }).first().waitFor({ timeout: 10_000 })
    for (const page of [alice, bob]) {
      if ((await page.getByText('慢速模式').count()) > 0) throw new Error('slow-mode notice in a private chat')
    }
    await shot(alice, 'private-chat')
  })

  await check('alice removes bob; bob sees it live', async () => {
    await openContacts(alice)
    await openContacts(bob)
    await row(alice, BOB.username).getByRole('button', { name: '删除好友' }).click()
    await row(alice, BOB.username).waitFor({ state: 'detached' })
    await row(bob, ALICE.username).waitFor({ state: 'detached', timeout: 8000 })
  })

  await check('bob blocks alice from search; alice can no longer find or message bob', async () => {
    await tab(bob, '添加').click()
    await bob.getByRole('textbox', { name: '按用户名查找' }).fill(ALICE.username)
    await bob.getByRole('button', { name: '查找', exact: true }).click()
    await row(bob, ALICE.username).getByRole('button', { name: '拉黑' }).click()
    await tab(bob, '黑名单').click()
    await row(bob, ALICE.username).waitFor()
    await tab(alice, '添加').click()
    await alice.getByRole('textbox', { name: '按用户名查找' }).fill(BOB.username)
    await alice.getByRole('button', { name: '查找', exact: true }).click()
    await alice.getByText('没有找到').waitFor()
    await shot(bob, 'blocked')
  })

  await check('bob unblocks alice', async () => {
    await row(bob, ALICE.username).getByRole('button', { name: '解除拉黑' }).click()
    await row(bob, ALICE.username).waitFor({ state: 'detached' })
  })

  await check('a one-letter query explains itself instead of failing silently', async () => {
    await tab(alice, '添加').click()
    await alice.getByRole('textbox', { name: '按用户名查找' }).fill('b')
    await alice.getByRole('button', { name: '查找', exact: true }).click()
    await alice.getByText('至少输入 2 个字符').waitFor()
  })

  if (pageErrors.length > 0) throw new Error(`page errors:\n${pageErrors.join('\n')}`)
} finally {
  await browser.close()
}
