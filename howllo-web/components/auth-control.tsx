"use client";

import { AuthLogin } from "@/components/auth-login";

type AuthControlProps = {
  myHref?: string;
};

export function AuthControl({ myHref }: AuthControlProps) {
  return <AuthLogin myHref={myHref} />;
}
