<p align="center">
  <img src="../art/howllo-logo-wordmark-horizontal.svg" alt="Howllo" width="420" />
</p>

# Howllo Documentation

Welcome to the official documentation for Howllo.

Local docs/help surface:

- path: `howllo-docs`
- reserved local port: `5116`
- purpose: board help, product documentation, setup, and self-hosting notes

<p align="center">
  <img src="../art/howllo-logo.svg" alt="Howllo logo" width="108" />
</p>

## Getting Started

1. Set up the database
2. Run the server
3. Connect the frontend

## Brand Assets

The user-facing surfaces now use the shared artwork from [`../art`](../art):

- `howllo-logo.svg`
- `howllo-wordmark.svg`
- `howllo-logo-wordmark-horizontal.svg`

Current usage:

- `howllo-web`: app header, favicon, and general product identity
- `howllo-landing`: navigation, hero, and footer branding
- `howllo-docs`: user docs header branding

## Local Surface Map

- `5110` -> `howllo-server`
- `5111` -> `howllo-admin`
- `5112` -> `howllo-web`
- `5113` -> board websocket / realtime
- `5114` -> `howllo-landing`
- `5116` -> `howllo-docs`

## Integration with Rooiam

Howllo uses Rooiam as the primary identity provider.
To integrate, set the following environment variables:

- `ROOIAM_JWT_SECRET`: The shared secret for token validation.
- `DATABASE_URL`: Your PostgreSQL connection string.

## Concepts

- **Boards**: The top-level container for feedback (e.g., Feature Requests).
- **Signals**: Individual pieces of feedback or requests.
- **Roadmap**: The visualization of signal statuses across transitions.
