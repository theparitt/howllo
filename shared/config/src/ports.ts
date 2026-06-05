// Canonical Howllo local-dev port map. This is the single source of truth.
// Keep app dev scripts and docker-compose in sync with these values.
export const PORTS = {
  /** howllo-server — Rust/Actix HTTP API */
  server: 5110,
  /** howllo-admin — Vite + React admin/moderation console */
  admin: 5111,
  /** howllo-web — Next.js public feedback boards */
  web: 5112,
  /** board realtime websocket endpoint */
  websocket: 5113,
  /** howllo-landing — marketing site */
  landing: 5114,
  /** reserved: howllo-widget (embeddable feedback widget) — later */
  widget: 5115,
  /** howllo-docs — documentation site */
  docs: 5116,
} as const;

export type PortName = keyof typeof PORTS;
