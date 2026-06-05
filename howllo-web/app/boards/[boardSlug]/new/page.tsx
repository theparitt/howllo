import Link from "next/link";
import { buildTenantPath, resolveTenantContext } from "@/lib/default-tenant";
import { CreatePostForm } from "@/components/create-post-form";

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
  const { tenantSlug: tenant, defaultTenantSlug } = await resolveTenantContext(query.tenant);

  return (
    <div className="grid" style={{ gap: "1.25rem", maxWidth: "640px", margin: "0 auto", width: "100%" }}>
      <section className="page-head">
        <Link className="back-link" href={buildTenantPath(`/boards/${boardSlug}`, tenant, defaultTenantSlug)}>
          ← Back
        </Link>
        <h1 className="page-title">Submit feedback</h1>
        <p className="page-lead">Share a request, problem, or idea.</p>
      </section>

      <CreatePostForm boardSlug={boardSlug} tenantSlug={tenant} />
    </div>
  );
}
