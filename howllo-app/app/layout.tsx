import type { Metadata } from "next";
import { AppShell } from "../components/app-shell";
import "../../howllo-web/app/globals.css";
import "./app.css";

export const metadata: Metadata = {
  title: "Howllo App",
  description: "Manage Howllo workspaces and boards.",
  icons: { icon: "/brand/howllo-logo.svg" },
};

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return <html lang="en"><body><AppShell>{children}</AppShell></body></html>;
}
