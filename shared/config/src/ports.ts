// Canonical Howllo local-dev port map. This is the single source of truth.
// Keep app dev scripts and docker-compose in sync with these values.
export const PORTS = {
  /** howllo-server — Rust/Actix HTTP API */
  server: 7700,
  /** howllo-admin — Vite + React admin/moderation console */
  admin: 7701,
  /** howllo-app — Next.js tenant management */
  app: 7702,
  /** howllo-web — Next.js public feedback boards */
  web: 7703,
  /** board realtime websocket shares the server port at /ws */
  websocket: 7700,
  /** howllo-landing — marketing site */
  landing: 5114,
  /** reserved: howllo-widget (embeddable feedback widget) — later */
  widget: 5115,
  /** howllo-docs — documentation site */
  docs: 5116,
} as const;

export type PortName = keyof typeof PORTS;
