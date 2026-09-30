/**
 * The paperclip: a menu of attachment kinds. Photo/video and file open the native file
 * picker; location, poll and contact are present but DISABLED (not hidden) until their
 * tasks (TG-407, TG-406, M4) enable them — Telegram's menu shape stays stable.
 */
import { useRef, useState, type ChangeEvent } from 'react'
import type { MenuItem } from '@tg/ui'
import { IconButton, Menu } from '@tg/ui'
import { ContactGlyph, FileGlyph, LocationGlyph, PaperclipGlyph, PhotoGlyph, PollGlyph } from './icons'

export interface AttachMenuProps {
  disabled?: boolean
  /** `asFiles`: picked through «文件», sent without photo/video treatment. */
  onFiles(files: File[], asFiles: boolean): void
}

export function AttachMenu({ disabled = false, onFiles }: AttachMenuProps) {
  const [open, setOpen] = useState(false)
  const triggerRef = useRef<HTMLButtonElement>(null)
  const mediaInput = useRef<HTMLInputElement>(null)
  const fileInput = useRef<HTMLInputElement>(null)

  const items: MenuItem[] = [
    { id: 'media', label: '图片或视频', icon: <PhotoGlyph />, onSelect: () => mediaInput.current?.click() },
    { id: 'file', label: '文件', icon: <FileGlyph />, onSelect: () => fileInput.current?.click() },
    { id: 'location', label: '位置', icon: <LocationGlyph />, disabled: true, separatorBefore: true },
    { id: 'poll', label: '投票', icon: <PollGlyph />, disabled: true },
    { id: 'contact', label: '联系人', icon: <ContactGlyph />, disabled: true },
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
    </>
  )
}
