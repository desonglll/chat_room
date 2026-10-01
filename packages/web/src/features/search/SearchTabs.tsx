/** TG-504: the seven search tabs under the sidebar search box, plus recent-search suggestions. */
import { useStore } from 'zustand/react'
import { SEARCH_TABS, suggestSearches } from './searchModel'
import { searchStore, selectSearchTab } from './searchStore'
import { t } from '../../i18n/index'

export function SearchTabs({ query, onPick }: { query: string; onPick: (query: string) => void }) {
  const tab = useStore(searchStore, (state) => state.tab)
  const recent = useStore(searchStore, (state) => state.recent)
  const suggestions = suggestSearches(recent, query)
  return (
    <div className="tg-search">
      <div className="tg-search__tabs" role="tablist" aria-label={t('w.search.c3113f')}>
        {SEARCH_TABS.map((item) => (
          <button
            key={item.id}
            type="button"
            role="tab"
            aria-selected={item.id === tab}
            className="tg-search__tab"
            onClick={() => selectSearchTab(item.id)}
          >
            {item.label}
          </button>
        ))}
      </div>
      {suggestions.length > 0 ? (
        <div className="tg-search__suggestions" aria-label={t('w.search.b6f4af')}>
          {suggestions.map((item) => (
            <button key={item} type="button" className="tg-search__chip" onClick={() => onPick(item)}>
              {item}
            </button>
          ))}
        </div>
      ) : null}
    </div>
  )
}
