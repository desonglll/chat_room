import { VisuallyHidden } from '@tg/ui'

/**
 * Skeleton page for TG-002. Its only job is to prove the toolchain: React 19 renders, JSX
 * compiles, and `@tg/ui` resolves across the workspace.
 *
 * TG-012 replaces this with the real shell (router, login, three-column layout).
 */
export function App() {
  return (
    <main>
      <VisuallyHidden>Echo Gate</VisuallyHidden>
      <h1>Echo Gate</h1>
      <p>React 19 client skeleton. The login flow and chat shell arrive with TG-012.</p>
    </main>
  )
}
