# Web Push Operations

Web Push is optional and disabled by default. Foreground browser notifications keep
working without it. Enabling Web Push adds encrypted background delivery for persisted
notification-center events.

## Generate VAPID Keys

Generate one URL-safe, unpadded key pair and keep it stable for the deployment:

```sh
npx web-push generate-vapid-keys --json
```

The command is provided by the upstream
[`web-push`](https://github.com/web-push-libs/web-push) project. Put `publicKey` in
`public_key` and `privateKey` in `private_key`. Never commit the private key or print it
in application logs. Rotating the pair requires browsers to create new subscriptions.

## Configure

```toml
[web_push]
enabled = true
public_key = "<publicKey>"
private_key = "<privateKey>"
subject = "mailto:admin@example.com"
allowed_endpoint_hosts = [
  "fcm.googleapis.com",
  "updates.push.services.mozilla.com",
  "web.push.apple.com",
  "notify.windows.com",
]
```

The equivalent environment variables are documented in `.env.example`. The subject
must be a monitored `mailto:` address or an HTTPS contact URL. Add a host only when the
deployment intentionally trusts that Push service; subscription endpoints can make the
server perform outbound HTTPS requests, so arbitrary hosts are rejected.

## Delivery Rules

- Payloads omit sender and message details by default. Each browser can explicitly
  enable a minimal summary in preferences.
- Room notification level and mute expiry are checked immediately before every send.
- A notification creates jobs only for devices subscribed at that time; old events are
  not replayed to newly subscribed browsers.
- HTTP 404/410 and malformed browser keys remove only that device subscription.
- Transient failures retry with bounded exponential backoff. The default is five
  attempts, with outbound requests limited to ten seconds.
- Logging identifies internal job and notification IDs only. Capability endpoints,
  encryption keys, VAPID credentials, and notification bodies are excluded.
