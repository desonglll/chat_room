import { useCallback, useEffect, useState } from 'react'
import { ACCENTS, panesFor, parseFragment, toFragment, WALLPAPERS, type GalleryState } from './galleryState'
import { DOC_PAGES, DOC_PAGE_IDS } from './pages'

/**
 * The documentation page for `@tg/ui`, modelled on TG-009's `preview.html`:
 *
 *  - one demo tree, rendered into two panes that differ ONLY by `data-tg-theme`, so a difference
 *    between day and night is always the token layer and never the markup;
 *  - all of the state in the URL fragment, so `#page=menu&theme=night&accent=cyan` is a link;
 *  - no build step and no Storybook. `bun run -F @tg/ui docs` serves this file directly.
 *
 * One known limitation, deliberate: overlays portal into `document.body`, which sits outside both
 * panes, so in split mode an open menu or dialog would paint with the root theme. The gallery
 * therefore mirrors the pane you are pointing at (or focused inside) onto the root element, which
 * is correct for the one-overlay-at-a-time case a reviewer actually exercises. The alternative was
 * a portal-container prop on every overlay, which is API surface added for a documentation need;
 * see docs/devlog/TG-010.md.
 */
export function Gallery() {
  const [state, setState] = useState<GalleryState>(() => parseFragment(window.location.hash, DOC_PAGE_IDS))

  useEffect(() => {
    const fragment = toFragment(state)
    if (window.location.hash !== fragment) window.history.replaceState(null, '', fragment)
  }, [state])

  useEffect(() => {
    const onHashChange = () => setState(parseFragment(window.location.hash, DOC_PAGE_IDS))
    window.addEventListener('hashchange', onHashChange)
    return () => window.removeEventListener('hashchange', onHashChange)
  }, [])

  const patch = useCallback((next: Partial<GalleryState>) => setState((current) => ({ ...current, ...next })), [])

  const adoptRoot = useCallback(
    (theme: 'day' | 'night') => {
      const root = document.documentElement
      root.setAttribute('data-tg-theme', theme)
      if (state.accent === '') root.removeAttribute('data-tg-accent')
      else root.setAttribute('data-tg-accent', state.accent)
      root.setAttribute('data-tg-wallpaper', state.wallpaper)
    },
    [state.accent, state.wallpaper],
  )

  const page = DOC_PAGES.find((candidate) => candidate.id === state.page) ?? DOC_PAGES[0]
  if (page === undefined) return null
  const { Demo } = page

  return (
    <div className="doc-shell" data-tg-theme={state.theme === 'night' ? 'night' : 'day'}>
      <nav className="doc-nav" aria-label="Components">
        <span className="doc-nav__brand">@tg/ui</span>
        {DOC_PAGES.map((candidate) => (
          <a
            key={candidate.id}
            href={toFragment({ ...state, page: candidate.id })}
            className="doc-nav__link"
            aria-current={candidate.id === page.id ? 'page' : undefined}
          >
            {candidate.title}
          </a>
        ))}
      </nav>

      <main className="doc-main">
        <header className="doc-header">
          <div className="doc-header__titles">
            <h1 className="doc-header__title">{page.title}</h1>
            <p className="doc-header__summary">{page.summary}</p>
          </div>
          <div className="doc-controls">
            <label className="doc-control">
              theme
              <select
                value={state.theme}
                onChange={(event) => patch({ theme: event.target.value as GalleryState['theme'] })}
              >
                <option value="split">day + night</option>
                <option value="day">day</option>
                <option value="night">night</option>
              </select>
            </label>
            <label className="doc-control">
              accent
              <select value={state.accent} onChange={(event) => patch({ accent: event.target.value })}>
                {ACCENTS.map((accent) => (
                  <option key={accent} value={accent}>
                    {accent === '' ? 'factory' : accent}
                  </option>
                ))}
              </select>
            </label>
            <label className="doc-control">
              wallpaper
              <select value={state.wallpaper} onChange={(event) => patch({ wallpaper: event.target.value })}>
                {WALLPAPERS.map((wallpaper) => (
                  <option key={wallpaper} value={wallpaper}>
                    {wallpaper}
                  </option>
                ))}
              </select>
            </label>
          </div>
        </header>

        {page.notes === undefined ? null : (
          <ul className="doc-notes">
            {page.notes.map((note) => (
              <li key={note}>{note}</li>
            ))}
          </ul>
        )}

        <div className={`doc-panes doc-panes--${state.theme}`}>
          {panesFor(state.theme).map((theme) => (
            <section
              key={theme}
              className="doc-pane"
              data-tg-theme={theme}
              data-tg-accent={state.accent === '' ? undefined : state.accent}
              data-tg-wallpaper={state.wallpaper}
              aria-label={`${page.title}, ${theme} theme`}
              onPointerEnter={() => adoptRoot(theme)}
              onFocus={() => adoptRoot(theme)}
            >
              <h2 className="doc-pane__label">{theme}</h2>
              <div className="doc-pane__body">
                <Demo />
              </div>
            </section>
          ))}
        </div>

        <footer className="doc-footer">
          Tokens are TG-009&rsquo;s and frozen. Reduced motion: switch it on in the OS and reload - movement collapses,
          fades survive, and the spinner keeps turning.
        </footer>
      </main>
    </div>
  )
}
