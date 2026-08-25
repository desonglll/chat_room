# TOON for AI Conversation Context

Research snapshot: 2026-08-25

## Executive conclusion

TOON is a good fit for sending a bounded chat transcript to an LLM because chat messages naturally form a uniform array of records. It should be used as a server-side serialization layer, not as a security boundary or a browser-side export of private room history.

The important compatibility caveat is that the current official specification and TypeScript reference encoder are v4.1, while the official Rust crate is still a v3.0 implementation. The practical recommendation for this Axum application is therefore:

1. Build and authorize the context on the Rust server.
2. Encode a deliberately flat message schema with `toon-format = { version = "0.5.0", default-features = false }`.
3. Use TOON only as model input; continue to request and validate ordinary JSON or plain text model output.
4. Describe the integration as TOON v3-compatible until the Rust crate reaches v4.x. Add an encoder contract test so a later crate upgrade is deliberate.

This is a reasonable interim choice because v4's major compression additions concern nested field groups and keyed tabular objects. The official v4 migration guide states that output without those features remains byte-for-byte compatible with v3. A flat `messages[]` table does not need the new constructs. However, the application must not claim full v4.1 conformance while using the current Rust crate. [v4 migration guide](https://github.com/toon-format/toon/blob/03bedee56dd0ca2e324f1d1c23008335c0703016/docs/guide/whats-new-in-v4.md) [Rust README](https://github.com/toon-format/toon-rust/blob/2136cb1a35f3ed63be733bb74d36b89d3b4592dd/README.md)

## Current project status

- The official specification is version 4.1 dated 2026-07-26, but its status is still **Working Draft**. Its own status section says it may be updated, replaced, or obsoleted. [official spec](https://github.com/toon-format/spec/blob/d6db4b04303bdea132351ce45aed612311c850b2/SPEC.md#status-of-this-document)
- The official TypeScript/JavaScript package is `@toon-format/toon` 4.1.1 and is identified by the specification repository as the reference implementation. [package metadata](https://github.com/toon-format/toon/blob/03bedee56dd0ca2e324f1d1c23008335c0703016/packages/toon/package.json) [spec README](https://github.com/toon-format/spec/blob/d6db4b04303bdea132351ce45aed612311c850b2/README.md#resources)
- The ecosystem page lists `.NET`, Dart, Go, Java, Julia, Python, Rust, Swift, and TypeScript/JavaScript as official implementations. It labels Rust stable. [official implementations](https://github.com/toon-format/toon/blob/03bedee56dd0ca2e324f1d1c23008335c0703016/docs/ecosystem/implementations.md#official-implementations)
- The latest official Rust crate is `toon-format` 0.5.0. Its README explicitly says it implements specification v3.0, and its repository pins an older spec revision. It supports any Serde `Serialize` value through `encode_default`/`encode`, which makes it easy to use with this application's existing Rust message structs. [Rust crate API and version](https://github.com/toon-format/toon-rust/blob/2136cb1a35f3ed63be733bb74d36b89d3b4592dd/README.md#library-usage) [Rust Cargo metadata](https://github.com/toon-format/toon-rust/blob/2136cb1a35f3ed63be733bb74d36b89d3b4592dd/Cargo.toml)
- TOON is most effective for uniform objects. Its own documentation recommends compact JSON for deeply nested or non-uniform data and notes that CSV is smaller for purely flat tables, though without TOON's declared lengths and fields. [format trade-offs](https://github.com/toon-format/toon/blob/03bedee56dd0ca2e324f1d1c23008335c0703016/README.md#when-not-to-use-toon)

## Recommended context shape

Keep the serialized data narrow and uniform:

```json
{
  "conversation": {
    "id": "room UUID",
    "name": "display name"
  },
  "messages": [
    {
      "id": "message UUID",
      "sender": "display name",
      "sent_at": "RFC 3339 timestamp",
      "content": "message text"
    }
  ]
}
```

Expected TOON input:

```toon
conversation:
  id: room-id
  name: Project chat
messages[2]{id,sender,sent_at,content}:
  message-1,Ada,"2026-08-25T10:10:00Z","Please review the API contract"
  message-2,Bob,"2026-08-25T10:12:00Z","I'll send notes this afternoon"
```

The official prompting guide recommends fencing TOON input and identifying it as TOON; it says the declared `[N]` length and `{fields}` header help the model track the structure. [LLM prompting guide](https://github.com/toon-format/toon/blob/03bedee56dd0ca2e324f1d1c23008335c0703016/docs/guide/llm-prompts.md#sending-toon-as-input)

Attachments should be represented only by safe metadata such as `file_name` and `mime_type` when it helps answer the question. Do not place local storage paths, signed URLs, access tokens, room passwords, or internal user IDs into model context.

## Security and prompt-injection boundaries

TOON's quoting and escaping rules prevent message text from breaking the serialization structure. The specification also uses declared row counts and widths to detect malformed or injected rows in strict decoding. It recommends resource limits because declared sizes are attacker-controlled and the format itself places no limit on nesting or document size. [TOON security considerations](https://github.com/toon-format/spec/blob/d6db4b04303bdea132351ce45aed612311c850b2/SPEC.md#15-security-considerations)

Those protections do **not** make conversation text safe instructions. A participant can write `ignore previous instructions` in an ordinary chat message; it will be faithfully encoded as data, but an LLM can still follow it. This is indirect prompt injection, not a TOON parser failure. OWASP describes the root problem as instructions and data being processed together, recommends clear structured separation, and warns that retrieved content must be treated as untrusted. [OWASP LLM Prompt Injection Prevention](https://cheatsheetseries.owasp.org/cheatsheets/LLM_Prompt_Injection_Prevention_Cheat_Sheet.html)

The endpoint should therefore enforce these controls:

- Authorize the requesting user against the selected room before loading any message.
- Load only messages visible to that user, exclude recalled/deleted content, and respect the membership join boundary already used by room history.
- Cap both message count and encoded byte size. A count-only limit is insufficient when a single message can be large.
- Put policy and the user's question in proper system/user messages, and put the TOON transcript in a clearly delimited data block with an explicit instruction that transcript content is untrusted evidence, never instructions.
- Keep this assistant read-only. It should have no tools that can send messages, modify rooms, inspect other conversations, or access secrets.
- Treat provider errors as server logs only; return a generic error to the client. Enforce request timeouts and per-user cooldown/rate limits.
- Render the answer as escaped text or sanitized Markdown, and cap the response size.
- Make the external AI transfer an explicit user action and disclose that selected conversation content is sent to the configured model provider.

## Integration recommendation for this repository

The existing server already owns the right seam: it authenticates sessions, checks `message.send`, loads recent room history, calls `genai`, applies a timeout and cooldown, and hides provider errors. Extend that server-side AI module rather than sending history to Vue for encoding.

Suggested flow:

```text
Vue AI workspace
  -> POST /api/ai/conversations/:room_id/query { question }
  -> authenticate session and authorize room membership
  -> fetch bounded visible history oldest-first
  -> map database rows to flat AiContextMessage records
  -> Serde serialize with toon-format 0.5.0
  -> place fenced TOON in an untrusted-data section of the model request
  -> return a bounded plain-text answer
```

Use `default-features = false`; the Rust package enables a CLI/TUI feature by default, which would otherwise bring unrelated terminal dependencies into the server. [Rust Cargo features](https://github.com/toon-format/toon-rust/blob/2136cb1a35f3ed63be733bb74d36b89d3b4592dd/Cargo.toml#L23)

Do not use the TypeScript encoder in the browser. That would require transferring authorized history to a less trusted layer and would duplicate the server's context rules. If exact v4.1 output becomes a hard requirement before the Rust implementation catches up, use the official TypeScript package only in a trusted server-side sidecar, or temporarily keep compact JSON; do not hand-roll a partial v4 encoder.

## Tests required for a safe rollout

- Authorization: a user cannot query a room they cannot read.
- Visibility: recalled messages and messages before the user's allowed history boundary never enter context.
- Serialization: commas, pipes, tabs, quotes, newlines, Unicode, `#` prefixes, and strings that resemble booleans/numbers remain single field values.
- Bounds: message count, encoded bytes, question length, timeout, cooldown, and output length are enforced.
- Prompt contract: system instructions remain outside the transcript and explicitly mark transcript rows as untrusted data.
- Provider failure: missing/invalid credentials produce an actionable availability state without leaking provider response details.
- Version contract: a fixed flat-message fixture produces the expected TOON representation, documenting the v3-compatible dependency until a reviewed v4 migration.

## Source snapshot

The findings above were checked against these first-party revisions:

- TOON TypeScript/reference repository: `03bedee56dd0ca2e324f1d1c23008335c0703016`
- TOON specification repository: `d6db4b04303bdea132351ce45aed612311c850b2`
- Official Rust repository: `2136cb1a35f3ed63be733bb74d36b89d3b4592dd` (`toon-format` 0.5.0)

