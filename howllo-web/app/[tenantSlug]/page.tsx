import Link from "next/link";
import { ApiUnavailable } from "@/components/api-unavailable";
import { ApiError, getBoards, getTenantBranding } from "@/lib/api";
import { themedSurfaceStyle } from "@/lib/theme";
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
          <span className="kicker">Feedback workspace</span>
          <h1 className="page-title">{branding.site_name} boards</h1>
          <p className="page-lead">Choose a board to read updates, share an idea, or report a problem.</p>
        </section>
        {boards.length ? <section className="board-directory" aria-label="Public boards">
          {boards.map((board) => (
            <Link
              className="board-directory__card"
              key={board.id}
              href={`/${encodeURIComponent(tenantSlug)}/boards/${encodeURIComponent(board.slug)}`}
              style={themedSurfaceStyle(board.background_color)}
            >
              <span className="board-directory__identity">{board.icon_url ? <img src={board.icon_url} alt="" /> : <span className="board-directory__initial">{board.name.trim().charAt(0).toUpperCase()}</span>}<span className="kicker">{board.board_type}</span></span>
              <h2>{board.name}</h2>
              <p>{board.description || "Read and share feedback on this board."}</p>
              <span className="board-directory__open">Open board →</span>
            </Link>
          ))}
        </section> : <section className="panel empty-state">
          <h2 className="empty-state__title">No public boards yet</h2>
          <p className="empty-state__copy">The workspace owner can add a public board from Manage → Boards.</p>
        </section>}
      </div>
  );
}
