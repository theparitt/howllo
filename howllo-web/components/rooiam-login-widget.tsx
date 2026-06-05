"use client";

import { useMemo, useState } from "react";
import {
  ROOIAM_WIDGET_BASE_URL,
  ROOIAM_WIDGET_CLIENT_ID,
  ROOIAM_WIDGET_WORKSPACE_ID,
} from "@/lib/config";

function buildWidgetUrl() {
  const url = new URL(ROOIAM_WIDGET_BASE_URL);
  url.searchParams.set("workspace_id", ROOIAM_WIDGET_WORKSPACE_ID);
  if (ROOIAM_WIDGET_CLIENT_ID.trim()) {
    url.searchParams.set("client_id", ROOIAM_WIDGET_CLIENT_ID.trim());
  }
  return url.toString();
}

export function RooiamLoginWidget() {
  const [open, setOpen] = useState(false);
  const widgetUrl = useMemo(buildWidgetUrl, []);
  const configured = ROOIAM_WIDGET_CLIENT_ID.trim().length > 0;

  return (
    <>
      <button
        className="button"
        type="button"
        onClick={() => setOpen((value) => !value)}
      >
        {open ? "Hide login" : "Sign in"}
      </button>

      {open ? (
        <div className="widget-shell" role="dialog" aria-modal="false" aria-label="Sign in">
          <button
            className="widget-shell__close"
            type="button"
            aria-label="Close"
            onClick={() => setOpen(false)}
          >
            ×
          </button>

          {configured ? (
            <iframe
              key={widgetUrl}
              title="Sign in"
              src={widgetUrl}
              width="420"
              height="520"
              allow="publickey-credentials-get *"
              className="widget-shell__frame"
            />
          ) : (
            <div className="notice notice--error">
              Set <code className="code-hint">NEXT_PUBLIC_ROOIAM_WIDGET_CLIENT_ID</code> to the
              login widget client ID for `howllo-web`.
            </div>
          )}
        </div>
      ) : null}
    </>
  );
}
