import { useEffect, useState } from 'react'
import { Avatar, AVATAR_PALETTE_SLOTS } from '../../primitives/Avatar'
import { Badge } from '../../primitives/Badge'
import { Button } from '../../primitives/Button'
import { Skeleton } from '../../primitives/Skeleton'
import { Spinner } from '../../primitives/Spinner'
import { Row, Section, Stack, State, type DocPage } from '../kit'

const NAMES = ['Ada Lovelace', 'Grace Hopper', 'Alan Turing', 'Katherine Johnson', 'Radia Perlman']

function AvatarDemo() {
  return (
    <>
      <Section title="Sizes">
        <Row>
          {(['sm', 'md', 'lg', 'xl'] as const).map((size) => (
            <Avatar key={size} label="Ada Lovelace" size={size} />
          ))}
          <Avatar label="Custom 96" size={96} />
        </Row>
      </Section>
      <Section title="Initials from the label, and the forum shape">
        <Row>
          {NAMES.map((name) => (
            <Avatar key={name} label={name} />
          ))}
          <Avatar label="Topic" shape="forum" />
        </Row>
      </Section>
      <Section title="Palette slots (all seven resolve to the accent until the token patch lands)">
        <Row>
          {Array.from({ length: AVATAR_PALETTE_SLOTS }, (_, index) => (
            <Avatar key={index} label={`Slot ${index}`} colorIndex={index} />
          ))}
        </Row>
      </Section>
      <Section title="Image, broken image fallback, online dot, badge">
        <Row>
          <Avatar
            label="1x1 placeholder image"
            src="data:image/gif;base64,R0lGODlhAQABAIAAAP///wAAACwAAAAAAQABAAACAkQBADs="
          />
          <Avatar label="Broken source" src="/does-not-exist.png" />
          <Avatar label="Ada Lovelace" online />
          <Avatar label="Ada Lovelace" badge={<Badge count={12} />} />
        </Row>
      </Section>
    </>
  )
}

function BadgeDemo() {
  const [hidden, setHidden] = useState(false)
  return (
    <>
      <Section title="Counts and the cap">
        <Row>
          <Badge count={1} />
          <Badge count={12} />
          <Badge count={99} />
          <Badge count={1284} />
          <Badge count={1284} max={0} />
        </Row>
      </Section>
      <Section title="Variants">
        <Row>
          {(['accent', 'muted', 'danger', 'success'] as const).map((variant) => (
            <Badge key={variant} variant={variant} count={7} />
          ))}
        </Row>
      </Section>
      <Section title="Dots and sizes">
        <Row>
          <Badge dot label="Unread" />
          <Badge dot size="sm" label="Unread, small" />
          <Badge size="sm" count={5} />
          <Badge>NEW</Badge>
        </Row>
      </Section>
      <Section title="Enter and exit (--tg-ease-badge-in / --tg-ease-badge-out)">
        <Row>
          <Button variant="tonal" size="sm" onClick={() => setHidden((v) => !v)}>
            toggle
          </Button>
          <Badge count={3} hidden={hidden} />
        </Row>
      </Section>
    </>
  )
}

function SpinnerDemo() {
  const [progress, setProgress] = useState(0.1)
  useEffect(() => {
    const timer = window.setInterval(() => setProgress((p) => (p >= 1 ? 0 : Number((p + 0.1).toFixed(2)))), 700)
    return () => window.clearInterval(timer)
  }, [])

  return (
    <>
      <Section title="Indeterminate: sizes and tints">
        <Row>
          {(['sm', 'md', 'lg'] as const).map((size) => (
            <Spinner key={size} size={size} label={`Loading ${size}`} />
          ))}
          <Spinner size={56} label="Loading 56" />
          <span className="doc-inverse-plate">
            <Spinner tint="inverse" label="Loading, inverse" />
          </span>
          <Spinner tint="muted" label="Loading, muted" />
        </Row>
      </Section>
      <Section title="Determinate">
        <Row>
          <Spinner label="Uploading" progress={progress} />
          <Spinner label="Half" progress={0.5} size="lg" />
          <Spinner label="Complete" progress={1} size="lg" />
          <span className="doc-note">{Math.round(progress * 100)}%</span>
        </Row>
      </Section>
      <Section title="Inside a Button">
        <Row>
          <Button loading>Sending</Button>
          <Button variant="tonal" loading>
            Sending
          </Button>
        </Row>
      </Section>
    </>
  )
}

function SkeletonDemo() {
  return (
    <>
      <Section title="Variants">
        <Stack>
          <State label="text">
            <Skeleton />
          </State>
          <State label="text, 3 lines">
            <Skeleton lines={3} />
          </State>
          <State label="rect">
            <Skeleton variant="rect" height={96} />
          </State>
          <State label="circle">
            <Skeleton variant="circle" />
          </State>
          <State label="not animated">
            <Skeleton variant="rect" height={48} animated={false} />
          </State>
        </Stack>
      </Section>
      <Section title="A list row placeholder, assembled from the primitives">
        <Stack>
          {[0, 1, 2].map((row) => (
            <div key={row} className="doc-skeleton-row">
              <Skeleton variant="circle" />
              <span className="doc-skeleton-row__text">
                <Skeleton width="40%" />
                <Skeleton width="80%" />
              </span>
            </div>
          ))}
        </Stack>
      </Section>
    </>
  )
}

export const displayPages: DocPage[] = [
  {
    id: 'avatar',
    title: 'Avatar',
    summary: 'Circular or forum-rounded image with an initials fallback.',
    notes: [
      'Generic by contract: it takes src and label, never a user or a chat. label is the accessible name, the initials source and the palette seed.',
      'INTEGRATION PATCH PENDING: the fallback palette needs seven colours and TG-009 froze 197 names without them. The component reads --tg-avatar-1..7 with var() fallbacks to --tg-accent, so today all seven slots look identical. The exact patch is in docs/devlog/TG-010.md.',
      'Initials are grapheme-aware, so an emoji or a surrogate pair is not cut in half.',
      'The online dot is decorative under the TG-606 contrast ruling (--tg-online is 2.42:1 on white); the host also carries data-tg-online as a non-colour signal, and the dot has a surface-coloured ring so it never merges into the avatar.',
    ],
    Demo: AvatarDemo,
  },
  {
    id: 'badge',
    title: 'Badge',
    summary: 'Counter or status pill. Counts anything; knows what it counts of nothing.',
    notes: [
      'The visible text truncates to "99+" but aria-label carries the exact number, so a screen reader user is never told the cap.',
      'A dot has no text, so label is required; without it the badge is aria-hidden rather than announced as an empty status.',
      "Motion: --tg-ease-badge-in and --tg-ease-badge-out are tweb's own --chatlist-badge-transition-in / -out. `hidden` keeps the element mounted so the exit curve can play.",
    ],
    Demo: BadgeDemo,
  },
  {
    id: 'spinner',
    title: 'Spinner',
    summary: 'Indeterminate or determinate progress ring.',
    notes: [
      'Decorative by default: with no label it is aria-hidden, which is right inside a control that is already aria-busy. With a label it becomes role="status", or role="progressbar" with aria-valuenow when determinate.',
      'REDUCED MOTION IS DIFFERENT HERE, on purpose. A frozen spinner reads as "hung", and TG-009\'s movement durations collapse to 1ms, which would strobe. The rotation therefore runs off a dedicated loop period and only the dash pulse stops.',
      "INTEGRATION PATCH PENDING: --tg-duration-loop. TG-009's nine durations are all one-shot transition durations; a continuous animation needs a period. Spinner and Skeleton both fall back to a single literal declared once per file until the patch lands.",
    ],
    Demo: SpinnerDemo,
  },
  {
    id: 'skeleton',
    title: 'Skeleton',
    summary: 'Loading placeholder.',
    notes: [
      'Always aria-hidden: it is decoration, and the region it fills should carry aria-busy so assistive technology hears "busy" rather than a wall of nothing.',
      'The sheen is a --tg-surface-raised gradient faded through `transparent`, so it needs no colour literal in either theme.',
      'Reduced motion: the sheen stops travelling and the block stays visible. Stopping is the correct degradation for pure decoration.',
      'lines > 1 makes the last line short, because a stack of equal bars does not read as text.',
    ],
    Demo: SkeletonDemo,
  },
]
