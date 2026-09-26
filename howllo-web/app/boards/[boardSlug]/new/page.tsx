import Link from "next/link";
import {
  WorkspaceContextError,
  buildTenantPath,
  resolveTenantContext,
} from "@/lib/default-tenant";
import { CreatePostForm } from "@/components/create-post-form";
import { WorkspaceState } from "@/components/workspace-state";
import { ApiError, getBoardDetail } from "@/lib/api";
import { ApiUnavailable } from "@/components/api-unavailable";
import { getServerBearerToken } from "@/lib/server-auth";
import { boardKind } from "@/lib/board-experience";
import { notFound } from "next/navigation";

type CreatePostPageProps = {
  params: Promise<{
    boardSlug: string;
  }>;
  searchParams: Promise<{
    tenant?: string;
  }>;
};

export default async function CreatePostPage({
  params,
  searchParams,
}: CreatePostPageProps) {
  const { boardSlug } = await params;
  const query = await searchParams;
  let tenant: string;
  let defaultTenantSlug: string;

  try {
    ({ tenantSlug: tenant, defaultTenantSlug } = await resolveTenantContext(query.tenant));
  } catch (error) {
    if (error instanceof WorkspaceContextError) {
      return <WorkspaceState kind={error.kind} workspaceSlug={error.workspaceSlug} />;
    }
    throw error;
  }

  let board;
  try {
    board = await getBoardDetail(tenant, boardSlug, await getServerBearerToken(tenant));
  } catch (error) {
    if (error instanceof ApiError && error.status === 404) notFound();
    return <ApiUnavailable message={error instanceof Error ? error.message : "Could not open this board."} />;
  }
  if (boardKind(board.board_type) === "announcements") notFound();
  const kind = boardKind(board.board_type);
  const title = kind === "bug-reports" ? "Report a bug" : kind === "discussions" ? "Start a discussion" : "Suggest a feature";

  return (
    <div className="grid" style={{ gap: "1.25rem", maxWidth: "640px", margin: "0 auto", width: "100%" }}>
      <section className="page-head">
        <Link className="back-link" href={buildTenantPath(`/boards/${boardSlug}`, tenant, defaultTenantSlug)}>
          ← Back
        </Link>
        <h1 className="page-title">{title}</h1>
        <p className="page-lead">{kind === "bug-reports" ? "Tell the team what happened and how to reproduce it." : kind === "discussions" ? "Ask a question or start a conversation." : "Describe the improvement you would like to see."}</p>
      </section>

      <CreatePostForm boardSlug={boardSlug} tenantSlug={tenant} boardKind={kind} />
    </div>
  );
}
