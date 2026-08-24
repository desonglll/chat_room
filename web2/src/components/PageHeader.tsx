import type { ReactNode } from 'react'

export function PageHeader({
  title,
  description,
  actions,
}: {
  title: string
  description?: string
  actions?: ReactNode
}) {
  return (
    <header className="flex min-h-[86px] items-center justify-between gap-4 border-b border-[#e1e6e4] px-5 py-4 sm:px-7">
      <div className="min-w-0">
        <h1 className="m-0 truncate text-xl font-semibold text-[#17201d]">{title}</h1>
        {description && <p className="mt-1 mb-0 truncate text-sm text-[#6c7873]">{description}</p>}
      </div>
      {actions && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
    </header>
  )
}
