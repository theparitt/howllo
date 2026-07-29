import { getRoadmap, getTenantBranding } from "@/lib/api";
import {
  WorkspaceContextError,
  buildTenantPath,
  resolveTenantContext,
} from "@/lib/default-tenant";
import { getServerBearerToken } from "@/lib/server-auth";
import { ApiUnavailable } from "@/components/api-unavailable";
import { RealtimeRefresh } from "@/components/realtime-refresh";
import { RoadmapItemCard } from "@/components/roadmap-item-card";
import { WorkspaceState } from "@/components/workspace-state";

type RoadmapPageProps = {
  searchParams: Promise<{
    tenant?: string;
    status?: string;
    tag?: string;
  }>;
};

export default async function RoadmapPage({ searchParams }: RoadmapPageProps) {
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
  let items;
  let branding;

  try {
    branding = await getTenantBranding(tenant);
    if (!branding.show_roadmap) {
      return (
        <div className="page-stack">
          <section className="panel empty-state">
            <h1 className="empty-state__title">Roadmap is hidden for this workspace</h1>
            <p className="empty-state__copy">
              This workspace has disabled its public roadmap.
            </p>
          </section>
        </div>
      );
    }

    items = await getRoadmap(tenant, {
      status: query.status,
      tag: query.tag,
      token,
    }).catch((error) => {
      throw new Error(
        error instanceof Error ? error.message : "Failed to load roadmap.",
      );
    });
  } catch (error) {
    return (
      <div className="page-stack">
        <ApiUnavailable
          message={error instanceof Error ? error.message : "Failed to load roadmap."}
        />
      </div>
    );
  }

  return (
    <div className="page-stack">
      <RealtimeRefresh tenantSlug={tenant} />
      <section className="page-head">
        <span className="kicker">Roadmap</span>
        <h1 className="page-title">Planned, in progress, and done</h1>
        <p className="page-lead">
          What we&rsquo;re working on next, and what&rsquo;s already shipped.
        </p>
      </section>

      {items.length > 0 ? (
        <section className="roadmap-list">
          {items.map((item) => (
            <RoadmapItemCard
              key={item.id}
              title={item.title}
              status={item.status}
              voteCount={item.vote_count}
              commentCount={item.comment_count}
              href={buildTenantPath(`/posts/${item.id}`, tenant, defaultTenantSlug)}
            />
          ))}
        </section>
      ) : (
        <section className="panel empty-state">
          <h2 className="empty-state__title">Nothing on the roadmap yet</h2>
          <p className="empty-state__copy">Planned and in-progress work will show up here.</p>
        </section>
      )}
    </div>
  );
}
