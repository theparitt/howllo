// A tenant is the owning company/workspace. `slug` is the unique public
// identifier (company-name + random suffix); `id` is the internal UUID.
export type Tenant = {
  id: string;
  slug: string;
  name: string;
  created_at: string;
  updated_at: string;
};

export type TenantBranding = {
  tenant_slug: string;
  tenant_name: string;
  site_name: string;
  logo_url: string | null;
  accent_color: string | null;
  background_color: string | null;
  show_powered_by: boolean;
  show_roadmap: boolean;
};

export type AdminTenantSummary = {
  id: string;
  slug: string;
  name: string;
  board_count: number;
  member_count: number;
  created_at: string;
  updated_at: string;
};
