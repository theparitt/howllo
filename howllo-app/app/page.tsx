import { WorkspaceHome } from "@/components/workspace-home";
import { StaffInvitations } from "../components/staff-invitations";

export default function AppHomePage() {
  return <><StaffInvitations /><WorkspaceHome publicWebOrigin={process.env.NEXT_PUBLIC_HOWLLO_PUBLIC_WEB_URL} staffSignIn /></>;
}
