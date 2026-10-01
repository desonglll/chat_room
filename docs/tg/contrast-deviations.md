# Contrast deviations (TG-606)

Measured from the token files by `packages/ui/src/test/contrast.test.ts` (WCAG relative-luminance
ratio, alpha composited over the background). The test fails if any **strict** pair drops below
4.5:1 or any deviation below moves by more than ±0.05 — a change here is always a conscious edit.

## Strict pairs — all ≥ 4.5:1 in day and night

Body text, secondary text, links, danger text and both bubbles' text on their backgrounds (except
the night outgoing bubble, below). TG-606 adjusted two **text** tokens to get there, keeping the
hue and leaving the corresponding fills untouched:

| token | theme | Telegram value → ours | ratio before → after |
| --- | --- | --- | --- |
| `--tg-text-danger` | day | `#df3f40` → `#dc2e2f` (HSL L −4) | 4.27 → 4.69 |
| `--tg-text-link` | night | `#8774e1` → `#9585e5` (HSL L +4) | 4.31 → 5.19 |

## Deviations kept on purpose (integration lead ruling, 2026-09-30)

| theme | foreground on background | ratio | used for | why it stays |
| --- | --- | --- | --- | --- |
| day | `--tg-text-on-accent` on `--tg-accent` | 3.31 | primary buttons, selected chat row, unread badge | Telegram's accent fill `#3390ec` is the visual identity |
| night | `--tg-text-on-accent` on `--tg-accent` | 3.74 | the same, night | Telegram's night accent `#8774e1` |
| night | `--tg-bubble-out-text` on `--tg-bubble-out` | 3.74 | text in outgoing bubbles | the outgoing bubble is an accent fill |
| day | `--tg-bubble-out-meta` on `--tg-bubble-out` | 2.75 | time and ticks in outgoing bubbles | meta; ticks carry the state by shape, the time is also in the message menu |
| day | `--tg-text-tertiary` on `--tg-surface` | 2.64 | input placeholders | decorative hint; every field has a visible label or `aria-label` |
| night | `--tg-text-tertiary` on `--tg-surface` | 4.98 | input placeholders | meets AA at night; listed for symmetry |

Re-evaluation trigger: a compliance requirement would add a high-contrast theme overlay (the token
layers already support one) rather than change the default theme.
