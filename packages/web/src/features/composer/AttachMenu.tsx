/**
 * The paperclip: a menu of attachment kinds. Photo/video and file open the native file
 * picker; «投票» opens TG-406's creation dialog (lazy: the poll form only loads when used);
 * «联系人» opens TG-410's friend picker; location is present but DISABLED (not hidden) until its task (TG-407,
 * M4) enables it — Telegram's menu shape stays stable.
 */
import { lazy, Suspense, useRef, useState, type ChangeEvent } from 'react'
import type { MenuItem } from '@tg/ui'
import { IconButton, Menu } from '@tg/ui'
import { ContactGlyph, FileGlyph, LocationGlyph, PaperclipGlyph, PhotoGlyph, PollGlyph } from './icons'

export interface AttachMenuProps {
  disabled?: boolean
  /** The chat a poll is created in; without it the «投票» entry stays disabled. */
  chatId?: string | undefined
  /** `asFiles`: picked through «文件», sent without photo/video treatment. */
  onFiles(files: File[], asFiles: boolean): void
}

const PollCreateDialog = lazy(() => import('../poll/PollCreateDialog'))
// TG-410: the friend picker loads only when «联系人» is used.
const ContactPickerDialog = lazy(() => import('../contact/ContactPickerDialog'))

export function AttachMenu({ disabled = false, chatId, onFiles }: AttachMenuProps) {
  const [open, setOpen] = useState(false)
  const [pollOpen, setPollOpen] = useState(false)
  const [contactOpen, setContactOpen] = useState(false)
  const triggerRef = useRef<HTMLButtonElement>(null)
  const mediaInput = useRef<HTMLInputElement>(null)
  const fileInput = useRef<HTMLInputElement>(null)

  const items: MenuItem[] = [
    { id: 'media', label: '图片或视频', icon: <PhotoGlyph />, onSelect: () => mediaInput.current?.click() },
    { id: 'file', label: '文件', icon: <FileGlyph />, onSelect: () => fileInput.current?.click() },
    { id: 'location', label: '位置', icon: <LocationGlyph />, disabled: true, separatorBefore: true },
    chatId
      ? { id: 'poll', label: '投票', icon: <PollGlyph />, onSelect: () => setPollOpen(true) }
      : { id: 'poll', label: '投票', icon: <PollGlyph />, disabled: true },
    chatId
      ? { id: 'contact', label: '联系人', icon: <ContactGlyph />, onSelect: () => setContactOpen(true) }
      : { id: 'contact', label: '联系人', icon: <ContactGlyph />, disabled: true },
  ]

  const pick = (asFiles: boolean) => (event: ChangeEvent<HTMLInputElement>) => {
    const files = Array.from(event.target.files ?? [])
    event.target.value = '' // picking the same file again must fire change again
    if (files.length > 0) onFiles(files, asFiles)
  }

  return (
    <>
      <IconButton
        ref={triggerRef}
        label="添加附件"
        className="tg-compose__tool"
        disabled={disabled}
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
      >
        <PaperclipGlyph />
      </IconButton>
      <Menu
        open={open}
        onClose={() => setOpen(false)}
        anchor={triggerRef}
        triggerRef={triggerRef}
        placement="top-end"
        offset={8}
        items={items}
        aria-label="附件类型"
      />
      <input ref={mediaInput} type="file" accept="image/*,video/*" multiple hidden onChange={pick(false)} />
      <input ref={fileInput} type="file" multiple hidden onChange={pick(true)} />
      {contactOpen && chatId ? (
        <Suspense fallback={null}>
          <ContactPickerDialog chatId={chatId} onClose={() => setContactOpen(false)} />
        </Suspense>
      ) : null}
      {pollOpen && chatId ? (
        <Suspense fallback={null}>
          <PollCreateDialog open chatId={chatId} onClose={() => setPollOpen(false)} />
        </Suspense>
      ) : null}
    </>
  )
}
