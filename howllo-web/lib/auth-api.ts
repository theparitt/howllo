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
    throw new Error(response.status === 401 ? "Credentials or recovery code are incorrect." :
      response.status === 429 ? "Too many attempts. Please try again shortly." :
      response.status === 422 ? "Check the username and password requirements." :
      "Could not complete this request.");
  }
  return response.json() as Promise<LocalAuthResult>;
}

export async function getLoginProviders(): Promise<LoginProvider[]> {
  const response = await fetch(`${API_BASE_URL}/api/auth/providers`, { cache: "no-store" });
  if (!response.ok) throw new Error("Could not load sign-in options.");
  return response.json() as Promise<LoginProvider[]>;
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
