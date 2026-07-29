import { ManageArea } from "@/components/manage-area";

// Tenant/board-owner management (boards + team). Tenant + token resolve
// client-side inside ManageArea, gated to owner/admin.
export default function ManagePage() {
  return <ManageArea />;
}
