"use client";

import { useEffect, useState } from "react";
import { AvatarCropper } from "@/components/avatar-cropper";
import { clearAllStoredBearerTokens, readAccountToken, readStoredBearerToken } from "@/components/dev-auth-panel";
import { updateMe, uploadImage } from "@/lib/api";
import { changeLocalPassword, getLocalAccountStatus, rotateLocalRecoveryCode } from "@/lib/auth-api";
import type { CurrentUser } from "@/lib/types";

type MyAccountPanelProps = {
  howlloUser: CurrentUser;
  tenantSlug: string;
};

export function MyAccountPanel({ howlloUser, tenantSlug }: MyAccountPanelProps) {
  const [displayName, setDisplayName] = useState(howlloUser.display_name);
  const [avatarUrl, setAvatarUrl] = useState(howlloUser.avatar_url ?? "");
  const [busy, setBusy] = useState<"save" | "avatar" | null>(null);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const [localAccount, setLocalAccount] = useState(false);
  const [hasRecoveryCode, setHasRecoveryCode] = useState(false);
  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [confirmNewPassword, setConfirmNewPassword] = useState("");
  const [recoveryPassword, setRecoveryPassword] = useState("");
  const [recoveryCode, setRecoveryCode] = useState("");
  const [passwordBusy, setPasswordBusy] = useState(false);

  useEffect(() => {
    const token = readAccountToken().trim();
    if (!token) return;
    let cancelled = false;
    getLocalAccountStatus(token).then((status) => {
      if (!cancelled) {
        setLocalAccount(status.has_local_credentials);
        setHasRecoveryCode(status.has_recovery_code);
      }
    }).catch(() => {});
    return () => { cancelled = true; };
  }, []);

  async function changePassword(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (newPassword !== confirmNewPassword) {
      setError("Passwords do not match.");
      return;
    }
    setPasswordBusy(true);
    setError("");
    try {
      await changeLocalPassword(readAccountToken().trim(), currentPassword, newPassword);
      setCurrentPassword("");
      setNewPassword("");
      setConfirmNewPassword("");
      clearAllStoredBearerTokens();
      window.location.assign("/");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Could not change password.");
    } finally {
      setPasswordBusy(false);
    }
  }

  async function rotateRecoveryCode(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setPasswordBusy(true);
    setError("");
    setRecoveryCode("");
    try {
      setRecoveryCode(await rotateLocalRecoveryCode(readAccountToken().trim(), recoveryPassword));
      setRecoveryPassword("");
      setHasRecoveryCode(true);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Could not create recovery code.");
    } finally {
      setPasswordBusy(false);
    }
  }

  async function saveProfile(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const token = readStoredBearerToken(tenantSlug).trim();
    if (!token) {
      setError("Sign in to this workspace before editing your profile.");
      return;
    }

    setBusy("save");
    setError("");
    setMessage("");
    try {
      const updated = await updateMe({
        token,
        displayName,
        avatarUrl: avatarUrl.trim() || null,
      });
      setDisplayName(updated.display_name);
      setAvatarUrl(updated.avatar_url ?? "");
      setMessage("Profile updated.");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Could not update profile.");
    } finally {
      setBusy(null);
    }
  }

  async function uploadAvatar(file: File) {
    const token = readStoredBearerToken(tenantSlug).trim();
    if (!token) {
      throw new Error("Sign in to this workspace before uploading an avatar.");
    }

    setBusy("avatar");
    setError("");
    setMessage("");
    try {
      const url = await uploadImage(file, token, tenantSlug);
      const updated = await updateMe({ token, displayName, avatarUrl: url });
      setAvatarUrl(updated.avatar_url ?? "");
      setMessage("Avatar updated.");
    } finally {
      setBusy(null);
    }
  }

  const avatarInitial = (displayName.trim() || howlloUser.email || "?").slice(0, 1).toUpperCase();

  return (
    <section className="my-grid">
      <article className="panel profile-card">
        <div className="avatar-preview" aria-hidden="true">
          {avatarUrl ? <img src={avatarUrl} alt="" /> : avatarInitial}
        </div>
        <div className="stack stack--tight">
          <h2 className="section-title">{displayName || "Unnamed user"}</h2>
          <p className="section-subtitle">{howlloUser.email}</p>
        </div>
        <dl className="detail-list">
          <div>
            <dt>Howllo user ID</dt>
            <dd><code>{howlloUser.id}</code></dd>
          </div>
          <div>
            <dt>Workspace session</dt>
            <dd>{tenantSlug}</dd>
          </div>
        </dl>
      </article>

      <div className="grid">
        <article className="panel">
          <h2 className="section-title">Edit profile</h2>
          <p className="section-subtitle" style={{ marginTop: "0.45rem" }}>
            Your public Howllo profile lives in this app. Your sign-in identity is managed by your login provider.
          </p>
          <form className="field-grid" style={{ marginTop: "1rem" }} onSubmit={saveProfile}>
            <label className="field-label">
              Display name
              <input
                className="field"
                value={displayName}
                onChange={(event) => setDisplayName(event.target.value)}
                disabled={busy !== null}
              />
            </label>
            <AvatarCropper disabled={busy !== null} onCrop={uploadAvatar} onError={setError} />
            <div className="toolbar">
              <button className="button" type="submit" disabled={busy !== null}>
                {busy === "save" ? "Saving..." : "Save profile"}
              </button>
            </div>
          </form>
        </article>

        {localAccount ? <article className="panel">
          <h2 className="section-title">Local account security</h2>
          <p className="section-subtitle" style={{ marginTop: "0.45rem" }}>Changing your password signs out all your devices. Keep your recovery code offline; it is shown only when created.</p>
          <form className="field-grid" style={{ marginTop: "1rem" }} onSubmit={(event) => void changePassword(event)}>
            <label className="field-label">Current password<input className="field" type="password" autoComplete="current-password" value={currentPassword} onChange={(event) => setCurrentPassword(event.target.value)} required /></label>
            <label className="field-label">New password<input className="field" type="password" autoComplete="new-password" minLength={12} value={newPassword} onChange={(event) => setNewPassword(event.target.value)} required /></label>
            <label className="field-label">Confirm new password<input className="field" type="password" autoComplete="new-password" minLength={12} value={confirmNewPassword} onChange={(event) => setConfirmNewPassword(event.target.value)} required /></label>
            <button className="button" type="submit" disabled={passwordBusy}>Change password</button>
          </form>
          <form className="field-grid" style={{ marginTop: "1.5rem" }} onSubmit={(event) => void rotateRecoveryCode(event)}>
            <h3 className="section-title">Recovery code</h3>
            <p className="section-subtitle">{hasRecoveryCode ? "Creating a new code invalidates the previous one." : "Create a code so you can recover this account if you forget your password."}</p>
            <label className="field-label">Current password<input className="field" type="password" autoComplete="current-password" value={recoveryPassword} onChange={(event) => setRecoveryPassword(event.target.value)} required /></label>
            <button className="button" type="submit" disabled={passwordBusy}>Create new recovery code</button>
          </form>
          {recoveryCode ? <div className="notice" role="status" style={{ marginTop: "1rem" }}>
            <p>Save this code now. It will not be shown again.</p>
            <code style={{ overflowWrap: "anywhere", userSelect: "all" }}>{recoveryCode}</code>
            <button className="button" type="button" onClick={() => void navigator.clipboard.writeText(recoveryCode)}>Copy code</button>
          </div> : null}
        </article> : null}

        {message ? <div className="notice">{message}</div> : null}
        {error ? <div className="notice notice--error">{error}</div> : null}
      </div>
    </section>
  );
}
