# Server categories

Unsupported channel rows now include a keyboard-focusable Open in Discord action (the arrow
button), including Stage/directory/unknown kinds and forum/media containers. It opens the
shared browser confirmation, even with no active timeline. It does not select history, join
voice or fetch anything. The action is disabled when current view permission or valid channel
metadata is unavailable. Loaded forum/media posts and Archive remain separate native actions.

Implemented September 10, 2026. The native sidebar groups server channels by the service's category IDs, sorts categories and their children by position with ID tie-breaking, and leaves channels with missing/non-category parents accessible above the category sections. Category buttons expand/collapse with the mouse or keyboard; they cannot select history. The selected channel remains visible when its category is collapsed. Voice/stage/directory kinds remain identifiable and disabled. Forum/media containers show their loaded thread posts but cannot select history themselves.

The existing READY snapshot supplies initial channel metadata. CHANNEL_CREATE adds navigation within the 4,000-entry account limit and a 4 MiB channel metadata ceiling; CHANNEL_UPDATE preserves absent fields and handles an explicit null parent separately; CHANNEL_DELETE removes the item and loaded child threads. Changes carrying permissions/flags retain the existing permission invalidation and history revalidation behavior. Role-based permission computation remains incomplete; categories confer no authorization. Collapse preferences stay in session memory, pruned against currently known categories. Sidebar rows are virtualized; no new runtime dependency or automatic history fetch is added.

Protocol evidence: Discord's [channel resource](https://docs.discord.com/developers/resources/channel#channel-object) documents type 4 categories, parent IDs, positions and ID ordering on ties. Its [Gateway channel events](https://docs.discord.com/developers/events/gateway-events#channels) document channel creation, updates and deletion. These developer references establish wire shapes, not approved normal-user API access. The application's normal-user Gateway remains unofficial and live-unverified.

Offline checks cover partial/null metadata, ordering ties, orphan children, collapsed selected channels, category selection rejection, keyboard activation, bounded navigation admission, and create/move/permission/delete dispatches through a loopback WebSocket. They contain synthetic data and never connect to Discord. Native screen-reader behavior and owner-controlled live category changes remain unverified.

## Loaded threads and forum posts

READY now merges guild thread arrays into bounded navigation. THREAD_CREATE/UPDATE/DELETE, THREAD_LIST_SYNC and owner removal through THREAD_MEMBERS_UPDATE update this local view; archive updates remove active threads. A sync replaces only the declared guild/parent scope. Omitted parent IDs cover the guild, while an explicit empty list covers nothing. Wrong-guild removals, malformed scopes, duplicate IDs and navigation collisions cannot delete unrelated entries. Removed selected threads cancel history/search, clear visible messages and reject late history responses while retaining unsent drafts.

Thread kinds 10-12 appear below their loaded text/announcement/forum/media parent in the same guild. Missing or malformed parents fall back to root rows without recursive traversal. Collapsing a category retains the selected thread and its parent. Selecting a thread uses existing text history/composer behavior. Forum/media labels say "loaded posts": this is not a complete active-thread directory. Join/create controls and subscription expansion remain unimplemented. Unknown-thread updates do not hydrate navigation; explicit archive reads can supply an archived conversation.

The synthetic demo adds an ideas forum with one post and a thread beneath getting-started. Headless checks cover hierarchy, orphan/cycle handling, collapse, keyboard post selection and scoped lifecycle events. Before/after native screenshots and actual narrow/light/dark/screen-reader interaction remain unavailable because desktop automation is paused after the owner's physical Escape stops. No real thread or account action was performed.

## Archived threads

The Archive button on loaded text, announcement, forum and media parents opens a bounded native browser. Public pages are available for those parent kinds; text parents also offer Joined private and Private. Service permissions remain authoritative. Reload requests the newest page, Older replaces it, and Retry repeats only the failed request. Loading, empty, exhausted, unavailable and rejected responses are visible. Search, pins and archives share one cancellable read slot, with no automatic paging or polling.

Open thread fetches ordinary history and admits at most one transient navigation entry. Opening does not join or reopen a thread. The header says "Opened from archive" to describe its origin, not assert its current remote status. Navigating away retires the transient metadata while preserving drafts and normal bounded cached history. Matching Gateway create/sync data can adopt it into active navigation; omission from an active-thread snapshot alone does not revoke an archived conversation. Explicit removal, permission invalidation and parent removal retain existing history safeguards. Snapshot mutations invalidate matching archive results before they can reopen stale metadata.

Archive pages retain at most 25 entries / 64 KiB, with a 512 KiB wire limit; replacing pages never accumulates a directory. Keyboard regression coverage exercises the forum Archive action, older/exhausted pages, Open history and Escape at 420x480 in light/dark headless egui. Demo archive entries are generated synthetic examples; native visual inspection and live interoperability remain unverified.


## Find conversation

Use the sidebar's Find conversation button or Ctrl+K (Command+K on macOS) to search the
conversations already loaded in this session. Query words match channel/server names and known
DM recipient names without case sensitivity. The current conversation appears first when it
matches; at most 20 results are shown, so type more of the name to narrow a large account.
Each result identifies its server or DM scope. Categories and unsupported channel kinds do not
appear; VIEW access is checked from current permission information.

Use Up/Down from the query, Tab between controls, Enter to open, or Escape/Close to cancel.
The picker waits for input-method composition to finish before activation or dismissal.
Cancellation restores prior focus. Selecting a text conversation focuses its composer, preserves
drafts and unfinished edits, and uses normal history/navigation cancellation. Choosing a voice
channel only opens its roster; joining remains a separate deliberate action. Selecting the
already active conversation closes the picker without clearing or reloading its history.
Search is local to supplied navigation; it does not fetch friends, members or missing channels.

Synthetic keyboard, pointer and composition tests cover the picker and shared composer. Native
screen-reader/IME behavior and screenshots remain unverified while desktop automation is paused.
