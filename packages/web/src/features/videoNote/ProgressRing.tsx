/**
 * A circular progress stroke around a round video (TG-402): the recording's minute in the
 * viewfinder, the playback position in the bubble. Pure SVG; `progress` is 0..1.
 */
export interface ProgressRingProps {
  progress: number
  className?: string
}

/** Circle radius in the 100-unit viewBox; the stroke sits just inside the edge. */
const RADIUS = 48

export function ProgressRing({ progress, className }: ProgressRingProps) {
  const circumference = 2 * Math.PI * RADIUS
  const clamped = Math.min(1, Math.max(0, progress))
  return (
    <svg className={className} viewBox="0 0 100 100" aria-hidden="true" focusable="false">
      <circle
        cx="50"
        cy="50"
        r={RADIUS}
        fill="none"
        stroke="currentColor"
        strokeWidth="2.5"
        strokeLinecap="round"
        strokeDasharray={circumference}
        strokeDashoffset={circumference * (1 - clamped)}
        transform="rotate(-90 50 50)"
      />
    </svg>
  )
}
