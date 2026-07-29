import Link from "next/link";
import { ApiUnavailable } from "@/components/api-unavailable";
import { StatusPill } from "@/components/status-pill";
import { WorkspaceState } from "@/components/workspace-state";
import { getMyActivity } from "@/lib/api";
import {
  WorkspaceContextError,
  buildTenantPath,
  resolveTenantContext,
} from "@/lib/default-tenant";
import { getServerBearerToken } from "@/lib/server-auth";
import type {
  MyCommentActivityItem,
  MyPostActivityItem,
  MyPostReferenceActivityItem,
  MyStatusActivityItem,
} from "@/lib/types";

type MyActivityPageProps = {
  searchParams: Promise<{
    tenant?: string;
  }>;
};

function formatDate(value: string) {
  return new Date(value).toLocaleString();
}

function ActivityPostRow({
  item,
  tenant,
  defaultTenantSlug,
}: {
  item: MyPostActivityItem;
  tenant: string;
  defaultTenantSlug: string;
}) {
  return (
    <Link className="list-row" href={buildTenantPath(`/posts/${item.id}`, tenant, defaultTenantSlug)}>
      <div>
        <strong>{item.title}</strong>
        <p className="section-subtitle">
          {item.board_name} • {item.comment_count} comments • {item.vote_count} votes • {formatDate(item.created_at)}
        </p>
      </div>
      <StatusPill status={item.status} />
    </Link>
  );
}

function ActivityCommentRow({
  item,
  tenant,
  defaultTenantSlug,
}: {
  item: MyCommentActivityItem;
  tenant: string;
  defaultTenantSlug: string;
}) {
  return (
    <Link className="list-row" href={buildTenantPath(`/posts/${item.post_id}`, tenant, defaultTenantSlug)}>
      <div>
        <strong>{item.post_title}</strong>
        <p className="section-subtitle">
          {item.board_name} • {item.comment_type} • {formatDate(item.created_at)}
        </p>
        <p className="activity-excerpt">{item.body}</p>
      </div>
    </Link>
  );
}

function ActivityReferenceRow({
  item,
  tenant,
  defaultTenantSlug,
}: {
  item: MyPostReferenceActivityItem;
  tenant: string;
  defaultTenantSlug: string;
}) {
  return (
    <Link className="list-row" href={buildTenantPath(`/posts/${item.post_id}`, tenant, defaultTenantSlug)}>
      <div>
        <strong>{item.post_title}</strong>
        <p className="section-subtitle">
          {item.board_name} • {formatDate(item.created_at)}
        </p>
      </div>
    </Link>
  );
}

function ActivityStatusRow({
  item,
  tenant,
  defaultTenantSlug,
}: {
  item: MyStatusActivityItem;
  tenant: string;
  defaultTenantSlug: string;
}) {
  return (
    <Link className="list-row" href={buildTenantPath(`/posts/${item.post_id}`, tenant, defaultTenantSlug)}>
      <div>
        <strong>{item.post_title}</strong>
        <p className="section-subtitle">
          {item.board_name} • {item.old_status ?? "none"} to {item.new_status} • {formatDate(item.created_at)}
        </p>
        {item.reason ? <p className="activity-excerpt">{item.reason}</p> : null}
      </div>
      <StatusPill status={item.new_status} />
    </Link>
  );
}

export default async function MyActivityPage({ searchParams }: MyActivityPageProps) {
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

  const token = await getServerBearerToken(tenant);

  if (!token) {
    return (
      <div className="page-stack">
        <section className="panel empty-state">
          <span className="kicker">My activity</span>
          <h1 className="empty-state__title">Sign in to this workspace.</h1>
          <p className="empty-state__copy">
            Activity is scoped to one Howllo workspace session at a time.
          </p>
        </section>
      </div>
    );
  }

  try {
    const activity = await getMyActivity(tenant, token);

    return (
      <div className="page-stack">
        <section className="page-head">
          <span className="kicker">My activity</span>
          <h1 className="page-title">Your work in this workspace</h1>
          <p className="page-lead">
            Posts, comments, votes, follows, and status changes connected to your account in {tenant}.
          </p>
        </section>

        <section className="activity-layout">
          <article className="panel">
            <h2 className="section-title">Posts created by me</h2>
            <div className="list-stack" style={{ marginTop: "1rem" }}>
              {activity.posts.length > 0 ? activity.posts.map((item) => (
                <ActivityPostRow key={item.id} item={item} tenant={tenant} defaultTenantSlug={defaultTenantSlug} />
              )) : <p className="section-subtitle">No posts yet.</p>}
            </div>
          </article>

          <article className="panel">
            <h2 className="section-title">Comments created by me</h2>
            <div className="list-stack" style={{ marginTop: "1rem" }}>
              {activity.comments.length > 0 ? activity.comments.map((item) => (
                <ActivityCommentRow key={item.id} item={item} tenant={tenant} defaultTenantSlug={defaultTenantSlug} />
              )) : <p className="section-subtitle">No comments yet.</p>}
            </div>
          </article>

          <article className="panel">
            <h2 className="section-title">Votes and follows</h2>
            <div className="activity-split" style={{ marginTop: "1rem" }}>
              <div className="list-stack">
                <h3 className="mini-title">Votes</h3>
                {activity.votes.length > 0 ? activity.votes.map((item) => (
                  <ActivityReferenceRow key={`vote-${item.post_id}`} item={item} tenant={tenant} defaultTenantSlug={defaultTenantSlug} />
                )) : <p className="section-subtitle">No votes yet.</p>}
              </div>
              <div className="list-stack">
                <h3 className="mini-title">Follows</h3>
                {activity.follows.length > 0 ? activity.follows.map((item) => (
                  <ActivityReferenceRow key={`follow-${item.post_id}`} item={item} tenant={tenant} defaultTenantSlug={defaultTenantSlug} />
                )) : <p className="section-subtitle">No follows yet.</p>}
              </div>
            </div>
          </article>

          <article className="panel">
            <h2 className="section-title">Status changes on my posts</h2>
            <div className="list-stack" style={{ marginTop: "1rem" }}>
              {activity.status_changes.length > 0 ? activity.status_changes.map((item) => (
                <ActivityStatusRow key={item.id} item={item} tenant={tenant} defaultTenantSlug={defaultTenantSlug} />
              )) : <p className="section-subtitle">No status changes yet.</p>}
            </div>
          </article>
        </section>
      </div>
    );
  } catch (error) {
    return (
      <div className="page-stack">
        <ApiUnavailable
          message={error instanceof Error ? error.message : "Failed to load activity."}
        />
      </div>
    );
  }
}
