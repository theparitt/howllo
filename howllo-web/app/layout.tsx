import type { Metadata } from "next";
import "./globals.css";
import { TenantShell } from "@/components/tenant-shell";

export const metadata: Metadata = {
  title: "Howllo",
  description: "Feedback, roadmap, and product communication.",
  icons: {
    icon: "/brand/howllo-logo.svg",
    shortcut: "/brand/howllo-logo.svg",
    apple: "/brand/howllo-logo.svg",
  },
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body>
        <TenantShell>{children}</TenantShell>
      </body>
    </html>
  );
}
