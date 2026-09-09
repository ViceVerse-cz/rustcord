# Threat model

Protect session credentials, private messages, draft integrity, account isolation and platform permissions. Trust Discord as the service endpoint, but validate transport origins, lengths and response shapes. Server authorization is authoritative.

Credentials are non-serializable, non-Clone, redacted, zeroized in owned Rust buffers where practical, and saved only through the OS credential store. Temporary copies exist inside HTTP/WebSocket/JS/platform engines; no universal memory-erasure claim is made. A password is never entered into a Rust form. The login bridge is limited to its own newly-created Discord webview and never searches other apps or profiles.

Threats addressed: redirected Authorization leakage, header injection, bot-session substitution, oversized REST/WebSocket input, partial-update corruption, stale session callbacks, uncontrolled queues, automatic ambiguous-write retries, disk account mixing, plaintext-token files and background data collection.

Open risks: unofficial Discord account policy; third-party login page or platform-engine compromise; native dependency supply chain; untested embedded challenge/QR handoff; local users/backups reading unencrypted SQLite; sophisticated permissions/event gaps; incomplete event/subscription coverage; Markdown/font/accessibility coverage; and OS keychain denial/unavailability. No encryption is downgraded for voice; voice is unavailable.

Do not report secrets or raw HTTP/Gateway payloads. Error UI uses fixed categories. Default tests use synthetic markers and local transports. No live network account testing in CI.

Strict cargo-audit currently blocks release on transitive Linux GTK/glib advisory warnings: RUSTSEC-2024-0370 and RUSTSEC-2024-0429. No ignore list is configured. See progress.md.
