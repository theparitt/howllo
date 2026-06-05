// Board icon tile. Shows the board's uploaded image, or falls back to the
// Howllo fox mark on a soft tint when no icon is set.
export function BoardIcon({
  iconUrl,
  size = 40,
}: {
  iconUrl?: string | null;
  size?: number;
}) {
  return (
    <span
      className={iconUrl ? "board-icon" : "board-icon board-icon--default"}
      style={{ width: size, height: size }}
    >
      <img
        src={iconUrl || "/brand/howllo-logo.svg"}
        alt=""
        aria-hidden
      />
    </span>
  );
}
