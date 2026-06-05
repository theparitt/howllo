export type WebhookEndpoint = {
  id: string;
  url: string;
  has_secret: boolean;
  is_active: boolean;
  created_at: string;
};

export type WebhookDelivery = {
  id: string;
  webhook_event_id: string;
  webhook_endpoint_id: string;
  status_code: number | null;
  success: boolean;
  response_body: string | null;
  delivered_at: string;
  attempt_count: number;
  next_retry_at: string | null;
};
