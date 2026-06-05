// Platform storage configuration — mirrors howllo-server src/storage/mod.rs.

export type StorageBackend = "local" | "minio";

export type PlatformStorageConfig = {
  backend: StorageBackend;
  backend_configured: boolean;
  local_path: string;
  public_base_url: string;
  minio_endpoint: string;
  minio_bucket: string;
  minio_access_key: string;
  minio_secret_key_configured: boolean;
  minio_use_ssl: boolean;
};

export type PlatformStorageConfigUpdate = {
  backend: StorageBackend;
  local_path: string;
  public_base_url: string;
  minio_endpoint: string;
  minio_bucket: string;
  minio_secret_key?: string;
  minio_access_key: string;
  minio_use_ssl: boolean;
};

export type TestStorageRequest = {
  backend: StorageBackend;
  local_path?: string;
  public_base_url?: string;
  minio_endpoint?: string;
  minio_bucket?: string;
  minio_access_key?: string;
  minio_secret_key?: string;
  minio_use_ssl?: boolean;
};

export type StorageTestResult = {
  ok: boolean;
  message: string;
};

export type PlatformDatabaseInfo = {
  url_masked: string;
  host: string;
  port: number;
  database: string;
  username: string;
  connection_ready: boolean;
  migration_count: number;
  message: string;
};

export type WorkspaceStorageUsageItem = {
  workspace_id: string;
  workspace_slug: string;
  workspace_name: string;
  asset_count: number;
  total_bytes: number;
  logo_bytes: number;
  attachment_bytes: number;
};

export type WorkspaceStorageUsageResponse = {
  backend: StorageBackend;
  total_assets: number;
  total_bytes: number;
  items: WorkspaceStorageUsageItem[];
};

export type PlatformStatusLevel = "ok" | "warning" | "fail";

export type PlatformStatusCheck = {
  key: string;
  label: string;
  level: PlatformStatusLevel;
  message: string;
  detail?: string | null;
};

export type PlatformStatusResponse = {
  overall: PlatformStatusLevel;
  checked_at: string;
  checks: PlatformStatusCheck[];
};

export type BuildEnvVar = {
  key: string;
  value: string;
};

export type BuildInfoResponse = {
  version: string;
  built_at: string;
  server_time: string;
  env: BuildEnvVar[];
};
