import Link from "next/link";
import { AuthControl } from "@/components/auth-control";

export function Header() {
  return (
    <header className="site-header">
      <div className="site-header__inner">
        <div className="brand">
          <Link href="/" className="brand__title" aria-label="Howllo">
            <img
              alt="Howllo"
              src="/brand/howllo-logo-wordmark-horizontal.svg"
              style={{ height: "1.7rem", width: "7.5rem", display: "block", objectFit: "contain" }}
            />
          </Link>
          <div className="brand__meta">Feedback &amp; feature requests, out in the open.</div>
        </div>
        <nav className="nav">
          <Link href="/dashboard" className="nav__link">
            Dashboard
          </Link>
          <Link href="/roadmap" className="nav__link">
            Roadmap
          </Link>
          <AuthControl />
        </nav>
      </div>
    </header>
  );
}
