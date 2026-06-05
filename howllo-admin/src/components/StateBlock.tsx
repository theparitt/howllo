import type { ReactNode } from "react";

// Renders loading / error / empty states, otherwise the children. Keeps every
// data view consistent without repeating the same three guards.
export function StateBlock({
  loading,
  error,
  empty,
  children,
}: {
  loading: boolean;
  error: string | null;
  empty?: string | null;
  children: ReactNode;
}) {
  if (loading) return <p className="muted">Loading...</p>;
  if (error) return <p className="error-text">{error}</p>;
  if (empty) return <p className="muted">{empty}</p>;
  return <>{children}</>;
}
