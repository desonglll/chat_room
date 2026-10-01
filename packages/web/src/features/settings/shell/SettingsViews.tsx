/**
 * The three kinds of settings view (TG-110): the root (profile header + section rows), a
 * section (its registered pages as rows, or «即将推出»), and a page (a registered component,
 * under the shell's header unless the page draws its own).
 */
import { Suspense, type ReactNode } from 'react'
import type { User } from '@tg/core'
import { authStore } from '@tg/core'
import { Avatar, IconButton, Spinner } from '@tg/ui'
import { useStore } from 'zustand/react'
import { profileHeaderModel } from './profileModel'
import { SettingsIcon } from './settingsIcons'
import { viewForSection, type SettingsView } from './settingsNavigation'
import {
  pagesInSection,
  SETTINGS_SECTIONS,
  type SettingsPageRegistration,
  type SettingsSection,
  type SettingsSectionId,
} from './settingsRegistry'
import { t } from '../../../i18n/index'

export interface ViewContext {
  pages: readonly SettingsPageRegistration[]
  push(view: SettingsView): void
  back(): void
  close(): void
}

export function SettingsHeader({
  title,
  onBack,
  backLabel = t('w.settings.11d024'),
  children,
}: {
  title: string
  onBack(): void
  backLabel?: string
  children?: ReactNode
}) {
  return (
    <header className="tg-settings__header">
      <IconButton label={backLabel} variant="plain" onClick={onBack}>
        <SettingsIcon name="back" />
      </IconButton>
      <h2 className="tg-settings__title" tabIndex={-1}>
        {title}
      </h2>
      {children}
    </header>
  )
}

function Row({ icon, title, value, onOpen }: { icon: ReactNode; title: string; value?: string; onOpen(): void }) {
  return (
    <button type="button" className="tg-settings__row" onClick={onOpen}>
      <span className="tg-settings__row-icon">{icon}</span>
      <span className="tg-settings__row-title">{title}</span>
      {value ? <span className="tg-settings__row-value">{value}</span> : null}
    </button>
  )
}

export function ProfileCard({ user, onEdit }: { user: User | null; onEdit(): void }) {
  if (!user) return null
  const model = profileHeaderModel(user)
  return (
    <button
      type="button"
      className="tg-settings-profile"
      onClick={onEdit}
      aria-label={t('w.settings.49ccd8', model.name)}
    >
      <Avatar src={model.avatarSrc} label={model.name} initials={model.initials} size="xl" />
      <span className="tg-settings-profile__text">
        <span className="tg-settings-profile__name">{model.name}</span>
        <span className="tg-settings-profile__handle">{model.handle}</span>
      </span>
    </button>
  )
}

export function RootView({ context }: { context: ViewContext }) {
  const user = useStore(authStore, (state) => state.session?.user ?? null)
  const openSection = (section: SettingsSectionId) =>
    context.push(viewForSection(section, pagesInSection(context.pages, section)))
  return (
    <>
      <SettingsHeader title={t('w.settings.7debf9')} onBack={context.close} backLabel={t('w.settings.7346c0')}>
        <IconButton label={t('w.settings.7719bd')} variant="plain" onClick={() => openSection('account')}>
          <SettingsIcon name="edit" />
        </IconButton>
      </SettingsHeader>
      <div className="tg-settings__body">
        <ProfileCard user={user} onEdit={() => openSection('account')} />
        <nav className="tg-settings__group tg-settings__group--rows" aria-label={t('w.settings.fb3849')}>
          {SETTINGS_SECTIONS.map((section) => (
            <Row
              key={section.id}
              icon={<SettingsIcon name={section.id} />}
              title={section.title}
              onOpen={() => openSection(section.id)}
            />
          ))}
        </nav>
      </div>
    </>
  )
}

function ComingSoon({ section }: { section: SettingsSection }) {
  return (
    <div className="tg-settings__placeholder" role="status">
      <SettingsIcon name={section.id} size={48} />
      <p className="tg-settings__placeholder-title">{t('w.settings.7553b9')}</p>
      <p className="tg-settings__hint">
        「{section.title}
        {t('w.settings.76d626')}
      </p>
    </div>
  )
}

/**
 * A section with several pages (TG-1002): Telegram shows the section's main page itself and the
 * other pages as rows under it («隐私与安全» = the rules, then «两步验证») — never a menu whose
 * first row repeats the section's own name.
 */
export function SectionView({ section, context }: { section: SettingsSection; context: ViewContext }) {
  const [primary, ...more] = pagesInSection(context.pages, section.id)
  if (!primary) {
    return (
      <>
        <SettingsHeader title={section.title} onBack={context.back} />
        <div className="tg-settings__body">
          <ComingSoon section={section} />
        </div>
      </>
    )
  }
  const Page = primary.component
  const rows =
    more.length === 0 ? null : (
      <div className="tg-settings__group tg-settings__group--rows tg-settings__group--more">
        {more.map((page) => (
          <Row
            key={page.id}
            icon={page.icon ?? <SettingsIcon name={section.id} />}
            title={page.title}
            onOpen={() => context.push({ kind: 'page', id: page.id })}
          />
        ))}
      </div>
    )
  const body = (
    <Suspense fallback={<Spinner label={t('w.settings.3667cb')} />}>
      <Page onBack={context.back} onClose={context.close} />
    </Suspense>
  )
  if (primary.ownsHeader) {
    return (
      <div className="tg-settings__body tg-settings__body--bare">
        {body}
        {rows}
      </div>
    )
  }
  return (
    <>
      <SettingsHeader title={section.title} onBack={context.back} />
      <div className="tg-settings__body">
        {body}
        {rows}
      </div>
    </>
  )
}

export function PageView({ page, context }: { page: SettingsPageRegistration; context: ViewContext }) {
  const Page = page.component
  const body = (
    <Suspense fallback={<Spinner label={t('w.settings.3667cb')} />}>
      <Page onBack={context.back} onClose={context.close} />
    </Suspense>
  )
  if (page.ownsHeader) return <div className="tg-settings__body tg-settings__body--bare">{body}</div>
  return (
    <>
      <SettingsHeader title={page.title} onBack={context.back} />
      <div className="tg-settings__body">{body}</div>
    </>
  )
}

export function sectionById(id: SettingsSectionId): SettingsSection {
  return SETTINGS_SECTIONS.find((section) => section.id === id) ?? SETTINGS_SECTIONS[0]!
}
