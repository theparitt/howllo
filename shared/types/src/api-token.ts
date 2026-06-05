export type ApiTokenListItem = {
  id: string;
  name: string;
  token_prefix: string;
  scopes: string[];
  revoked_at: string | null;
  created_at: string;
};

export type ApiTokenCreated = {
  id: string;
  name: string;
  token: string;
  token_prefix: string;
  scopes: string[];
};
