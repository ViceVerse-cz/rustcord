# Chat timeline

Messages now show UTC times and date separators. Consecutive messages from the same author
within five minutes share a compact layout, except replies, unsupported
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

September 10 hover update: continuation timestamps appear in the avatar gutter only on hover
or keyboard focus. Rows use the shared subtle surface color; an overlay at the upper right
provides Add reaction, Reply, own-message Edit and the existing More menu. Tab focuses a
message and then its actions. Open menus remain available while the pointer leaves the row.
Hover does not change wrapping or cached heights. Edited continuations stay grouped and keep
an explicit edited label. Empty reaction rows no longer reserve space.

Native synthetic before/after evidence is in `docs/pr-evidence/message-hover/`; this supersedes
the earlier computer-use capture blocker. See `docs/performance.md` for the measured comparison.

Windows/Linux presentation, actual screen-reader/IME behavior and live Discord exchange remain
unverified for this slice. The previously recorded owner report that reacting can lose chat
messages has not been reproduced or resolved by this timeline change.

September 10 image/spacing update: adjacent image attachments share two-column rows, falling
back to one column below 280 points of content width. Previews preserve aspect ratios and
open the existing viewer individually; image filenames appear in accessibility labels and
the viewer, not as chat captions. Non-image files retain their names and download controls.
The gallery reuses the bounded media cache and the existing spoiler reveal boundary.
Markdown's final block newline no longer creates an empty line beneath messages; internal
line breaks and the existing author/date/reply/unread grouping boundaries remain intact.
The default offline fixture now includes a second image to demonstrate the gallery.


The More menu also exposes Delete for another author's loaded guild message when effective
MANAGE_MESSAGES access allows it. It opens the existing confirmation; changing permissions before
confirmation disables deletion. Copy/reply and author-only edit behavior remain available as before.
Non-deletable/unknown service message types do not gain a working Delete action.
