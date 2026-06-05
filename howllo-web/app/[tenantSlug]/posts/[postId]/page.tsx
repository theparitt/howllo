import PostPage from "@/app/posts/[postId]/page";

type TenantPostPageProps = {
  params: Promise<{
    tenantSlug: string;
    postId: string;
  }>;
};

export default async function TenantPostPage({
  params,
}: TenantPostPageProps) {
  const { tenantSlug, postId } = await params;

  return PostPage({
    params: Promise.resolve({ postId }),
    searchParams: Promise.resolve({
      tenant: tenantSlug,
    }),
  });
}
