/** A centred service pill (joins, leaves, renames …) on the chat wallpaper. */
export function SystemMessage({ text }: { text: string }) {
  return (
    <div className="tg-message tg-message--service" role="note">
      <span className="tg-service-pill">{text}</span>
    </div>
  )
}
