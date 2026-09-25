import { redirect } from "next/navigation";

// Legacy unscoped management link. Pick a workspace before opening Howllo App.
export default function ManagePage() {
  redirect("/");
}
