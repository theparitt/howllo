# Email in Howllo

Email is optional. A new installation starts with delivery **disabled**. Local
passwords, one-time recovery codes, in-app notifications, and staff invitation
acceptance continue to work without SMTP.

## Turn on delivery

1. Generate a stable 32-byte key with `openssl rand -hex 32` and set
   `HOWLLO_EMAIL_CONFIG_KEY` in the server environment. Keep it secret and back
   it up; it encrypts SMTP credentials and queued message bodies. Restart the
   server after setting it.
2. In **Admin → Platform settings → Email**, enter an SMTP host, port, security
   mode, credentials if required, and a sender address that your mail provider
   permits. Save.
3. Send a test email to an inbox you control. After it arrives, click **Enable
   email**. Changing a connection or sender setting requires a new test.
4. In **App → Workspace → Email**, enable email for that workspace and select
   notification and announcement features. Its daily and monthly limits cannot
   exceed the platform limits.
5. Customers verify an email address in **Web → My account → Email** and choose
   which messages to receive. Broadcasts require an explicit customer opt-in.

Plain SMTP is accepted only for `localhost`, `127.0.0.1`, or `::1` for local
testing. Use STARTTLS or implicit TLS with real SMTP providers. Howllo does
not verify SPF, DKIM, or DMARC; configure them with your mail provider before
using a production sender domain.

Set `HOWLLO_STAFF_APP_URL` to the public staff app URL in the server environment
so invitation emails point to the right app. Set `HOWLLO_CUSTOMER_WEB_ORIGIN`
to the public customer site for password reset and announcement links.

## What gets sent

| Message | Requires workspace opt-in | Requires verified customer email |
| --- | --- | --- |
| Staff invitation | No | No; the inviter shares a single-use code privately for local staff, or the recipient accepts with a bound RooIAM identity |
| Customer email verification and password reset | No | Verification creates the verified address; reset requires one |
| Reply and important update | Yes | Yes; each customer can turn either off |
| Daily digest | Yes | Yes; customer must opt in |
| Announcement | Yes | Yes; customer must explicitly opt in |

Reply/update emails wait five minutes and group repeated events for the same
thread and recipient into one email per 15-minute window. Digests are scheduled
once a day after 08:00 UTC. Every queued non-system message is checked again
against current workspace membership, verified address, preferences, and
suppression before sending. Disable actions cancel queued messages. SMTP
delivery is retried with backoff and pauses platform email after five
consecutive failures. A successful test clears the pause.

Defaults: 500 workspace emails/day, 5,000/month, 100,000 platform emails/month,
20 messages/recipient/day, 5 reset or verification emails/recipient/day, 20
staff invitations/actor/day, 1,000 announcement recipients, two announcements
per workspace/day and five/week. Password reset and other system messages do
not consume workspace or platform notification quota, but still have recipient
rate limits. SMTP test emails are limited to five per recipient and 20 total
per hour. Platform and tenant admins can lower their applicable limits.

The platform sender address is fixed for all workspaces. Tenant staff cannot
read SMTP credentials or set a `From` address. Queue bodies and SMTP passwords
are encrypted at rest. Recipient addresses, subjects, message metadata, and
delivery status remain readable in the database for operational support;
protect database access accordingly. Mail transport is at-least-once: a crash
after an SMTP server accepts a message but before the database marks it sent
can cause a duplicate after the lease expires.

If a provider reports a bounce or complaint, the platform operator can add the
address under **Admin → Platform settings → Email → Blocked recipients**. This
cancels queued messages and prevents new ones. Remove the block only after the
address owner or mail provider confirms the issue is resolved. Generic SMTP
does not provide automatic bounce webhooks in this version.

## Local MailHog check

With MailHog listening on SMTP `127.0.0.1:1025` and HTTP `127.0.0.1:8025`,
save `127.0.0.1`, port `1025`, security `None`, no SMTP username or password,
and a test sender such as `mail@howllo.test` in Admin. Send a test email and
inspect <http://127.0.0.1:8025>. Never use MailHog as production mail.

The integration tests use a disposable PostgreSQL database, never the live
Howllo database. After migrating that database, run:

```sh
DATABASE_URL=postgres://postgres:postgres@127.0.0.1:55432/howllo_email_test \
HOWLLO_TEST_MAILHOG=1 cargo test --manifest-path howllo-server/Cargo.toml email::tests
```

The tests cover role authorization, disabled mode, SMTP test/enable, encrypted
secrets, actual MailHog delivery, quotas, duplicate aggregation, customer
opt-out, verification, password reset privacy/replay, session revocation,
announcements, digests, and retry/pause behavior.

## Test perspectives and edge cases

| Perspective | Scenario | Expected result |
| --- | --- | --- |
| Platform operator | Save SMTP, test through MailHog, enable | Test arrives; delivery starts only after a successful test |
| Platform operator | Change sender or connection after enabling | Status returns to configured; enable requires a new test |
| Workspace staff | Platform email disabled | Email menu is hidden; staff invitations still work in-app |
| Workspace staff | Enable notifications, exceed platform ceiling | Server rejects the override; platform ceiling remains authoritative |
| Workspace staff | Send announcement without recipient opt-in | No recipient queued; no campaign slot consumed |
| Workspace staff | Send two announcements at the daily or weekly cap | Later campaign rejected; a PostgreSQL lock serializes concurrent reservations |
| Customer | Verify address, replay verification code | First succeeds; replay fails |
| Customer | Request reset for known and unknown address | Both get the same accepted response; only the known, verified local account queues email |
| Customer | Use reset token twice or after expiry | First valid use changes password and rotates recovery code; replay/expired token fails |
| Customer | Opt out or leave workspace after mail is queued | Worker cancels the pending non-system message |
| Any recipient | Repeated notifications to the same thread | One queued message groups events in a 15-minute window |
| Any recipient | Suppressed address or daily recipient cap | Delivery is skipped |
| Operator | SMTP repeatedly fails | Retry with backoff; platform pauses after five failures |

The automated MailHog test uses local plaintext SMTP only. It does not prove
deliverability through a real provider or production domain; confirm those
separately with the provider's test inbox and DNS records before enabling
production email.
