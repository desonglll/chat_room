/**
 * The TG-402 E2E's session harness: numbered steps and screenshots, the HTTP API, account
 * registration through the real login page, polling, the record-button gestures, and extra
 * signed-in contexts.
 */

export function createHarness(BASE_URL, SHOTS) {
  let step = 0

  async function check(name, body) {
    step += 1
    await body()
    console.log(`ok ${step} ${name}`)
  }

  async function shot(page, name) {
    await page.screenshot({ path: `${SHOTS}/${String(step).padStart(2, '0')}-${name}.png` })
  }

  async function api(token, method, path, body) {
    const response = await fetch(`${BASE_URL}${path}`, {
      method,
      headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
      body: body === undefined ? undefined : JSON.stringify(body),
    })
    if (!response.ok) throw new Error(`${method} ${path} → ${response.status} ${await response.text()}`)
    const text = await response.text()
    return text ? JSON.parse(text) : null
  }

  async function register(page, user) {
    await page.goto(`${BASE_URL}/login`)
    await page.getByRole('button', { name: '注册新账号' }).click()
    await page.locator('input[name="username"]').fill(user.username)
    await page.locator('input[name="password"]').fill(user.password)
    await page.getByRole('button', { name: '注册', exact: true }).click()
    await page.getByRole('button', { name: '主菜单' }).first().waitFor()
    const session = await page.evaluate(() => JSON.parse(localStorage.getItem('tg.session.v1') ?? 'null'))
    return { token: session.token, id: session.user.id }
  }

  async function waitFor(predicate, what, timeout = 10_000) {
    const until = Date.now() + timeout
    for (;;) {
      const value = await predicate()
      if (value) return value
      if (Date.now() > until) throw new Error(`timed out: ${what}`)
      await new Promise((resolve) => setTimeout(resolve, 150))
    }
  }

  async function buttonCentre(page) {
    const box = await page.locator('.tg-voice-mic').boundingBox()
    return { x: box.x + box.width / 2, y: box.y + box.height / 2 }
  }

  /** Hold the record button for `ms`, optionally sliding by (dx, dy) before letting go. */
  async function hold(page, ms, slide = null) {
    const { x, y } = await buttonCentre(page)
    await page.mouse.move(x, y)
    await page.mouse.down()
    await page.waitForTimeout(ms)
    if (slide) await page.mouse.move(x + slide.dx, y + slide.dy, { steps: 8 })
    await page.mouse.up()
  }

  /**
   * A further browser context signed in as `source`'s account (session copied before the app
   * boots), optionally with an init script, opened at `path`.
   */
  async function openAs(browser, source, path, { init, errors, ...options } = {}) {
    const context = await browser.newContext({ viewport: { width: 1280, height: 800 }, ...options })
    const session = await source.evaluate(() => localStorage.getItem('tg.session.v1'))
    await context.addInitScript((value) => localStorage.setItem('tg.session.v1', value), session)
    if (init) await context.addInitScript(init)
    const page = await context.newPage()
    if (errors) page.on('pageerror', (error) => errors.push(error.message))
    await page.goto(`${BASE_URL}${path}`)
    return { page, context }
  }

  return { check, shot, api, register, waitFor, buttonCentre, hold, openAs, steps: () => step }
}
