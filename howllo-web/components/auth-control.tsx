"use client";

import { ACTIVE_AUTH_PROVIDER, authProviderLabel } from "@/lib/auth-provider";
import { RooiamLoginWidget } from "@/components/rooiam-login-widget";

type AuthControlProps = {
  myHref?: string;
};

export function AuthControl({ myHref }: AuthControlProps) {
  switch (ACTIVE_AUTH_PROVIDER) {
    case "rooiam":
      return <RooiamLoginWidget myHref={myHref} />;
    case "disabled":
      return null;
    default:
      return (
        <button className="button" type="button" disabled title={`${authProviderLabel(ACTIVE_AUTH_PROVIDER)} is not configured in howllo-web yet.`}>
          {authProviderLabel(ACTIVE_AUTH_PROVIDER)}
        </button>
      );
  }
}
