import { useEffect, useRef, useState } from "react";
import { admin } from "@howllo/api-client";
import type { BuildInfoResponse } from "@howllo/types";
import { useSession } from "../lib/session";
import { useEscapeKey, useFocusTrap } from "../lib/a11y";

// Quiet lower-right build info showing the running version + build time.
// Environment details stay hidden behind the existing click sequence.

function shortBuilt(builtAt: string): string {
  // "2026-06-05T09:30:00Z" -> "2026-06-05 09:30 UTC"
  const m = builtAt.match(/^(\d{4}-\d{2}-\d{2})T(\d{2}:\d{2})/);
  return m ? `${m[1]} ${m[2]} UTC` : builtAt;
}

export function DebugBadge() {
  const { client, isAuthenticated } = useSession();
  const [info, setInfo] = useState<BuildInfoResponse | null>(null);
  const [clicks, setClicks] = useState(0);
  const [open, setOpen] = useState(false);
  const dialogRef = useRef<HTMLDivElement>(null);

  useEscapeKey(open, () => setOpen(false));
  useFocusTrap(open, dialogRef);

  useEffect(() => {
    if (!isAuthenticated) return;
    let cancelled = false;
    admin(client)
      .getBuildInfo()
      .then((data) => {
        if (!cancelled) setInfo(data);
      })
      .catch(() => {
        /* badge stays minimal if build info is unavailable */
      });
    return () => {
      cancelled = true;
    };
  }, [client, isAuthenticated]);

  if (!isAuthenticated) return null;

  const onBadgeClick = () => {
    const next = clicks + 1;
    if (next >= 5) {
      setOpen(true);
      setClicks(0);
    } else {
      setClicks(next);
    }
  };

  const version = info?.version ?? "...";
  const built = info ? shortBuilt(info.built_at) : "loading...";
  return (
    <>
      <button
        type="button"
        className="debug-badge"
        onClick={onBadgeClick}
        title="Build information"
      >
        <span className="debug-badge__text">
          v{version}
          <span className="debug-badge__sub">{built}</span>
        </span>
      </button>

      {open ? (
        <div className="debug-overlay" onClick={() => setOpen(false)}>
          <div
            className="debug-panel"
            role="dialog"
            aria-modal="true"
            ref={dialogRef}
            tabIndex={-1}
            onClick={(e) => e.stopPropagation()}
          >
            <div className="debug-panel__head">
              <div>
                <strong>Environment</strong>
                <p className="muted small">
                  Effective values the server is using. Secrets are masked.
                </p>
              </div>
              <button
                className="debug-panel__close"
                aria-label="Close"
                onClick={() => setOpen(false)}
              >
                x
              </button>
            </div>

            <dl className="debug-env">
              <div className="debug-env__row">
                <dt>version</dt>
                <dd>{info?.version}</dd>
              </div>
              <div className="debug-env__row">
                <dt>built_at</dt>
                <dd>{info?.built_at}</dd>
              </div>
              {info?.env.map((item) => (
                <div className="debug-env__row" key={item.key}>
                  <dt>{item.key}</dt>
                  <dd>{item.value}</dd>
                </div>
              ))}
            </dl>
          </div>
        </div>
      ) : null}
    </>
  );
}
