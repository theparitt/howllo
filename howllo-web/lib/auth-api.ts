import { API_BASE_URL } from "@/lib/config";

export type LoginProvider = {
  id: string;
  display_name: string;
  kind: "local" | "oidc";
  login_url: string;
};

export type LocalAuthResult = { access_token: string; recovery_code?: string };

async function localAuthRequest(path: string, body: object): Promise<LocalAuthResult> {
  const response = await fetch(`${API_BASE_URL}/api/auth/local/${path}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!response.ok) {
    throw new Error(response.status === 401 ? (path === "login" ? "Username or password is incorrect." : "Recovery code is invalid or expired.") :
      response.status === 429 ? "Too many attempts. Please try again shortly." :
      response.status === 422 ? "Check the username and password requirements." :
      "Could not complete this request.");
  }
  return response.json() as Promise<LocalAuthResult>;
}

export async function getLoginProviders(tenantSlug?: string | null): Promise<LoginProvider[]> {
  const endpoint = tenantSlug ? `/api/auth/customer-providers?tenant_slug=${encodeURIComponent(tenantSlug)}` : "/api/auth/providers";
  const response = await fetch(`${API_BASE_URL}${endpoint}`, { cache: "no-store" });
  if (!response.ok) throw new Error("Could not load sign-in options.");
  return response.json() as Promise<LoginProvider[]>;
}

export type CustomerRooiamConfig = { provider: "rooiam"; rooiam_workspace_id: string; rooiam_client_id: string; rooiam_widget_base_url: string };
export async function getCustomerRooiamConfig(tenantSlug: string): Promise<CustomerRooiamConfig> {
  const response = await fetch(`${API_BASE_URL}/api/auth/customer-rooiam?tenant_slug=${encodeURIComponent(tenantSlug)}`, { cache: "no-store" });
  if (!response.ok) throw new Error("RooIAM is not enabled for this workspace.");
  return response.json() as Promise<CustomerRooiamConfig>;
}

export async function localSignIn(
  action: "login" | "register",
  username: string,
  password: string,
): Promise<LocalAuthResult> {
  return localAuthRequest(action, { username, password });
}

export async function resetLocalPassword(username: string, recoveryCode: string, newPassword: string): Promise<LocalAuthResult> {
  return localAuthRequest("reset-password", { username, recovery_code: recoveryCode, new_password: newPassword });
}

export async function emailAvailable(): Promise<boolean> {
  const response = await fetch(`${API_BASE_URL}/api/email/availability`, { cache: "no-store" });
  return response.ok && ((await response.json()) as { enabled: boolean }).enabled;
}

export async function requestEmailReset(email: string): Promise<void> {
  const response = await fetch(`${API_BASE_URL}/api/auth/local/request-email-reset`, {
    method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ email }),
  });
  if (!response.ok) throw new Error(response.status === 429 ? "Too many requests. Please try later." : "Could not request a reset code.");
}

export async function confirmEmailReset(token: string, newPassword: string): Promise<string> {
  const response = await fetch(`${API_BASE_URL}/api/auth/local/confirm-email-reset`, {
    method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ token, new_password: newPassword }),
  });
  if (!response.ok) throw new Error(response.status === 401 ? "Code is invalid, expired, or already used." : response.status === 429 ? "Too many attempts. Please try later." : "Could not reset password.");
  const result = await response.json() as { recovery_code: string };
  return result.recovery_code;
}

export async function getLocalAccountStatus(token: string): Promise<{ has_local_credentials: boolean; has_recovery_code: boolean }> {
  const response = await fetch(`${API_BASE_URL}/api/auth/local/status`, { headers: { Authorization: token }, cache: "no-store" });
  if (!response.ok) throw new Error("Could not load local account status.");
  return response.json() as Promise<{ has_local_credentials: boolean; has_recovery_code: boolean }>;
}

export async function changeLocalPassword(token: string, currentPassword: string, newPassword: string): Promise<void> {
  const response = await fetch(`${API_BASE_URL}/api/auth/local/change-password`, {
    method: "POST",
    headers: { "content-type": "application/json", Authorization: token },
    body: JSON.stringify({ current_password: currentPassword, new_password: newPassword }),
  });
  if (!response.ok) throw new Error(response.status === 401 ? "Current password is incorrect." : "Could not change password.");
}

export async function rotateLocalRecoveryCode(token: string, currentPassword: string): Promise<string> {
  const response = await fetch(`${API_BASE_URL}/api/auth/local/rotate-recovery-code`, {
    method: "POST",
    headers: { "content-type": "application/json", Authorization: token },
    body: JSON.stringify({ current_password: currentPassword }),
  });
  if (!response.ok) throw new Error(response.status === 401 ? "Current password is incorrect." : "Could not create recovery code.");
  const data = await response.json() as { recovery_code: string };
  return data.recovery_code;
}

export async function exchangeSignInCode(code: string): Promise<{ access_token: string; return_to: string }> {
  const response = await fetch(`${API_BASE_URL}/api/auth/exchange`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ code }),
  });
  if (!response.ok) throw new Error("The sign-in link expired. Please try again.");
  return response.json() as Promise<{ access_token: string; return_to: string }>;
}

export async function logoutAccount(token: string): Promise<void> {
  await fetch(`${API_BASE_URL}/api/auth/logout`, { method: "POST", headers: { Authorization: token } });
}

export async function providerAccountRequest<T>(token: string, path: string, method = "GET", body?: object): Promise<T> {
  const response = await fetch(`${API_BASE_URL}/api/me/identity/${path}`, {
    method,
    headers: { Authorization: token, ...(body ? { "content-type": "application/json" } : {}) },
    body: body ? JSON.stringify(body) : undefined,
    cache: "no-store",
  });
  if (!response.ok) {
    if (response.status === 404) throw new Error("Account security is not connected for this workspace. Sign in again if you recently changed providers.");
    if (response.status === 401) throw new Error("Your sign-in expired. Please sign in again.");
    if (response.status === 429) throw new Error("Too many requests. Please try again shortly.");
    throw new Error("Could not update your account security. Please try again.");
  }
  return response.json() as Promise<T>;
}
