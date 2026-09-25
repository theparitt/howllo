import { API_BASE_URL } from "@howllo/config";

export type ClientOptions = {
  baseUrl?: string;
  /**
   * Full Authorization header value, e.g. `"Bearer <jwt>"`. Sent verbatim; the
   * server validates it. The server owns all authorization.
   */
  authorization?: string;
  /**
   * Tenant slug. Tenant-scoped routes take it as a `tenant_slug` query param
   * (the server resolves+authorizes the tenant per request).
   */
  tenant?: string;
  /** Optional hook invoked when the server rejects the current auth state. */
  onUnauthorized?: () => void;
};

export class HowlloApiError extends Error {
  constructor(
    public status: number,
    public code: string,
    message: string,
  ) {
    super(message);
    this.name = "HowlloApiError";
  }
}

export class HowlloClient {
  readonly baseUrl: string;
  private authorization?: string;
  readonly tenant?: string;
  private readonly onUnauthorized?: () => void;

  constructor(opts: ClientOptions = {}) {
    this.baseUrl = opts.baseUrl ?? API_BASE_URL;
    this.authorization = opts.authorization;
    this.tenant = opts.tenant;
    this.onUnauthorized = opts.onUnauthorized;
  }

  /** True once an Authorization value has been provided. */
  get isAuthenticated(): boolean {
    return Boolean(this.authorization);
  }

  /**
   * Append `tenant_slug` to a path when a tenant is configured. Pass
   * `withTenant: false` for routes that don't take it (e.g. /notifications).
   */
  private resolvePath(path: string, withTenant: boolean): string {
    if (!withTenant || !this.tenant) return path;
    const sep = path.includes("?") ? "&" : "?";
    return `${path}${sep}tenant_slug=${encodeURIComponent(this.tenant)}`;
  }

  private createHeaders(init: RequestInit): Headers {
    const headers = new Headers(init.headers);
    headers.set("accept", "application/json");
    if (init.body && !headers.has("content-type")) {
      headers.set("content-type", "application/json");
    }
    if (this.authorization) headers.set("authorization", this.authorization);
    return headers;
  }

  private async send(
    path: string,
    init: RequestInit & { withTenant?: boolean } = {},
  ): Promise<Response> {
    const { withTenant = true, ...rest } = init;
    const res = await fetch(
      `${this.baseUrl}${this.resolvePath(path, withTenant)}`,
      {
        ...rest,
        headers: this.createHeaders(rest),
        cache: "no-store",
      },
    );

    if (!res.ok) {
      if (res.status === 401) {
        this.onUnauthorized?.();
      }
      let code = "error";
      let message = res.statusText;
      try {
        const body = (await res.json()) as { code?: string; message?: string };
        code = body.code ?? code;
        message = body.message ?? message;
      } catch {
        // non-JSON error body; keep status text
      }
      throw new HowlloApiError(res.status, code, message);
    }

    return res;
  }

  async request<T>(
    path: string,
    init: RequestInit & { withTenant?: boolean } = {},
  ): Promise<T> {
    const res = await this.send(path, init);
    if (res.status === 204 || res.headers.get("content-length") === "0") {
      return undefined as T;
    }
    // Some endpoints return empty 200 bodies (finish()).
    const text = await res.text();
    return (text ? JSON.parse(text) : undefined) as T;
  }

  async requestText(
    path: string,
    init: RequestInit & { withTenant?: boolean } = {},
  ): Promise<string> {
    const res = await this.send(path, init);
    return res.text();
  }

  /**
   * Upload an image to `/api/uploads` (base64 JSON body) and return its public
   * URL. Used for workspace logos and post screenshots.
   */
  async uploadImage(file: File): Promise<string> {
    if (!this.tenant) throw new Error("Select a workspace before uploading an image.");
    const dataUrl: string = await new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(String(reader.result));
      reader.onerror = () => reject(new Error("Could not read the selected file"));
      reader.readAsDataURL(file);
    });
    const base64 = dataUrl.split(",")[1] ?? "";
    const { url } = await this.request<{ url: string }>("/api/uploads", {
      method: "POST",
      withTenant: false,
      body: JSON.stringify({
        tenant_slug: this.tenant,
        filename: file.name,
        content_type: file.type,
        data: base64,
      }),
    });
    return url;
  }
}
