import { useState } from "react";

// Compact, copyable identifier chip for UUIDs / request IDs in tables.
// Shows a shortened form (e.g. 61dc657e...202e55) and a copy button that copies
// the full value, with brief "copied" feedback. Title shows the full value.

function compact(value: string, head = 6, tail = 4): string {
  if (value.length <= head + tail + 1) return value;
  return `${value.slice(0, head)}...${value.slice(-tail)}`;
}

export function IdChip({ value }: { value: string | null | undefined }) {
  const [copied, setCopied] = useState(false);

  if (!value) return <span className="muted">-</span>;

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1200);
    } catch {
      /* clipboard unavailable; no-op */
    }
  };

  return (
    <button
      type="button"
      className={copied ? "id-chip is-copied" : "id-chip"}
      title={copied ? "Copied" : `Copy ${value}`}
      onClick={copy}
    >
      <span className="id-chip__text">{compact(value)}</span>
      <span className="id-chip__icon" aria-hidden>
        {copied ? (
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round">
            <path d="M20 6L9 17l-5-5" />
          </svg>
        ) : (
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
            <rect x="9" y="9" width="11" height="11" rx="2" />
            <path d="M5 15V5a2 2 0 012-2h10" />
          </svg>
        )}
      </span>
    </button>
  );
}

export default IdChip;
