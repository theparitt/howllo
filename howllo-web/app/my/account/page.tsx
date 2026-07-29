import Link from "next/link";
import { ApiUnavailable } from "@/components/api-unavailable";
import { MyAccountPanel } from "@/components/my-account-panel";
import { WorkspaceState } from "@/components/workspace-state";
import { getMe } from "@/lib/api";
import {
  WorkspaceContextError,
  buildTenantPath,
  resolveTenantContext,
} from "@/lib/default-tenant";
import { getServerBearerToken } from "@/lib/server-auth";

type MyAccountPageProps = {
  searchParams: Promise<{
    tenant?: string;
  }>;
};

export default async function MyAccountPage({ searchParams }: MyAccountPageProps) {
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
          <span className="kicker">My account</span>
          <h1 className="empty-state__title">Sign in to this workspace.</h1>
          <p className="empty-state__copy">
            Howllo only opens account tools after this workspace has an active session.
          </p>
        </section>
      </div>
    );
  }

  try {
    const me = await getMe(token);

    return (
      <div className="page-stack">
        <section className="page-head">
          <span className="kicker">My account</span>
          <h1 className="page-title">Profile</h1>
          <p className="page-lead">
            RooIAM handles sign-in. Your public Howllo profile for this workspace is stored in Howllo.
          </p>
        </section>

        <MyAccountPanel howlloUser={me} tenantSlug={tenant} />

        <section className="panel">
          <h2 className="section-title">Workspace activity</h2>
          <p className="section-subtitle" style={{ marginTop: "0.45rem" }}>
            Howllo activity remains scoped to this workspace session.
          </p>
          <div className="hero__actions" style={{ marginTop: "1rem" }}>
            <Link className="button" href={buildTenantPath("/my/activity", tenant, defaultTenantSlug)}>
              View my activity
            </Link>
          </div>
        </section>
      </div>
    );
  } catch (error) {
    return (
      <div className="page-stack">
        <ApiUnavailable
          message={error instanceof Error ? error.message : "Failed to load account."}
        />
      </div>
    );
  }
}
