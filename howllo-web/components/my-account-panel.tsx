"use client";

import { useState } from "react";
import { readStoredBearerToken } from "@/components/dev-auth-panel";
import { updateMe, uploadImage } from "@/lib/api";
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

  async function uploadAvatar(event: React.ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) return;

    const token = readStoredBearerToken(tenantSlug).trim();
    if (!token) {
      setError("Sign in to this workspace before uploading an avatar.");
      return;
    }

    setBusy("avatar");
    setError("");
    setMessage("");
    try {
      const url = await uploadImage(file, token);
      setAvatarUrl(url);
      setMessage("Avatar uploaded. Save profile to keep it.");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Could not upload avatar.");
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
          <div>
            <dt>Identity subject</dt>
            <dd><code>{howlloUser.rooiam_subject}</code></dd>
          </div>
        </dl>
      </article>

      <div className="grid">
        <article className="panel">
          <h2 className="section-title">Edit profile</h2>
          <p className="section-subtitle" style={{ marginTop: "0.45rem" }}>
            Your public Howllo profile lives in this app. Sign-in identity stays tied to your RooIAM subject and cannot be edited here.
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
            <label className="field-label">
              Avatar upload
              <input
                className="field"
                type="file"
                accept="image/*"
                onChange={uploadAvatar}
                disabled={busy !== null}
              />
            </label>
            <label className="field-label">
              Avatar URL
              <input
                className="field"
                value={avatarUrl}
                onChange={(event) => setAvatarUrl(event.target.value)}
                disabled={busy !== null}
              />
            </label>
            <div className="toolbar">
              <button className="button" type="submit" disabled={busy !== null}>
                {busy === "save" ? "Saving..." : "Save profile"}
              </button>
            </div>
          </form>
        </article>

        {message ? <div className="notice">{message}</div> : null}
        {error ? <div className="notice notice--error">{error}</div> : null}
      </div>
    </section>
  );
}
