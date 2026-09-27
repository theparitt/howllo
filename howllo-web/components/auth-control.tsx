"use client";

import { AuthLogin } from "@/components/auth-login";

type AuthControlProps = {
  myHref?: string;
  staffMode?: boolean;
  hideSignedOutTrigger?: boolean;
};

export function AuthControl({ myHref, staffMode, hideSignedOutTrigger }: AuthControlProps) {
  return <AuthLogin myHref={myHref} staffMode={staffMode} hideSignedOutTrigger={hideSignedOutTrigger} />;
}
