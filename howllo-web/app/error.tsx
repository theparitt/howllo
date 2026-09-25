"use client";

import { useEffect } from "react";

/**
 * Route-level error boundary. Catches any error thrown while rendering a page
 * (including a backend that is unreachable) and shows a friendly state with a
 * retry, instead of Next.js's raw runtime error overlay.
 */
export default function PageError({
  error,
  reset,
}: {
  error: Error & { digest?: string };
  reset: () => void;
}) {
  useEffect(() => {
    console.error(error);
  }, [error]);

  // A backend that is down surfaces as a fetch/network failure.
  const message = error.message ?? "";
  const isUnavailable =
    error.name === "ApiUnavailableError" ||
    /fetch failed|Failed to fetch|ECONNREFUSED|NetworkError|network/i.test(message);

  return (
    <div className="page-stack">
      <section className="panel empty-state">
        <span className="kicker">{isUnavailable ? "Server unavailable" : "Something went wrong"}</span>
        <h1 className="empty-state__title">
          {isUnavailable
            ? "We can’t reach the Howllo server right now."
            : "This page hit an unexpected error."}
        </h1>
        <p className="empty-state__copy">
          {isUnavailable
            ? "The backend may be starting up or temporarily offline. Please try again in a moment."
            : "Try again, and if it keeps happening, refresh the page."}
        </p>
        {isUnavailable ? (
          <p className="workspace-state__hint">
            Expecting a local server? Start it on <code>127.0.0.1:7700</code>.
          </p>
        ) : null}
        <div className="inline-actions" style={{ marginTop: "1rem" }}>
          <button type="button" className="button" onClick={() => reset()}>
            Try again
          </button>
        </div>
      </section>
    </div>
  );
}
