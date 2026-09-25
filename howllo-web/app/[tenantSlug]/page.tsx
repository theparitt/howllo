import Link from "next/link";
import { ApiUnavailable } from "@/components/api-unavailable";
import { ApiError, getBoards, getTenantBranding } from "@/lib/api";
import { notFound } from "next/navigation";

type TenantRootPageProps = {
  params: Promise<{
    tenantSlug: string;
  }>;
};

export default async function TenantRootPage({ params }: TenantRootPageProps) {
  const { tenantSlug } = await params;
  let branding;
  let boards;
  try {
    [branding, boards] = await Promise.all([
      getTenantBranding(tenantSlug),
      getBoards(tenantSlug),
    ]);
  } catch (error) {
    if (error instanceof ApiError && error.status === 404) notFound();
    return <ApiUnavailable message={error instanceof Error ? error.message : "Could not load this workspace."} />;
  }

  if (!branding.show_boards) notFound();

  return (
      <div className="page-stack">
        <section className="page-head">
          <h1 className="page-title">Boards</h1>
          <p className="page-lead">Choose where to post or read feedback.</p>
        </section>
        {boards.length ? <section className="list-stack" aria-label="Public boards">
          {boards.map((board) => (
            <Link
              className="list-row"
              key={board.id}
              href={`/${encodeURIComponent(tenantSlug)}/boards/${encodeURIComponent(board.slug)}`}
              style={board.background_color ? { backgroundColor: board.background_color } : undefined}
            >
              <span className="board-list__item">
                {board.icon_url ? <img className="board-list__icon" src={board.icon_url} alt="" /> : null}
                <span><strong>{board.name}</strong>{board.description ? <span className="section-subtitle">{board.description}</span> : null}</span>
              </span>
              <span aria-hidden="true">→</span>
            </Link>
          ))}
        </section> : <section className="panel empty-state">
          <h2 className="empty-state__title">No public boards yet</h2>
          <p className="empty-state__copy">Check back soon.</p>
        </section>}
      </div>
  );
}
