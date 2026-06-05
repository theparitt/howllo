import { useEffect, useState } from "react";
import { admin } from "@howllo/api-client";
import type { Tag } from "@howllo/types";
import { SessionRequired } from "../../components/SessionRequired";
import { useSession } from "../../lib/session";
import { useAsync } from "../../lib/useAsync";
import { Panel } from "../../components/Panel";
import { StateBlock } from "../../components/StateBlock";

export function TagsPage() {
  const { client, tenant, authorization } = useSession();
  const api = admin(client);
  const tags = useAsync<Tag[]>(
    () => (authorization ? api.listTags() : Promise.resolve([])),
    [tenant, authorization],
  );

  const [name, setName] = useState("");
  const [slug, setSlug] = useState("");
  const [color, setColor] = useState("#6c7bff");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!authorization) {
    return (
      <section>
        <h1>Tags</h1>
        <SessionRequired />
      </section>
    );
  }

  const create = async () => {
    setBusy(true);
    setError(null);
    try {
      await api.createTag({
        name: name.trim(),
        slug: slug.trim(),
        color: color || undefined,
      });
      setName("");
      setSlug("");
      tags.reload();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to create tag");
    } finally {
      setBusy(false);
    }
  };

  return (
    <section>
      <h1>Tags</h1>
      <p className="muted">Workspace-scoped tags for organizing feedback.</p>

      <Panel title="New tag">
        <div className="form-row">
          <label>
            Name
            <input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="Mobile"
            />
          </label>
          <label>
            Slug
            <input
              value={slug}
              onChange={(e) => setSlug(e.target.value)}
              placeholder="mobile"
            />
          </label>
          <label>
            Color
            <input
              type="color"
              value={color}
              onChange={(e) => setColor(e.target.value)}
            />
          </label>
          <button
            className="primary"
            disabled={busy || !name.trim() || !slug.trim()}
            onClick={create}
          >
            {busy ? "Adding..." : "Add"}
          </button>
        </div>
        {error ? <p className="error-text">{error}</p> : null}
      </Panel>

      <Panel title="All tags">
        <StateBlock
          loading={tags.loading}
          error={tags.error}
          empty={tags.data?.length === 0 ? "No tags yet." : null}
        >
          <div className="tag-grid">
            {tags.data?.map((t) => (
              <TagRow key={t.id} tag={t} onChanged={tags.reload} />
            ))}
          </div>
        </StateBlock>
      </Panel>
    </section>
  );
}

function TagRow({ tag, onChanged }: { tag: Tag; onChanged: () => void }) {
  const { client } = useSession();
  const api = admin(client);
  const [name, setName] = useState(tag.name);
  const [color, setColor] = useState(tag.color ?? "#6c7bff");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setName(tag.name);
    setColor(tag.color ?? "#6c7bff");
    setError(null);
  }, [tag]);

  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      await api.updateTag(tag.id, {
        name: name.trim(),
        color: color || undefined,
      });
      onChanged();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to update tag");
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    setBusy(true);
    setError(null);
    try {
      await api.deleteTag(tag.id);
      onChanged();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to delete tag");
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="tag-chip-row">
      <span className="tag-chip" style={color ? { background: color } : undefined}>
        {tag.slug}
      </span>
      <input value={name} onChange={(e) => setName(e.target.value)} />
      <input type="color" value={color} onChange={(e) => setColor(e.target.value)} />
      <button disabled={busy || !name.trim()} onClick={save}>
        Save
      </button>
      <button className="danger small" disabled={busy} onClick={remove}>
        Delete
      </button>
      {error ? <div className="error-text">{error}</div> : null}
    </div>
  );
}
