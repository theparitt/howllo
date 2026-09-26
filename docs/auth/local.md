# Local board accounts

Local board accounts are enabled by default. Set `HOWLLO_AUTH_LOCAL_ENABLED=true` explicitly in production. A visitor chooses **Local account**, creates a username (3–32 lowercase letters, digits or underscores) and password (12–128 characters), then signs in. Passwords are stored as Argon2id hashes. Login attempts are limited to 10 per minute per client IP and route.

Local accounts have no verified email. They cannot claim email invitations until an explicit verified-email feature is added. They can create a workspace or join a public board through normal Howllo membership rules. To disable local accounts, set `HOWLLO_AUTH_LOCAL_ENABLED=false`; existing sessions still expire or can be revoked.

## Password and recovery code

Registration returns a random recovery code **once**. Save it offline; Howllo stores only its SHA-256 digest. The code has 256 random bits and is independent of the password. The login page has **Forgot password? Use recovery code**: enter the username, saved code and a new password. A successful reset consumes the old code, returns a replacement code to save, and revokes all account and workspace sessions. The same code cannot reset twice. Reset failures do not reveal whether a username exists.

While signed in, **My account → Local account security** lets a local user change their password after entering the current password, or generate a replacement recovery code after entering that password. Changing the password revokes all sessions. Generating a new code invalidates the previous one. Existing local accounts created before this feature can generate a code there. These endpoints are limited to 10 attempts per minute per client IP and route.

There is no email reset because local accounts have no verified email address or mail delivery configuration. If both the password and recovery code are lost, this version cannot safely prove account ownership. A server operator must use a separate, audited recovery procedure; Howllo does not offer an unauthenticated admin reset endpoint.

The separate admin console is bootstrapped with `HOWLLO_ADMIN_BOOTSTRAP_KEY` and its own password (at least 12 characters on new setups). Admin setup and sign-in share a PostgreSQL-based limit of 30 attempts per minute across API instances. Set a strong `HOWLLO_ADMIN_JWT_SECRET` for that console. Never use the example secret on a network-facing deployment.

Development with no external IdP: start PostgreSQL and MinIO, copy `howllo-server/.env.example`, run migrations, start the API and web. No RooIAM settings are needed.
