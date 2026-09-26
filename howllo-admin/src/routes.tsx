import { createBrowserRouter, Navigate } from "react-router-dom";
import { AdminLayout } from "./app/AdminLayout";
import { AuditPage } from "./features/audit/AuditPage";
import { DashboardPage } from "./features/dashboard/DashboardPage";
import { BoardsPage } from "./features/boards/BoardsPage";
import { IntegrationsPage } from "./features/integrations/IntegrationsPage";
import { ModerationPage } from "./features/moderation/ModerationPage";
import { RoadmapPage } from "./features/roadmap/RoadmapPage";
import { TagsPage } from "./features/tags/TagsPage";
import { MembersPage } from "./features/members/MembersPage";
import { SettingsPage } from "./features/settings/SettingsPage";
import { TenantsPage } from "./features/tenants/TenantsPage";
import { WorkspaceDetailPage } from "./features/tenants/WorkspaceDetailPage";
import { PlatformSettingsPage } from "./features/platform/PlatformSettingsPage";
import { LocalUsersPage } from "./features/platform/LocalUsersPage";

// Route table mirrors @howllo/config adminRoutes.
export const router = createBrowserRouter([
  {
    path: "/",
    element: <Navigate to="/admin" replace />,
  },
  {
    path: "/admin",
    element: <AdminLayout />,
    children: [
      { index: true, element: <DashboardPage /> },
      { path: "tenants", element: <TenantsPage /> },
      { path: "tenants/:workspaceSlug", element: <WorkspaceDetailPage /> },
      { path: "platform", element: <PlatformSettingsPage /> },
      { path: "users", element: <LocalUsersPage /> },
      { path: "boards", element: <BoardsPage /> },
      { path: "boards/:boardId", element: <BoardsPage /> },
      { path: "tags", element: <TagsPage /> },
      { path: "members", element: <MembersPage /> },
      { path: "audit", element: <AuditPage /> },
      { path: "integrations", element: <IntegrationsPage /> },
      { path: "roadmap", element: <RoadmapPage /> },
      { path: "moderation", element: <ModerationPage /> },
      { path: "settings", element: <SettingsPage /> },
    ],
  },
]);
