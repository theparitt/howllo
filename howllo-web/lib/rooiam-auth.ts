const RETURN_TO_KEY = "howllo.rooiam.returnTo";
const OIDC_KEY = "howllo.rooiam.oidc";

export function rememberRooiamReturnTo(path: string) {
  if (typeof window === "undefined") return;
  window.sessionStorage.setItem(RETURN_TO_KEY, path);
}

export function readRooiamReturnTo() {
  if (typeof window === "undefined") return "";
  return window.sessionStorage.getItem(RETURN_TO_KEY) ?? "";
}

export function consumeRooiamReturnTo() {
  if (typeof window === "undefined") return "";
  const value = window.sessionStorage.getItem(RETURN_TO_KEY) ?? "";
  window.sessionStorage.removeItem(RETURN_TO_KEY);
  return value;
}

export type RooiamOidcState = {
  state: string;
  codeVerifier: string;
  redirectUri: string;
};

export function writeRooiamOidcState(value: RooiamOidcState) {
  if (typeof window === "undefined") return;
  window.sessionStorage.setItem(OIDC_KEY, JSON.stringify(value));
}

export function readRooiamOidcState(): RooiamOidcState | null {
  if (typeof window === "undefined") return null;
  const raw = window.sessionStorage.getItem(OIDC_KEY);
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw) as RooiamOidcState;
    if (!parsed.state || !parsed.codeVerifier || !parsed.redirectUri) {
      return null;
    }
    return parsed;
  } catch {
    return null;
  }
}

export function clearRooiamOidcState() {
  if (typeof window === "undefined") return;
  window.sessionStorage.removeItem(OIDC_KEY);
}
