"use client";

import { useEffect, useState } from "react";
import { API_BASE_URL } from "@/lib/config";
import { readStoredBearerToken } from "@/components/dev-auth-panel";
import type { BoardCategory, Tag } from "@/lib/types";

const slugFor = (value: string) => value.trim().toLowerCase().normalize("NFKD")
  .replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "").slice(0, 64);

export function BoardTaxonomyEditor({ boardId, tenant }: { boardId: string; tenant: string }) {
  const [categories, setCategories] = useState<BoardCategory[]>([]);
  const [tags, setTags] = useState<Tag[]>([]);
  const [categoryName, setCategoryName] = useState("");
  const [tagName, setTagName] = useState("");
  const [categoryColor, setCategoryColor] = useState("#64748b");
  const [tagColor, setTagColor] = useState("#64748b");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const boardUrl = `${API_BASE_URL}/api/admin/boards/${encodeURIComponent(boardId)}/categories`;
  const tagsUrl = `${API_BASE_URL}/api/admin/tags?tenant_slug=${encodeURIComponent(tenant)}`;
  const token = () => readStoredBearerToken(tenant).trim();

  async function refresh() {
    const [categoryResponse, tagResponse] = await Promise.all([
      fetch(boardUrl, { headers: { Authorization: token() }, cache: "no-store" }),
      fetch(tagsUrl, { headers: { Authorization: token() }, cache: "no-store" }),
    ]);
    if (!categoryResponse.ok || !tagResponse.ok) throw new Error("Could not load categories and tags.");
    setCategories(await categoryResponse.json() as BoardCategory[]);
    setTags(await tagResponse.json() as Tag[]);
  }

  useEffect(() => { void refresh().catch((cause) => setError(cause instanceof Error ? cause.message : "Could not load categories."));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [boardId, tenant]);

  async function write(url: string, method: string, body?: object) {
    setBusy(true); setError("");
    try {
      const response = await fetch(url, { method, headers: { Authorization: token(), ...(body ? { "content-type": "application/json" } : {}) }, body: body ? JSON.stringify(body) : undefined });
      if (!response.ok) throw new Error("Could not save. Check the name and try again.");
      await refresh();
      return true;
    } catch (cause) { setError(cause instanceof Error ? cause.message : "Could not save."); return false; }
    finally { setBusy(false); }
  }

  return <section className="board-taxonomy">
    <div><h3 className="section-title">Categories</h3><p className="section-subtitle">People choose one category when posting. Categories belong to this board.</p></div>
    {categories.length ? <div className="board-taxonomy__chips">{categories.map((category) => <span className="board-taxonomy__chip" key={category.id}>
      <input className="board-taxonomy__color" type="color" value={category.color} aria-label={`Color for ${category.name}`} disabled={busy} onChange={(event) => void write(`${boardUrl}/${category.id}`, "PATCH", { name: category.name, color: event.target.value })} />{category.name}
      <button type="button" disabled={busy} aria-label={`Rename ${category.name}`} onClick={() => {
        const name = window.prompt("Category name", category.name)?.trim();
        if (name && name !== category.name) void write(`${boardUrl}/${category.id}`, "PATCH", { name, color: category.color });
      }}>Edit</button>
      <button type="button" disabled={busy} aria-label={`Delete ${category.name}`} onClick={() => {
        if (window.confirm(`Delete ${category.name}? Existing posts will have no category.`)) void write(`${boardUrl}/${category.id}`, "DELETE");
      }}>×</button>
    </span>)}</div> : <p className="section-subtitle">No categories yet.</p>}
    <form className="board-taxonomy__add" onSubmit={(event) => {
      event.preventDefault(); const name = categoryName.trim(); const slug = slugFor(name);
      if (!slug) { setError("Use a category name with Latin letters or numbers for its URL."); return; }
      void write(boardUrl, "POST", { name, slug, color: categoryColor }).then((saved) => { if (saved) setCategoryName(""); });
    }}><input className="manage-input" value={categoryName} maxLength={80} onChange={(event) => setCategoryName(event.target.value)} placeholder="Add category, e.g. Mobile app" aria-label="New category" required /><input type="color" value={categoryColor} aria-label="Category color" onChange={(event) => setCategoryColor(event.target.value)} /><button className="button" disabled={busy} type="submit">Add</button></form>

    <div><h3 className="section-title">Tags</h3><p className="section-subtitle">Tags are shared across this workspace. People can add up to three to a post.</p></div>
    {tags.length ? <div className="board-taxonomy__chips">{tags.map((tag) => <span className="board-taxonomy__chip" key={tag.id}><input className="board-taxonomy__color" type="color" value={tag.color ?? "#64748b"} aria-label={`Color for ${tag.name}`} disabled={busy} onChange={(event) => void write(`${API_BASE_URL}/api/admin/tags/${tag.id}`, "PATCH", { name: tag.name, color: event.target.value })} />{tag.name}<button type="button" disabled={busy} aria-label={`Rename ${tag.name}`} onClick={() => {
      const name = window.prompt("Tag name", tag.name)?.trim();
      if (name && name !== tag.name) void write(`${API_BASE_URL}/api/admin/tags/${tag.id}`, "PATCH", { name, color: tag.color });
    }}>Edit</button><button type="button" disabled={busy} aria-label={`Delete ${tag.name}`} onClick={() => {
      if (window.confirm(`Delete tag ${tag.name} from the whole workspace?`)) void write(`${API_BASE_URL}/api/admin/tags/${tag.id}`, "DELETE");
    }}>×</button></span>)}</div> : <p className="section-subtitle">No tags yet.</p>}
    <form className="board-taxonomy__add" onSubmit={(event) => {
      event.preventDefault(); const name = tagName.trim(); const slug = slugFor(name);
      if (!slug) { setError("Use a tag name with Latin letters or numbers for its URL."); return; }
      void write(`${API_BASE_URL}/api/admin/tags`, "POST", { tenant_slug: tenant, name, slug, color: tagColor }).then((saved) => { if (saved) setTagName(""); });
    }}><input className="manage-input" value={tagName} maxLength={80} onChange={(event) => setTagName(event.target.value)} placeholder="Add tag, e.g. Accessibility" aria-label="New tag" required /><input type="color" value={tagColor} aria-label="Tag color" onChange={(event) => setTagColor(event.target.value)} /><button className="button" disabled={busy} type="submit">Add</button></form>
    {error ? <p className="error-text" role="alert">{error}</p> : null}
  </section>;
}
