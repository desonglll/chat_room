# ADR 0001: Persistent System Administrators

- Status: Accepted
- Date: 2026-08-27
- Task: SEC-101

## Context

System-wide administration is currently granted by comparing an authenticated
account's mutable username with `admin.usernames`. In an open-registration
deployment, an unclaimed configured name can be registered by an attacker.
Renaming or changing the capitalization of an account also must not change its
authorization.

Deployments additionally need to decide whether anyone may register, only an
invited person may register, or registration is unavailable.

## Decision

`system_admins` stores administrator membership by immutable `user_id` and is
the only authorization source after this migration. Grants, revocations,
bootstrap, and legacy imports append metadata to `system_admin_events`; these
records contain identifiers and action names, never credentials.

The server exposes authenticated administrator operations to list, grant, and
revoke administrators. Revoking the last administrator is rejected inside the
same database transaction. A system administrator cannot delete their account
until another administrator revokes that role, so foreign-key enforcement also
protects the last-administrator invariant.

The first administrator is authorized through the local
`server bootstrap-admin --username <name>` command. It only accepts an existing
account and only succeeds while no administrator or completed bootstrap exists.
There is no network bootstrap endpoint or reusable bootstrap secret.

For upgrade compatibility, `admin.usernames` is consumed once after migrations.
Only matching accounts that already exist at that moment are imported, then a
durable marker disables all future imports. An unclaimed configured username
therefore never grants authority to an account registered later. The setting is
deprecated and must not be used for new deployments.

`auth.registration_mode` accepts:

- `open`: normal public registration;
- `invite_only`: registration requires a valid, unexpired, one-time invitation;
- `disabled`: registration is rejected.

Administrators create invitations through an authenticated endpoint. Only the
hash is stored; the bearer value is returned once. Consuming an invitation and
creating its account happen in one transaction.

## Consequences

Authorization survives username display changes and capitalization changes.
Database restores preserve administrators and bootstrap completion. Operators
upgrading from username configuration must ensure those accounts exist before
the first upgraded startup, or use the local bootstrap command afterward.

The CLI requires direct database access and intentionally cannot bootstrap a
remote server over HTTP. Invitation delivery remains an operator concern; the
application does not log or resend invitation bearer values.
