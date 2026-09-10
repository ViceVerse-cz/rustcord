# Chat timeline

Messages now show UTC times and date separators. Consecutive messages from the same author
within five minutes share a compact layout, except replies, edited messages, unsupported
content and unread boundaries. Each message retains its copy/reply/edit/delete/read actions
and the existing reaction controls. Reply previews use only loaded content, truncate at
120 characters and conceal spoilers; missing originals show an explicit history placeholder.

The new-message divider uses the existing service read marker. Unknown read state produces
no divider. Scrolling does not acknowledge messages: the existing explicit “Mark read through
here” action remains authoritative. Service behavior retains the unofficial/live-unverified
classification documented in [compatibility](discord-compatibility.md).

Scrolling upward near the top requests the next bounded page through the existing history
command. The manual Earlier messages action remains available. Older-page admission sets
retention direction before live arrivals can evict the reading window; cancellation preserves
that direction until a recent-page request. Jump to latest reloads recent history when needed,
and explicitly overrides persisted scroll position. Reload keeps current content visible while
revalidating it. Search-result navigation keeps its requested message anchor.

The existing 500-message / 4 MiB timeline, bounded formatting/media caches and offscreen
overscan remain. Row-height keys include grouping, date and unread separators. No dependency,
remote endpoint or persistent storage allowance was added. Times use Discord's documented
[snowflake timestamp](https://docs.discord.com/developers/reference#snowflakes), checked
September 10, 2026; the five-minute grouping threshold is a local presentation choice.

## Reproduction and evidence

- `cargo run --locked -p serein -- --demo --demo-chat`: fixed synthetic conversation with
  grouped authors, two UTC dates and a known read marker. Both flags are required; `--demo-chat`
  alone does not enable offline mode.
- `cargo run --locked -p serein -- --demo`: existing mixed-content fixture. Scroll up to load
  history, then use Jump to latest. Use a message's actions to mark read explicitly.
- Offline egui tests exercise 500 rows at 900-point/dark and 360-point/light widths, bounded
  visible-row measurement, prepend anchoring, explicit jumps, grouping/date/unread boundaries,
  edited-label preservation and spoiler invalidation. Cache tests exercise live arrivals during
  pending/canceled older requests and restoring recent-page retention.

Native screenshot/interaction verification is blocked: the installed computer-use tool returns
`Sky Computer Use native pipe startup failed`, including after a session reset. An inherited
before-only image from e9fb3e4 was inspected and preserved locally in
`target/chat-original-before.png`; it is not a valid comparison against the updated 74c0d79
baseline and is excluded from the PR. No final screenshot or visual parity claim is fabricated.
The first tool selection auto-launched the login gate; it was closed without login interaction.
Subsequent preview launches explicitly use `--demo`.

Windows/Linux presentation, actual screen-reader/IME behavior and live Discord exchange remain
unverified for this slice. The previously recorded owner report that reacting can lose chat
messages has not been reproduced or resolved by this timeline change.
