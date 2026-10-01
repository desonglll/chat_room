/**
 * TG-1206: the media panel's «表情» tab — Unicode emoji and, beside them, the viewer's
 * installed custom emoji sets (TG-304's `CustomEmojiTab`). A custom pick goes through the
 * composer controller so the draft keeps the entity, not just the fallback character.
 */
import { Tabs } from '@tg/ui'
import { CustomEmojiTab, type PickedCustomEmoji } from '../customEmoji'
import { EmojiPanel } from './EmojiPanel'
import { t } from '../../i18n/index'

export interface ComposerEmojiTabProps {
  onPick(emoji: string): void
  onPickCustom(emoji: PickedCustomEmoji): void
}

export function ComposerEmojiTab({ onPick, onPickCustom }: ComposerEmojiTabProps) {
  return (
    <Tabs
      variant="segmented"
      stretch
      activation="manual"
      aria-label={t('w.composer.emojiKinds')}
      className="tg-compose__emoji-kinds"
      items={[
        { id: 'unicode', label: t('w.composer.emojiUnicode') },
        { id: 'custom', label: t('w.composer.emojiCustom') },
      ]}
      panels={{
        unicode: <EmojiPanel onPick={onPick} />,
        custom: (
          <div className="tg-compose__emoji tg-compose__emoji--custom">
            <CustomEmojiTab onPick={onPickCustom} />
          </div>
        ),
      }}
    />
  )
}
