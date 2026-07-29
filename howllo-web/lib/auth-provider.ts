export type AuthProviderId =
  | "rooiam"
  | "local"
  | "oidc"
  | "oauth2"
  | "disabled";

const KNOWN_PROVIDERS = new Set<AuthProviderId>([
  "rooiam",
  "local",
  "oidc",
  "oauth2",
  "disabled",
]);

function parseProvider(value: string | undefined): AuthProviderId {
  const normalized = value?.trim().toLowerCase();
  return normalized && KNOWN_PROVIDERS.has(normalized as AuthProviderId)
    ? (normalized as AuthProviderId)
    : "rooiam";
}

function parseProviderList(value: string | undefined, fallback: AuthProviderId) {
  const items = (value ?? "")
    .split(",")
    .map((item) => parseProvider(item))
    .filter((item, index, list) => list.indexOf(item) === index);
  return items.length > 0 ? items : [fallback];
}

export const ACTIVE_AUTH_PROVIDER = parseProvider(
  process.env.NEXT_PUBLIC_HOWLLO_AUTH_PROVIDER,
);

export const ENABLED_AUTH_PROVIDERS = parseProviderList(
  process.env.NEXT_PUBLIC_HOWLLO_AUTH_PROVIDERS,
  ACTIVE_AUTH_PROVIDER,
);

export function authProviderLabel(provider: AuthProviderId) {
  switch (provider) {
    case "rooiam":
      return "RooIAM";
    case "local":
      return "Local account";
    case "oidc":
      return "OpenID Connect";
    case "oauth2":
      return "OAuth2";
    case "disabled":
      return "Disabled";
  }
}

export function providerSupportsSelfService(provider: AuthProviderId) {
  return provider === "rooiam";
}
