import Link from "next/link";
import { getRoadmap } from "@/lib/api";
import { buildTenantPath, resolveTenantContext } from "@/lib/default-tenant";
import { getServerBearerToken } from "@/lib/server-auth";
import { ApiUnavailable } from "@/components/api-unavailable";
import { StatusPill } from "@/components/status-pill";

type RoadmapPageProps = {
  searchParams: Promise<{
    tenant?: string;
    status?: string;
    tag?: string;
  }>;
};

export default async function RoadmapPage({ searchParams }: RoadmapPageProps) {
  const query = await searchParams;
  const { tenantSlug: tenant, defaultTenantSlug } = await resolveTenantContext(query.tenant);
  const token = await getServerBearerToken();
  let items;

  try {
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
      <section className="page-head">
        <span className="kicker">Roadmap</span>
        <h1 className="page-title">Planned, in progress, and done</h1>
        <p className="page-lead">
          What we&rsquo;re working on next, and what&rsquo;s already shipped.
        </p>
      </section>

      {items.length > 0 ? (
        <section className="grid roadmap-grid">
          {items.map((item) => (
            <Link
              className="post-card"
              href={buildTenantPath(`/posts/${item.id}`, tenant, defaultTenantSlug)}
              key={item.id}
            >
              <div className="card-head">
                <h2 className="card-title">{item.title}</h2>
                <StatusPill status={item.status} />
              </div>
              <div className="metadata muted">
                <span>{item.vote_count} votes</span>
                <span>{item.comment_count} comments</span>
              </div>
            </Link>
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
