// Tenant-scoped roles. Authorization is enforced in howllo-server; never trust
// a role claim from the frontend.
export type Role = "owner" | "admin" | "moderator" | "member";
