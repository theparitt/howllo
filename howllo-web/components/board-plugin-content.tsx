"use client";

import { createElement, useEffect, useRef, useState } from "react";
import ReactMarkdown from "react-markdown";
import { attachmentKind, hasCapability, safeAttachmentUrl, type ActiveBoardPlugins } from "@/lib/board-plugin-runtime";

const MARKDOWN_ELEMENTS = ["p", "br", "strong", "em", "a", "ul", "ol", "li", "blockquote", "code"];

export function PostBody({ text, plugins }: { text: string; plugins: ActiveBoardPlugins }) {
  if (!hasCapability(plugins, "post.body")) return <p className="post-body">{text}</p>;
  return <div className="post-body howllo-markdown"><ReactMarkdown skipHtml allowedElements={MARKDOWN_ELEMENTS} unwrapDisallowed>{text}</ReactMarkdown></div>;
}

export function MarkdownEditor({ label, value, onChange, placeholder, required = false }: {
  label: string; value: string; onChange: (value: string) => void; placeholder?: string; required?: boolean;
}) {
  const input = useRef<HTMLTextAreaElement>(null);
  const [preview, setPreview] = useState(false);

  function wrap(before: string, after = before) {
    const el = input.current;
    if (!el) return;
    const start = el.selectionStart;
    const end = el.selectionEnd;
    const selected = value.slice(start, end) || "text";
    onChange(value.slice(0, start) + before + selected + after + value.slice(end));
    requestAnimationFrame(() => { el.focus(); el.setSelectionRange(start + before.length, start + before.length + selected.length); });
  }

  return <div className="howllo-markdown-editor">
    <span className="manage-label">{label}</span>
    <div className="howllo-markdown-editor__toolbar" role="toolbar" aria-label={`${label} formatting`}>
      <button type="button" onClick={() => wrap("**")} aria-label="Bold"><strong>B</strong></button>
      <button type="button" onClick={() => wrap("*")} aria-label="Italic"><em>I</em></button>
      <button type="button" onClick={() => wrap("[", "](https://example.com)")} aria-label="Link">Link</button>
      <button type="button" onClick={() => wrap("- ", "")} aria-label="List">List</button>
      <button type="button" onClick={() => setPreview((current) => !current)} aria-pressed={preview}>{preview ? "Write" : "Preview"}</button>
    </div>
    <textarea ref={input} className="textarea" value={value} onChange={(event) => onChange(event.target.value)} placeholder={placeholder} aria-label={label} required={required} />
    {preview ? <div className="howllo-markdown-editor__preview howllo-markdown"><ReactMarkdown skipHtml allowedElements={MARKDOWN_ELEMENTS} unwrapDisallowed>{value || "Nothing to preview yet."}</ReactMarkdown></div> : null}
  </div>;
}

export function AttachmentPreview({ url, plugins, compact = false }: { url: string; plugins: ActiveBoardPlugins; compact?: boolean }) {
  const safeUrl = safeAttachmentUrl(url);
  const imageDialog = useRef<HTMLDialogElement>(null);
  const [open, setOpen] = useState(false);
  const [modelReady, setModelReady] = useState(false);
  const kind = attachmentKind(url);
  useEffect(() => {
    if (open && kind === "glb" && hasCapability(plugins, "attachment.preview.glb")) {
      void import("@google/model-viewer").then(() => setModelReady(true));
    }
  }, [open, kind, plugins]);
  if (!safeUrl) return null;
  if (kind === "image") return <>
    <button type="button" className="attachment-thumb" onClick={() => imageDialog.current?.showModal()} aria-label="Preview attached image"><img src={safeUrl} alt="Post attachment" loading="lazy" /></button>
    <dialog ref={imageDialog} className="attachment-image-dialog" aria-label="Attached image preview" onClick={(event) => { if (event.target === imageDialog.current) imageDialog.current?.close(); }}>
      <button type="button" className="ghost-button" onClick={() => imageDialog.current?.close()}>Close</button>
      <img src={safeUrl} alt="Post attachment, full size" />
      <a href={safeUrl} target="_blank" rel="noreferrer">Open image ↗</a>
    </dialog>
  </>;
  const supported = kind === "pdf" ? hasCapability(plugins, "attachment.preview.pdf") : kind === "glb" && hasCapability(plugins, "attachment.preview.glb");
  const label = kind === "pdf" ? "PDF document" : kind === "glb" ? "3D model" : "Attachment";
  if (compact || !supported) return <a className="attachment-file" href={safeUrl} target="_blank" rel="noreferrer">{label} ↗</a>;
  return <div className="attachment-plugin-preview">
    <button type="button" className="ghost-button" onClick={() => setOpen((current) => !current)} aria-expanded={open}>{open ? "Hide" : "Preview"} {label.toLowerCase()}</button>
    <a href={safeUrl} target="_blank" rel="noreferrer">Open file ↗</a>
    {open && kind === "pdf" ? <iframe className="howllo-pdf-preview" src={safeUrl} title="PDF attachment preview" loading="lazy" /> : null}
    {open && kind === "glb" ? modelReady
      ? createElement("model-viewer", { className: "howllo-model-preview", src: safeUrl, alt: "Interactive 3D attachment", "camera-controls": "", loading: "lazy" })
      : <span className="muted">Loading preview…</span> : null}
  </div>;
}
