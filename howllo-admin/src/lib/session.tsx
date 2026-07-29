import {
  useCallback,
  createContext,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { HowlloClient } from "@howllo/api-client";
import { API_BASE_URL, DEFAULT_TENANT } from "@howllo/config";

// Local single-admin session.
//
// Auth is a password login against the Howllo backend (no external IdP). The
// backend mints a short-lived JWT that authorizes every /api/admin/* call; this
// module just stores that token and attaches it to the API client. The server
// still owns all authorization.

const TENANT_KEY = "howllo.admin.tenant";
const ACCESS_TOKEN_KEY = "howllo.admin.access_token";
const USER_NAME_KEY = "howllo.admin.user_name";
const USER_EMAIL_KEY = "howllo.admin.user_email";
const LEGACY_TENANTS = new Set(["rooiam", "rooiam-9i998d"]);

type SessionUser = {
  name: string;
  email: string;
};

type AuthState = {
  bootstrapped: boolean;
  setupAvailable: boolean;
};

type Session = {
  authorization: string;
  tenant: string;
  client: HowlloClient;
  user: SessionUser | null;
  isAuthenticated: boolean;
  /** Whether an admin password has been set yet. Null until the probe resolves. */
  bootstrapped: boolean | null;
  /** Whether the server allows first-run setup (ADMIN_BOOTSTRAP_KEY present). */
  setupAvailable: boolean;
  /** Set when the auth-state probe failed (e.g. server unreachable). */
  probeError: string | null;
  /** True until the initial auth-state probe finishes. */
  loading: boolean;
  setTenant: (value: string) => void;
  setup: (input: { bootstrapKey: string; password: string }) => Promise<void>;
  login: (password: string) => Promise<void>;
  logout: () => void;
};

const SessionContext = createContext<Session | null>(null);

function readStored(key: string, fallback = ""): string {
  if (typeof window === "undefined") return fallback;
  return window.localStorage.getItem(key) ?? fallback;
}

function normalizeTenantSlug(value: string): string {
  const trimmed = value.trim();
  if (LEGACY_TENANTS.has(trimmed)) {
    return DEFAULT_TENANT;
  }
  return trimmed;
}

type TokenResponse = {
  access_token: string;
  user: { name: string; email: string };
};

async function postJson<T>(path: string, body: unknown): Promise<T> {
  const res = await fetch(`${API_BASE_URL}${path}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!res.ok) {
    let message = `Request failed (${res.status})`;
    try {
      const data = (await res.json()) as { error?: { message?: string } };
      if (data?.error?.message) message = data.error.message;
    } catch {
      // keep default
    }
    throw new Error(message);
  }
  return (await res.json()) as T;
}

export function SessionProvider({ children }: { children: ReactNode }) {
  const [tenant, setTenantState] = useState(() =>
    normalizeTenantSlug(readStored(TENANT_KEY, DEFAULT_TENANT)),
  );
  const [accessToken, setAccessToken] = useState(() =>
    readStored(ACCESS_TOKEN_KEY),
  );
  const [user, setUser] = useState<SessionUser | null>(() => {
    const name = readStored(USER_NAME_KEY);
    const email = readStored(USER_EMAIL_KEY);
    return name || email ? { name, email } : null;
  });
  // Null until the probe resolves. We never assume "bootstrapped"; if the probe
  // fails we surface the error instead of silently showing a login form the
  // operator can't satisfy (there may be no password yet).
  const [authState, setAuthState] = useState<AuthState | null>(null);
  const [probeError, setProbeError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const clearAuth = useCallback(() => {
    window.localStorage.removeItem(ACCESS_TOKEN_KEY);
    window.localStorage.removeItem(USER_NAME_KEY);
    window.localStorage.removeItem(USER_EMAIL_KEY);
    setAccessToken("");
    setUser(null);
  }, []);

  useEffect(() => {
    let cancelled = false;
    fetch(`${API_BASE_URL}/api/admin/auth/state`)
      .then(async (res) => {
        if (!res.ok) throw new Error(`Server returned ${res.status}`);
        // Backend uses snake_case on the wire; map to our camelCase shape.
        return (await res.json()) as {
          bootstrapped: boolean;
          setup_available: boolean;
        };
      })
      .then((data) => {
        if (!cancelled) {
          if (!data.bootstrapped) {
            clearAuth();
          }
          setAuthState({
            bootstrapped: data.bootstrapped,
            setupAvailable: data.setup_available,
          });
          setProbeError(null);
        }
      })
      .catch((err: unknown) => {
        if (!cancelled) {
          setProbeError(
            err instanceof Error
              ? `Cannot reach the Howllo server at ${API_BASE_URL}. ${err.message}`
              : "Cannot reach the Howllo server.",
          );
        }
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const authorization = accessToken ? `Bearer ${accessToken}` : "";
  const isAuthenticated =
    authState?.bootstrapped === false ? false : Boolean(accessToken);

  const client = useMemo(
    () =>
      new HowlloClient({
        authorization: authorization || undefined,
        tenant: tenant || undefined,
        onUnauthorized: clearAuth,
      }),
    [authorization, clearAuth, tenant],
  );

  const setTenant = (value: string) => {
    const trimmed = normalizeTenantSlug(value);
    window.localStorage.setItem(TENANT_KEY, trimmed);
    setTenantState(trimmed);
  };

  const applyToken = (data: TokenResponse) => {
    window.localStorage.setItem(ACCESS_TOKEN_KEY, data.access_token);
    window.localStorage.setItem(USER_NAME_KEY, data.user.name);
    window.localStorage.setItem(USER_EMAIL_KEY, data.user.email);
    setAccessToken(data.access_token);
    setUser({ name: data.user.name, email: data.user.email });
    setAuthState((prev) => ({
      setupAvailable: prev?.setupAvailable ?? false,
      bootstrapped: true,
    }));
  };

  const setup = async (input: { bootstrapKey: string; password: string }) => {
    const data = await postJson<TokenResponse>("/api/admin/auth/setup", {
      bootstrap_key: input.bootstrapKey,
      password: input.password,
    });
    applyToken(data);
  };

  const login = async (password: string) => {
    const data = await postJson<TokenResponse>("/api/admin/auth/login", {
      password,
    });
    applyToken(data);
  };

  const logout = () => {
    clearAuth();
  };

  const value: Session = {
    authorization,
    tenant,
    client,
    user,
    isAuthenticated,
    bootstrapped: authState ? authState.bootstrapped : null,
    setupAvailable: authState?.setupAvailable ?? false,
    probeError,
    loading,
    setTenant,
    setup,
    login,
    logout,
  };

  return (
    <SessionContext.Provider value={value}>{children}</SessionContext.Provider>
  );
}

export function useSession(): Session {
  const ctx = useContext(SessionContext);
  if (!ctx) throw new Error("useSession must be used within SessionProvider");
  return ctx;
}
