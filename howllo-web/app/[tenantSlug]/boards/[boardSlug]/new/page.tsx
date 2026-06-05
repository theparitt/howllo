import CreatePostPage from "@/app/boards/[boardSlug]/new/page";

type TenantCreatePostPageProps = {
  params: Promise<{
    tenantSlug: string;
    boardSlug: string;
  }>;
};

export default async function TenantCreatePostPage({
  params,
}: TenantCreatePostPageProps) {
  const { tenantSlug, boardSlug } = await params;

  return CreatePostPage({
    params: Promise.resolve({ boardSlug }),
    searchParams: Promise.resolve({
      tenant: tenantSlug,
    }),
  });
}
