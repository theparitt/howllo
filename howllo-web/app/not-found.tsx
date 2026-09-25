import Link from "next/link";

export default function NotFound() {
  return (
    <div className="page-stack howl-not-found-wrap">
      <section className="howl-not-found">
        <div className="howl-not-found__art" aria-hidden="true">
          <span className="howl-not-found__orbit howl-not-found__orbit--one" />
          <span className="howl-not-found__orbit howl-not-found__orbit--two" />
          <img src="/brand/howllo-logo.svg" alt="" />
          <span className="howl-not-found__paw">404</span>
        </div>
        <div className="howl-not-found__copy">
          <span className="kicker">Wrong trail</span>
          <h1 className="page-title">This page wandered off.</h1>
          <p className="page-lead">It may have moved, or this board or page may be turned off by its workspace.</p>
          <div className="inline-actions">
            <Link className="button button--cta" href="/">Back to workspace</Link>
            <Link className="ghost-button" href="https://howllo.dev/">Howllo home</Link>
          </div>
        </div>
      </section>
    </div>
  );
}
