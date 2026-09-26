import type { ReactNode } from "react";

export const LIST_PAGE_SIZE = 20;

export function ListSearch({ value, onChange, placeholder, children }: { value: string; onChange: (value: string) => void; placeholder: string; children?: ReactNode }) {
  return <div className="list-controls"><input className="manage-input" type="search" value={value} onChange={(event) => onChange(event.target.value)} placeholder={placeholder} aria-label={placeholder} />{children}</div>;
}

export function ListPager({ page, total, onPage }: { page: number; total: number; onPage: (page: number) => void }) {
  const pages = Math.max(1, Math.ceil(total / LIST_PAGE_SIZE));
  if (total <= LIST_PAGE_SIZE) return null;
  const current = Math.min(page, pages);
  return <nav className="list-pager" aria-label="List pages"><span>{(current - 1) * LIST_PAGE_SIZE + 1}–{Math.min(current * LIST_PAGE_SIZE, total)} of {total}</span><button type="button" className="ghost-button" disabled={current <= 1} onClick={() => onPage(current - 1)}>Previous</button><span>{current} / {pages}</span><button type="button" className="ghost-button" disabled={current >= pages} onClick={() => onPage(current + 1)}>Next</button></nav>;
}
