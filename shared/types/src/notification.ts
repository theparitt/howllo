export type Notification = {
  id: string;
  event_type: string;
  title: string;
  body: string;
  is_read: boolean;
  post_id: string | null;
  created_at: string;
};
