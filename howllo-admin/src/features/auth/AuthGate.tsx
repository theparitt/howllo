import { useState, type FormEvent } from "react";
import { useSession } from "../../lib/session";

// Gate shown before the admin UI. On first run (no password set yet) it asks
// for the bootstrap key + a new password; afterwards it's a plain password
// login. The backend enforces both - this is only the entry surface.

export function AuthGate() {
  const { bootstrapped, setupAvailable, probeError, setup, login } =
    useSession();
  const isSetup = bootstrapped === false;

  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [bootstrapKey, setBootstrapKey] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    setError(null);

    if (isSetup && password !== confirm) {
      setError("Passwords do not match.");
      return;
    }

    setBusy(true);
    try {
      if (isSetup) {
        await setup({ bootstrapKey: bootstrapKey.trim(), password });
      } else {
        await login(password);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Sign in failed.");
    } finally {
      setBusy(false);
    }
  }

  if (probeError) {
    return (
      <div className="auth-screen">
        <div className="auth-card">
          <div className="auth-head">
            <img
              className="auth-mark"
              src="/brand/howllo-logo-wordmark-horizontal.svg"
              alt="Howllo"
            />
            <h1>Server unreachable</h1>
            <p className="muted">The admin panel could not contact the backend.</p>
          </div>
          <div className="auth-notice">{probeError}</div>
        </div>
      </div>
    );
  }

  return (
    <div className="auth-screen">
      <form className="auth-card" onSubmit={onSubmit}>
        <div className="auth-head">
          <img
            className="auth-mark"
            src="/brand/howllo-logo-wordmark-horizontal.svg"
            alt="Howllo"
          />
          <h1>{isSetup ? "Set up admin" : "Admin sign in"}</h1>
          <p className="muted">
            {isSetup
              ? "Choose a password to secure the admin panel."
              : "Enter your admin password to continue."}
          </p>
        </div>

        {isSetup && !setupAvailable ? (
          <div className="auth-notice">
            Setup is disabled. Set <code>HOWLLO_ADMIN_BOOTSTRAP_KEY</code> in the
            server environment, then reload this page.
          </div>
        ) : (
          <>
            {isSetup ? (
              <label className="auth-field">
                <span>Bootstrap key</span>
                <input
                  type="text"
                  autoComplete="off"
                  value={bootstrapKey}
                  onChange={(e) => setBootstrapKey(e.target.value)}
                  placeholder="From HOWLLO_ADMIN_BOOTSTRAP_KEY"
                  required
                />
              </label>
            ) : null}

            <label className="auth-field">
              <span>{isSetup ? "New password" : "Password"}</span>
              <input
                type="password"
                autoComplete={isSetup ? "new-password" : "current-password"}
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder={isSetup ? "At least 12 characters" : "Password"}
                minLength={isSetup ? 12 : undefined}
                required
              />
            </label>

            {isSetup ? (
              <label className="auth-field">
                <span>Confirm password</span>
                <input
                  type="password"
                  autoComplete="new-password"
                  value={confirm}
                  onChange={(e) => setConfirm(e.target.value)}
                  placeholder="Re-enter password"
                  required
                />
              </label>
            ) : null}

            {error ? <div className="auth-error">{error}</div> : null}

            <button className="auth-submit" type="submit" disabled={busy}>
              {busy
                ? "Working..."
                : isSetup
                  ? "Set password & continue"
                  : "Sign in"}
            </button>
          </>
        )}
      </form>
    </div>
  );
}
