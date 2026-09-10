# Discord compatibility — checked 2026-09-10

Inline spoilers (September 10): Discord's [Spoiler Tags help](https://support.discord.com/hc/en-us/articles/360022320632-Spoiler-Tags)
documents paired pipe delimiters and exempts code blocks. The native timeline recognizes
bounded paired literal delimiters outside code, with up to 32 independently revealed text
regions. Hidden spans create a keyboard-focusable reveal control rather than hidden text,
links, references or emoji widgets. Spoiler attachments/cards keep a separate media reveal;
reply previews remain conservatively concealed. Parser limits use conservative concealment
instead of exposing uncertain source. This is a local Markdown subset, not full Discord parsing
parity; native accessibility and live rendering equivalence remain unverified.

Incoming typing (September 10): Discord's [Typing Start event](https://docs.discord.com/developers/events/gateway-events#typing-start)
documents channel/user IDs and a Unix-seconds timestamp. The decoder accepts only these fields
within 16 KiB and discards optional guild/member metadata; invalid signals are ignored. The
desktop retains at most eight current-conversation users for at most ten seconds, rejects old
timestamps and more than five seconds of future skew, and clears state on navigation, messages
from that author, disconnect and access/session changes. Names come only from existing DM,
People or timeline state; missing identities use a generic label. Signals never cause profile
requests, outgoing typing, subscriptions, storage writes or timeline re-layout. A separate
eight-slot lossy inbox preserves reliable-event capacity; duplicate/burst wakeups are bounded
to eight per two seconds. The UI schedules only the next visible expiry, with no dot animation.
Existing People typing subscriptions are unchanged; guild delivery may depend on that pane's
subscription. Service availability and normal-account acceptance remain live-unverified.

Unsupported ordinary-message content (September 10): the [Discord message resource](https://docs.discord.com/developers/resources/message)
documents poll, sticker_items, deprecated stickers, components and IS_COMPONENTS_V2 (1 << 15).
The decoder now preserves only independent presence markers for these sources through full
messages, absent/null partial updates and bounded local cache reloads. It keeps no poll answer,
sticker or component payload. Native static Poll/Sticker/Components placeholders accompany any
supported text/media and share one confirmed Open in Discord action. This implements recognition
and a fallback, not poll voting, sticker rendering or interactive components. Old cache rows
cannot recover metadata previously discarded and gain markers during ordinary history refresh.
The local decoder caps arrays at 100 objects, each direct object at 64 fields, within the existing
4 MiB wire limit; these are application bounds, not Discord quotas. Native/live behavior remains
unverified; the source documentation does not establish normal-account API acceptance.

External fallback (September 10): unsupported channel rows and message placeholders offer
Open in Discord through an explicit browser confirmation. URLs use the fixed Discord HTTPS
origin and typed guild/channel/message IDs; DMs use @me with the conversation ID. No content,
names, credentials or signed media URLs enter the route. Zero IDs, missing DM/guild identity
and unavailable view permission disable the action. Discord's [message-link help](https://support.discord.com/hc/en-us/articles/206346498-Where-can-I-find-my-User-Server-Message-ID)
documents linking to an accessible message/conversation; its [Trust and Safety example](https://support.discord.com/hc/en-us/community/posts/1500000159602-How-to-Properly-Report-People-On-Discord)
shows the guild and @me path structure on the older discordapp.com domain. The current
discord.com route is an integration assumption, not a new API guarantee. Headless tests cover
construction and explicit confirmation; native launch, browser account selection and destination
resolution remain owner-unverified. The browser uses its own session and Discord authorization.

Loaded threads (September 10): READY guild thread arrays follow the original [discord.py-self guild parser](https://github.com/dolfies/discord.py-self/blob/master/discord/guild.py); active create/update/delete, scoped sync, archive eviction and owner-removal handling are informed by its [dispatch implementation](https://github.com/dolfies/discord.py-self/blob/master/discord/state.py). Discord's [Gateway thread events](https://docs.discord.com/developers/events/gateway-events#thread-list-sync) document the guild/parent scope and membership fields. These primary sources establish wire evidence, not normal-account acceptance; no implementation blocks were copied. Active discovery is limited to service-supplied snapshots/events; explicit archive reads are described below, with existing subscriptions unchanged. Unknown updates do not hydrate a missing thread. See [native navigation scope](categories.md).

Serein is unofficial and not endorsed by Discord. No normal-user live session has been tested. Technical compatibility does not imply approval. Discord forbids normal-account automation outside its OAuth2/bot API and warns of account termination ([policy](https://support.discord.com/hc/en-us/articles/115002192352-Automated-User-Accounts-Self-Bots)); its [terms](https://discord.com/terms) also apply.

| Capability | Credential / evidence | Classification | Serein status / fallback |
|---|---|---|---|
| Supported sign-in | OAuth2 access token; [scopes](https://docs.discord.com/developers/topics/oauth2) | Documented for limited scopes; RPC/other scopes restricted | No full replacement-client grant established. No invented OAuth login |
| Developer session import | Normal-user session credential intentionally entered by its owner; [Abaddon source](https://github.com/uowuo/abaddon/tree/master/src/discord) reviewed today | Unofficial, unstable, account risk | Developer build only, memory-only, no extraction; actual service validation pending |
| Official login webview | The owner completes Discord’s hosted login; [Wry](https://docs.rs/wry/0.57.0/wry/) supplies an ephemeral platform webview | Official hosted UI, **unofficial/unstable handoff** | Implemented own-webview same-origin fetch/XHR header observation; synthetic bridge tests only; email, QR, MFA, CAPTCHA and passkeys are individually live-unverified |
| Saved login | Normal session credential in [OS keyring](https://docs.rs/keyring/4.2.0/keyring/v1/) | Native credential-store integration | Save after verified native readiness, restore at launch, delete on logout; real credential-store round trip not exercised |
| Account and guild summaries | Normal session `/users/@me`, `/users/@me/guilds`; [user API](https://docs.discord.com/developers/resources/user) and Abaddon | Public resource shapes; normal-session behavior unofficial | Experimental adapter, not live verified |
| Channels / existing DMs | Normal session; READY navigation and guild channel resource; Abaddon | Unofficial session snapshot | Fail visibly on oversized/incompatible metadata; no member scraping |
| Server categories | [Channel resource](https://docs.discord.com/developers/resources/channel), existing session snapshot and channel events | Documented metadata; normal-user delivery unofficial | Ordered collapsible headings, orphan fallback, partial create/update/delete; offline and keyboard tests; [details](categories.md) |
| Server icons | [Image reference](https://docs.discord.com/developers/reference#image-formatting), guild metadata and GUILD_UPDATE | Documented CDN path; nested READY properties unofficial | Cached static icons and hash updates, initials fallback, offline tests; [details](icons.md) |
| Message embeds | [Message resource](https://docs.discord.com/developers/resources/message#embed-object) | Documented attributes; normal-user delivery/proxy conversion unverified | Native cards, partial embed-only updates, suppression/spoilers, cached static service-proxy previews; video opens externally, [limits](embeds.md) |
| People / member pane | Normal session; [discord.py-self Gateway](https://github.com/dolfies/discord.py-self/blob/master/discord/gateway.py), [member-list identity](https://github.com/dolfies/discord.py-self/blob/master/discord/abc.py), [wire types](https://github.com/dolfies/discord.py-self/blob/master/discord/types/gateway.py) | Unofficial and unstable | On-demand opcode 37 with the required guild typing subscription, first 100 list positions, typed incremental operations, identity/request filtering and 15-second timeout; list identity resolves from current bounded permission metadata rather than the initial channel snapshot; DM recipients from READY. Missing metadata or unsupported replies show unavailable; live-unverified |
| Profile pictures | [Discord image formatting](https://docs.discord.com/developers/reference#image-formatting), [User resource](https://docs.discord.com/developers/resources/user#user-object) | Documented CDN paths and user metadata; normal-session acquisition unofficial | Static PNGs, credential-free requests, account-isolated disk cache, bounded decode/textures, fallback initials; offline transport/cache tests only |
| User mentions | [Message formatting](https://docs.discord.com/developers/reference#message-formatting) | Documented syntax; normal-user notification behavior unverified | Local @ autocomplete, clickable names/profile cards, exact user allowlists, bounded SQLite metadata; [tests and limits](mentions.md) |
| User profile cards | Normal session `/users/{id}/profile`; [public implementation](https://github.com/dolfies/discord.py-self/blob/master/discord/http.py) | Unofficial route and payload; live-unverified | Anchored popout with banner/avatar/bio/pronouns/badge artwork/server tag/theme colors/connections/mutual servers and retained presence/custom status, cancellable requests and visible failures; badge and server-tag CDN paths are observed, not documented; [evidence](profiles.md) |
| History, send, edit, delete, replies | Normal session; [message resource](https://docs.discord.com/developers/resources/message) and Abaddon | Documented bot-facing resource; user compatibility unofficial | Experimental text adapter; owner permissions remain server-authoritative |
| Realtime, resume | Normal session; [Gateway](https://docs.discord.com/developers/events/gateway), Abaddon Identify/READY | Documented lifecycle; normal-user Identify and dispatch differences unofficial | Bounded JSON transport; no compression requested; incompatible snapshots fail |
| Rate limits | Credential-specific response headers/body; [rate limits](https://docs.discord.com/developers/topics/rate-limits) | Documented; do not assume bot quotas | Conservative shared cooldown, no blind write retry |
| Nonce correlation | Message nonce; message resource | Documented finite deduplication; user applicability unverified | Correlate confirmations only; no automatic ambiguous resend or indefinite idempotency claim |
| Native message formatting | Existing message content; [pulldown-cmark source](https://github.com/pulldown-cmark/pulldown-cmark) and [egui LayoutJob](https://docs.rs/egui/0.36.2/egui/text/struct.LayoutJob.html) | Local rendering; no additional service capability | Bounded emphasis/code/quotes/lists/strike, inert HTML, explicit HTTP(S) link confirmation, up to 32 inline text spoilers with separate media reveal; CommonMark differs from Discord Markdown; no automatic previews |
| Read markers | Normal session; [discord.py-self HTTP](https://github.com/dolfies/discord.py-self/blob/master/discord/http.py) and [dispatch implementation](https://github.com/dolfies/discord.py-self/blob/master/discord/state.py) | Unofficial and unstable | Explicit per-message acknowledgement, bounded READY read state, MESSAGE_ACK and PASSIVE_UPDATE_V2; synthetic checks only, actual cross-device behavior unverified |
| Relationships | Normal session or restricted Social SDK scopes; OAuth scope table and Abaddon | Unofficial / restricted | Unsupported |
| Reactions | Normal session; [message reaction resource](https://docs.discord.com/developers/resources/message#reaction-object) and [Gateway reaction events](https://docs.discord.com/developers/events/gateway-events#message-reaction-add), rechecked September 10 | Documented routes/shapes; normal-user acceptance live-unverified | Native counts, eight-emoji picker, existing Unicode/custom emoji toggles, normal own-reaction PUT/DELETE and bounded message readback. Synthetic HTTP/Gateway/keyboard tests pass; no live validation |
| Upload / preview / save | Normal session + separate credential-free media transfer; [attachment reference](https://docs.discord.com/developers/resources/message#attachment-object) and [normal-user staged upload](https://github.com/dolfies/discord.py-self/blob/2ba64a9a997e151a9c259984e0a179b1fdf4aff4/discord/abc.py#L1551) | Documented attachment metadata; staged upload unofficial; service delivery/conversion live-unverified | One explicitly selected file up to 20,000,000 bytes, streamed upload/cancel and message confirmation; static previews, native viewer and explicit image/file Save As (1 byte through 100 MiB). Upload limits below; [preview/save limits](chat-images.md) |
| Conversation search | Normal session; [discord.py-self search flow](https://github.com/dolfies/discord.py-self/blob/master/discord/abc.py), [HTTP routes](https://github.com/dolfies/discord.py-self/blob/master/discord/http.py), [response types](https://github.com/dolfies/discord.py-self/blob/master/discord/types/message.py) checked September 10 | Unofficial and unstable | Search current conversation, newest/older result pages, snapshot snippets, history revalidation on Open message; indexing/permission failures are visible. Live-unverified |
| Pinned messages | Normal session; [Get Channel Pins](https://docs.discord.com/developers/resources/message#get-channel-pins) and [normal-user implementation](https://github.com/dolfies/discord.py-self/blob/master/discord/http.py) | Documented route; normal-user acceptance live-unverified | Manual 25-pin pages, Older/Retry, Reload newest and history-validated Open message; pin/unpin mutations unsupported |
| Threads / forums | Normal session; [navigation and archive scope](categories.md), archive sources below | Wire evidence checked September 10; normal-user behavior live-unverified | READY/thread-event reconciliation, parent/post hierarchy and manual public/private/joined-private archive pages implemented; complete active directory and create/join controls remain incomplete |
| One-to-one DM voice | Normal session + per-call credentials; [Discord voice](https://docs.discord.com/developers/topics/voice-connections), [DAVE](https://daveprotocol.com/), [discord.py-self signaling](https://github.com/dolfies/discord.py-self/blob/master/discord/gateway.py) | Voice transport/DAVE documented; normal-user DM entry/ringing unofficial; implementation live-unverified | Optional native Opus/CPAL/DAVE v1 path, call controls and bounded resume implemented; synthetic tests only. Physical audio and official-client two-way call gate blocked; default build text-only |
| Logout | Local credential/task disposal; OAuth revocation applies only to OAuth tokens | Local behavior implemented; remote session revocation unverified | Local logout only, does not delete sent messages or claim server token revocation |

`messages.read` refers to local RPC; it is not a general REST chat grant. `rpc` needs approved-partner access and depends on the official application. Social SDK relationship access does not establish general desktop-client chat access. No client secret, token broker, fingerprint imitation, native password endpoint, CAPTCHA/MFA workaround, or harvested QR flow is implemented. The owner explicitly requested the official login page in an authentication-only webview and secure saved login; OAuth is not the product login.

Discord states DAVE is required for the listed call types from March 1, 2026. libdave supplies encryption components, not capture, playback, codecs, jitter handling, or a complete call engine. No legacy unencrypted fallback is acceptable.

All implemented network features remain **experimental and live-unverified** until the manual gate in authentication.md passes. Public bot documentation is protocol evidence, not proof of normal-user acceptance. Abaddon was inspected for wire evidence only; no GPL source is copied into this dual-licensed implementation.

September 10 continuation: Gateway documentation was rechecked for heartbeat, Resume and invalid-session handling. Offline coverage now includes a real loopback WebSocket lifecycle, including non-resumable invalidation and credential-expiry close; no Discord connection was made. The localhost dial override is compiled only into unit tests and never changes production/resume origin validation. Native formatting, font glyph coverage and persisted appearance have offline tests; these do not expand the live-verified capability set.

## Authentication evidence and changed requirements

The owner superseded the initial storage/login constraints during implementation: official-login webview, saved OS credential-store token, and subsequently bounded local SQLite caches/drafts are now required/allowed. SPEC.md records the final policy.

[Discord Userdoccers’ original protocol research](https://docs.discord.food/authentication), checked 2026-09-09, describes normal-user password login, MFA tickets, and authentication tokens. It is unofficial evidence, not Discord approval. The final implementation delegates that login to Discord’s hosted UI instead of implementing these endpoints itself. Email/phone login, QR, TOTP, CAPTCHA, passkeys and device verification are offered only as the official page permits in the platform engine; none was tested with a real account here.

The handoff is original code in `crates/platform/src/login-handoff.js`. It observes an Authorization header only for this temporary webview’s own `https://discord.com/api/v*/` requests, after user-controlled authentication. It reads neither localStorage nor other applications, profiles or tabs, and does not inspect passwords or QR secrets. A random per-webview capability scopes the IPC handoff; Rust validates that capability, the IPC origin, and token length, then verifies the account via `/users/@me` and rejects bot accounts. This handoff is a technical hypothesis with offline tests; Discord’s current page may use an unsupported transport, reject the embedded engine, or change its behavior. Failure must not be called successful login.

Navigation is limited to Discord’s HTTPS origin; new windows and downloads are blocked. Third-party challenge subframes are left to the platform engine; popup-dependent methods may fail. No spoofed official client user agent/properties are supplied. Resume URLs are restricted to recognized Discord gateway hosts.

September 10 People/avatar continuation: the member subscription uses the opcode 14 shape found in the current original discord.py-self implementation. [Original lazy-guild research](https://arandomnewaccount.gitlab.io/discord-unofficial-docs/lazy_guilds.html) describes list positions including groups and ambiguous empty SYNC responses; it is unofficial evidence, not a service guarantee. A newer opcode 37 has also been [reported by Userdoccers](https://github.com/discord-userdoccers/discord-userdoccers/issues/191); Serein does not claim opcode 14 works for every account. Channel-specific list identities require the available everyone-role permissions and channel overwrites. The small noncryptographic Murmur3 identity calculation is implemented locally and checked against known vectors; no third-party client code blocks were copied. This identity selects a list; it grants no permissions. No complete member directory is fetched or persisted. Profile cards expose only available name, ID and avatar, not invented bios, roles or relationships. Server-specific custom avatars and full profile endpoints remain unsupported.

## DM voice evidence — September 10

Normal-user DM entry uses guild_id:null with main Gateway opcodes 13/4, based on the original [discord.py-self Gateway implementation](https://github.com/dolfies/discord.py-self/blob/master/discord/gateway.py). CALL_CREATE/UPDATE/DELETE and voice state/server shapes follow its [dispatch source](https://github.com/dolfies/discord.py-self/blob/master/discord/state.py) and [voice types](https://github.com/dolfies/discord.py-self/blob/master/discord/types/voice.py). Ring/stop-ringing use the [HTTP implementation](https://github.com/dolfies/discord.py-self/blob/master/discord/http.py); its [DM connect flow](https://github.com/dolfies/discord.py-self/blob/master/discord/channel.py) establishes voice before ringing. These are unofficial interoperability evidence, not approved normal-account APIs or a bot-token workaround. No source-code blocks were copied.

Discord's documented voice transport and [DAVE protocol](https://daveprotocol.com/) supply encryption/protocol requirements. Serein uses Davey 0.1.4/OpenMLS, an unofficial implementation, rather than claiming to ship Discord's libdave or an independently audited engine. Real MLS, DAVE, RTP, Opus and loopback WebSocket/UDP tests exercise the adapter with synthetic participants. They do not verify current Discord acceptance, microphone permission, device quality or a remote official client. Existing group DMs, Stage channels, video and screen sharing remain unsupported. Guild voice is implemented separately below, with its live gate still unverified. See [voice scope and owner-operated gate](voice.md) and [adapter details](../crates/discord-voice/README.md).

Image attachments: documented wire metadata and spoiler bit 3 are implemented; additional sensitive flags and proxy PNG conversion rely on unofficial implementation evidence. Native viewing and bounded cache/patch behavior have offline coverage; actual account image delivery remains unverified. [Scope and sources](chat-images.md).

Read-state continuation (September 10): channel read cursors and latest-message IDs drive boolean unread badges; absent service read state remains unknown. The message menu sends one acknowledgement only after an explicit action on a loaded message in a fresh authenticated view. No scroll-triggered acknowledgement, mention-counter rewriting, bulk acknowledgements or outgoing mark-unread action is implemented. Incoming manual mark-unread updates are honored. A later Gateway update wins over an in-flight HTTP completion. Snapshot/update lists are capped at 4000 entries, retained cursors are limited to known navigation channels, and only one write is pending. A legacy acknowledgement token, if supplied, is capped at 2048 bytes in zeroized session memory; the response body is capped at 4096 bytes. Neither cursors nor acknowledgement tokens are written to SQLite. The linked primary client implementation supplies unofficial wire evidence; local HTTP/WebSocket fixtures do not establish Discord acceptance.

Search continuation (September 10): guild conversations use the guild search route with an exact channel filter; DMs use the channel route. Search content is percent-encoded, with timestamp-descending order and explicit max_id pagination. One replaceable task uses existing REST permits, deadlines and cooldowns. Indexing responses require another deliberate Search action after the service delay; no automatic polling, broad account search, advanced filters, NSFW override or search-result persistence is implemented. Service totals and partial-index status are displayed as supplied, not asserted complete. Opening a result fetches up to 50 history messages ending at that ID and positions the timeline there; unavailable results are reported. Existing reload returns to latest history. Search snapshots are cleared on relevant edits/deletes, navigation, disconnect, permission invalidation and logout. Original-client sources supply wire evidence only; no source-code blocks were copied and no authenticated service request was used as validation.

Login compatibility correction (September 10): READY read_state accepts both the legacy array and the versioned entries/version/partial object, under the same 4000-entry bound. Serein's Identify does not request the versioned_read_states capability; rejecting the legacy shape previously rejected the entire login payload. The capability's effect is described in the original [discord.py-self capability definitions](https://github.com/dolfies/discord.py-self/blob/master/discord/flags.py), rechecked September 10. Partial snapshots leave omitted channels unknown. Identify capabilities remain unchanged. Static error labels distinguish account verification, Gateway discovery, READY decoding and connection setup without exposing payloads, credentials or remote error text. Synthetic regression and loopback evidence do not establish actual account login success.

## Reaction refresh and pinned messages - September 10

Reaction readback now uses a history request with limit=1 and around=the exact message ID, matching the current normal-user [discord.py-self get_message implementation](https://github.com/dolfies/discord.py-self/blob/master/discord/http.py). The prior single-message GET can reject normal-user sessions; its Forbidden result caused the existing channel-invalidation policy to clear the conversation. Exactly one matching channel/message record is accepted. Missing targets, neighboring messages and malformed/oversized-count replies leave reaction state unavailable without substituting content. Genuine Forbidden history responses still revoke the channel and its cached history. No write is automatically retried.

Pinned-message browsing uses GET /channels/{channel}/messages/pins?limit=25, following [Discord's Get Channel Pins reference](https://docs.discord.com/developers/resources/message#get-channel-pins) and the primary normal-user implementation's [pins_from request](https://github.com/dolfies/discord.py-self/blob/master/discord/http.py) and [pin iterator](https://github.com/dolfies/discord.py-self/blob/master/discord/abc.py), rechecked September 10. Older pages send the last pin's timestamp as the ISO8601 before cursor, independently of message IDs. Nanosecond precision is retained; each page must advance in pin order. Pages replace the previous 25-item snapshot, with explicit Older pins, Retry older pins after failure, and Reload to newest. Returned has_more controls continuation; no complete/stable listing is promised while remote pins change. Summaries remain text-only with concealed spoilers. No pin/unpin mutation, background polling or automatic acknowledgement is implemented. Open message revalidates normal history. Search, pins and archives share one cancellable task/result slot; none of these snapshots is persisted. No owner-controlled live service test has validated these changes.

## Single-file uploads - September 10

The selected file is uploaded only after Send. Serein requests a staging target with authenticated `POST /channels/{channel}/attachments`, using `files:[{id:"0",filename,file_size}]`; streams a credential-free PUT to its `upload_url`; then creates a message with `attachments:[{id:"0",filename,uploaded_filename}]`. Existing content, reply, explicit mention allowlists and nonce correlation are preserved, including attachment-only messages. This sequence follows the primary [HTTP implementation](https://github.com/dolfies/discord.py-self/blob/2ba64a9a997e151a9c259984e0a179b1fdf4aff4/discord/http.py#L1073), [route](https://github.com/dolfies/discord.py-self/blob/2ba64a9a997e151a9c259984e0a179b1fdf4aff4/discord/http.py#L1527) and [file serialization](https://github.com/dolfies/discord.py-self/blob/2ba64a9a997e151a9c259984e0a179b1fdf4aff4/discord/file.py#L197), inspected at commit `2ba64a9a997e151a9c259984e0a179b1fdf4aff4`. No implementation source was copied. Public bot multipart examples do not verify this normal-user path.

Only `https://discord-attachments-uploads-prd.storage.googleapis.com` on effective port 443 is accepted. This exact origin was independently observed in the public `Content-Security-Policy` returned by unauthenticated `curl.exe --silent --head https://discord.com/app` on September 10. Userinfo, fragments and redirects are rejected; signed path/query values remain opaque, bounded and unlogged. A separate HTTP client sends neither Discord authorization nor cookies to storage. Test-only loopback origins are absent from shipped builds.

The local limit is **one nonempty regular file, at most 20,000,000 bytes**. This is Serein's conservative cap, not an inferred account entitlement. Discord's [File Attachments FAQ](https://support.discord.com/hc/en-us/articles/25444343291031-File-Attachments-FAQ), updated August 13, 2026, states a 20 MB free upload limit and larger paid limits; server permissions, experiments and rejections remain authoritative. Serein does not detect paid limits or compress/rewrite files.

Application chunks are at most 64 KiB; negotiation and storage responses at most 64 KiB, signed URLs 4096 bytes, server upload names 1024 bytes, local paths 4096 encoded bytes and filenames 256 UTF-8 bytes. One upload job uses latest-value progress, not an expanding event queue. The PUT has a 300-second overall deadline, 10-second connection timeout and 30-second read timeout. Filesystem work stays outside rendering.

Path and open-file size/modification metadata are checked before transfer and again before message creation. Missing or observably changed files fail explicitly. These checks are not an immutable snapshot or a defense against a writer restoring identical metadata; no hidden recovery copy is created. Cancellation before message creation prevents the message POST, but staged bytes already sent may remain remotely; Serein does not claim remote deletion or a retention deadline. Cancellation after message POST begins reports an unknown outcome. No transfer or message write is automatically retried. Signed upload targets and local source paths are session-only.

Synthetic tests cover actual loopback HTTP, credential isolation, redirects, file bounds/changes, cancellation during PUT and cancellation during message creation. They do not establish normal-user interoperability: an owner-controlled developer-session test with an official-client recipient remains required. Multiple attachments, tier-dependent larger files and remote staging cleanup remain unimplemented.

File selection also accepts one native file dropped into the active conversation window. The pinned egui 0.36.2 DroppedFileHandle exposes a path; Serein moves the event handles, accepts only one absolute path within the existing path limit, and never invokes their whole-file bytes API. Drops reuse the picker validation and cancellation slot. They cannot replace an existing selection or active operation and never start an upload themselves. Unsupported/multiple drops and unavailable conversation states report an error. A composer hover hint explains the limit and explicit Send behavior. Native OS drag/drop delivery remains unverified; synthetic handle admission and late-result isolation are tested.

Archived-thread browsing (September 10): [Discord public/private/joined-private archive endpoints](https://docs.discord.com/developers/resources/channel#list-public-archived-threads) describe public and private pages ordered by archive timestamp with an ISO8601 before cursor; joined-private pages use descending thread IDs and a snowflake cursor. The primary normal-user implementation exposes the [same three HTTP routes](https://github.com/dolfies/discord.py-self/blob/master/discord/http.py) and [channel archive selection](https://github.com/dolfies/discord.py-self/blob/master/discord/channel.py). These sources supply wire evidence, not live account acceptance. Serein makes only explicit GET requests, limits each page to 25 entries, checks archived metadata, guild/parent/type, duplicates and cursor progress, and ignores member summaries. Public/private timestamps preserve nanoseconds. Private archive enumeration requires the service permissions described by Discord, including MANAGE_THREADS; joined-private is a separate choice. Open loads history without a join/reopen mutation. No live account was used to validate these routes.

### Unicode emoji artwork (September 10, 2026)

Formatted messages (including existing formatted embed/search surfaces), Unicode reaction
counts and the reaction menu use bundled [Twemoji 17.0.3](https://github.com/jdecked/twemoji/releases/tag/v17.0.3)
artwork from Twitter and contributors, under CC BY 4.0. This is a local rendering feature,
not a new protocol endpoint or proof of matching Discord's current artwork revision.
Grapheme matching supports flags, modifiers, keycaps and ZWJ sequences, including optional
emoji presentation selectors. Code, explicit text-presentation and unknown sequences remain
literal. Original message/reaction strings and outgoing requests are unchanged. Whole-message
Copy preserves the original text; drag-selection of rendered text excludes inline image widgets.
The editable composer continues to use native font text. Custom server emoji and animation
are not supplied by Twemoji and retain their existing text fallback. Validation is synthetic;
no live Discord session was used.

### Custom server emoji, chat picker and copying (September 10, 2026)

The current server's catalog is received from READY and known-guild GUILD_CREATE, updated
by GUILD_EMOJIS_UPDATE, and cleared on GUILD_DELETE. The documented emoji fields and update
shape are supported by [Emoji Resource](https://docs.discord.com/developers/resources/emoji)
and [Gateway Events](https://docs.discord.com/developers/events/gateway-events#guild-emojis-update).
Normal-user READY remains unofficial/unstable; this change was tested with synthetic events
and local sockets, not a live account. Joining new guilds' full navigation remains pre-existing
unsupported behavior. Missing catalogs are displayed as unavailable rather than empty.

Formatted message/profile/embed text renders `<:name:id>` and `<a:name:id>` as static CDN
images, using the documented [custom emoji CDN endpoint](https://docs.discord.com/developers/reference#image-formatting-cdn-endpoints).
Custom reactions use the same bounded media worker. Deleted/failing previews have a fixed
placeholder and retain copyable original markup. Standard Unicode and custom image widgets
participate in text selection: copying a selection retains Unicode sequences/custom markup,
and right-click Copy emoji copies the entire token. Selection endpoints treat each image as
one item, avoiding broken ZWJ sequences or partial custom markup. Whole-message Copy is unchanged.

The chat Emoji button opens a searchable Unicode/name palette and current-server tab. Choosing
inserts at the saved text cursor or replaces its selection, preserves Unicode presentation
selectors, and records the draft without sending. Escape/close restores keyboard focus.
Only catalog entries explicitly available, unmanaged and unrestricted by roles are enabled;
unknown eligibility remains disabled. Cross-server/DM catalog selection and full role/Nitro
entitlement inference are not implemented; the service remains authoritative for actual sends.
Animated emoji are inserted with their original animated markup and shown as still previews.
The composer now displays known user mentions as `@name`, Unicode as bundled Twemoji, and custom emoji as static server artwork, while retaining original wire text for editing/copy/send. Unresolved user IDs remain literal; unavailable server artwork shows its name.


## Server voice — September 10, 2026

Guild voice entry uses the documented [Gateway voice state update and voice allocation flow](https://docs.discord.com/developers/topics/voice-connections): guild-scoped join/mute/leave, matching owner session plus guild server update, and guild ID as voice Identify/Resume server ID. The DAVE group remains identified by channel ID. This protocol documentation is not approval or live evidence for normal-user accounts. Server channel rosters use READY, GUILD_CREATE, VOICE_STATE_UPDATE and unofficial READY_SUPPLEMENTAL/PASSIVE_UPDATE_V2 shapes from the original [discord.py-self dispatcher](https://github.com/dolfies/discord.py-self/blob/master/discord/state.py).

The optional media engine extends the existing DAVE/Opus path to bounded group membership and simultaneous remote audio mixing. Server mute/deafen gates local media. Empty rooms wait without opening audio devices; microphone capture requires an established encrypted group. Stage channels and group DMs remain visibly unsupported. Main Gateway disconnect, channel/session moves, permission invalidation or voice server migration stop the current media session and require deliberate rejoin. No automatic call or DM ringing is triggered by viewing a roster or joining a guild channel.

The implementation is tested with synthetic protocol/crypto/UI data. The owner-controlled official-client two-way audio, multi-party join/leave, permission, device and network tests in [voice.md](voice.md) remain required before claiming working live interoperability.

### Server people subscription repair (September 10, 2026)

The member pane previously sent deprecated opcode 14 with `typing:false`. Current
[Gateway implementation](https://github.com/dolfies/discord.py-self/blob/master/discord/gateway.py)
and [channel subscription prerequisites](https://github.com/dolfies/discord.py-self/blob/master/discord/state.py)
show opcode 37 and a guild typing subscription before requesting channel member ranges.
Serein now enables that subscription only for the active member pane and clears it with
channel ranges when the pane closes or navigation changes. This receives typing events;
it does not send typing notifications. Incoming typing for the selected conversation is
handled as described below; other conversations' typing is discarded.
The request still covers only positions 0–99, with the existing 128-KiB retained member
budget, request/list identity filtering and timeout. No full-directory fetch was added.

A read-only observation of the owner's already-open app confirmed an unavailable pane
with a nonzero server total. That is reproduction evidence, not successful validation of
this repaired build. Tests use a synthetic local WebSocket; normal-user acceptance of the
new request remains unverified. Role display is **not implemented**: only role permissions
for list identity are read; member role IDs, group headings and role colors are discarded.
Role administration is outside the product scope.


## Channel visibility and history revocation - September 10, 2026

The documented [CHANNEL_OBFUSCATED flag](https://docs.discord.com/developers/resources/channel#obfuscated-channels)
(bit 17 / 131072), described in the [obfuscation change log](https://docs.discord.com/developers/change-log#channel-obfuscation-for-users-and-bots),
is authoritative for this path. Placeholder names are not permission evidence. READY omits flagged
channels and threads under flagged parents; an independently visible child of an obfuscated category
remains visible. CHANNEL_CREATE/UPDATE and guild channel snapshots revoke flagged IDs through the
existing removal path. Thread create/update/archive admission follows the same flag boundary.
Raw navigation item and duplicate-ID limits apply before filtering. Thread sync carries explicit
hidden-ID removals in the same bounded event, including a currently browsed archived thread;
ordinary absence from an active-thread snapshot still preserves that archived view.

An unflagged guild CHANNEL_UPDATE with a valid ID, type and name can restore missing navigation
only for an already loaded guild. Optional flags, position and parent fields need not be present.
Existing channels still receive absent/null-aware patches; restoration does not replace their
omitted fields, rejoin voice or accept an old history result. Incomplete updates wait for a full
update or READY. No guild identity is guessed from an obfuscated payload.

Accepted READY replaces the readable navigation set and cancels prior history requests. Reload
requires a currently loaded text channel, as do HTTP results and disk-cache hydration. Revocation
clears the active view and ends affected voice state; restoration requires deliberate selection or
join. Voice allowances and guild roster snapshots exclude obfuscated channels.
This closes explicit visibility and stale-response paths, not the full role/overwrite permission
mirror. Service permission failures remain authoritative; normal-user live behavior is unverified.

## Permission-aware actions (September 10, 2026)

The session now mirrors the current account's guild owner, roles, self-member role IDs and
timeout, and channel role/self overwrites. Unknown metadata is distinct from a known zero-bit
result and cannot enable guild actions. Owner/administrator bypass, everyone then combined
role then self overwrites, and timeout restrictions follow the documented
[permission calculation](https://docs.discord.com/developers/topics/permissions).
Threads use their loaded text/announcement/forum/media parent's overwrites and
SEND_MESSAGES_IN_THREADS; category overwrites are not recursively applied to children.
Existing DM/group-DM text access remains service-authoritative without guild metadata.

READY and known-guild GUILD_CREATE install bounded snapshots. Role create/update/delete,
self GUILD_MEMBER_UPDATE, owner and channel overwrite updates replace the relevant metadata;
READY_SUPPLEMENTAL and PASSIVE_UPDATE_V2 can update already supplied self-member data.
No full member request or new subscription is added. Normal-user evidence is pinned to
[READY merged members](https://github.com/dolfies/discord.py-self/blob/2ba64a9a997e151a9c259984e0a179b1fdf4aff4/discord/state.py#L1731),
[guild owner properties](https://github.com/dolfies/discord.py-self/blob/2ba64a9a997e151a9c259984e0a179b1fdf4aff4/discord/guild.py#L691),
and [member updates](https://github.com/dolfies/discord.py-self/blob/2ba64a9a997e151a9c259984e0a179b1fdf4aff4/discord/member.py#L375),
plus the [documented member event](https://docs.discord.com/developers/events/gateway-events#guild-member-update).
These are wire references, not live account acceptance or a guarantee of event delivery.
Absent incremental fields preserve known state; explicit null makes roles/overwrites/owner
unknown or clears a timeout. A full self-member snapshot without a timeout means no timeout.

Shared command/UI checks cover sending, attachment selection/drop/upload, own-message editing
and deletion, reactions, history/search/pins/archive reads, and voice admission/capture.
VIEW without READ_MESSAGE_HISTORY permits fresh live messages and separately authorized sends,
but cannot fetch, seed or persist history. Permission loss clears the affected loaded view,
pending reads, result pages and reply target; late content cannot restore it. Recovery drafts
remain. Read restoration requires deliberate reload/reselection. New reactions require
ADD_REACTIONS; an existing emoji can be added without that bit, with history and timeout checks.
Removing one's existing reaction remains separate. Private archive enumeration additionally
requires MANAGE_THREADS. Voice CONNECT permits listen-only joining; SPEAK gates capture;
without USE_VAD, focused push-to-talk must be enabled and held. Restoration never rejoins.

Discord remains authoritative for action rejection, private-thread membership, account emoji
entitlements and delivery of permission updates. Role names/colors, role administration,
moderating other users' messages, new guild joining, and full member-directory synchronization
are outside this slice. Synthetic parser, localhost transport and reducer/UI guard tests do
not establish live compatibility. Native automation remains paused after owner Escape stops;
no live account action, microphone capture or new native screenshot was performed.

## Composer and notifications — September 10, 2026

See [notifications](notifications.md) for service badge/read-state reconciliation, focused
view ACKs, session-only native opt-in and fail-closed mute/DND handling. READY read-state
counts, settings and session presence use isolated unofficial normal-user wire shapes.
Synthetic protocol/UI tests are not live Discord validation. Role/everyone mention events,
blocked relationships and complete protobuf notification preferences remain unsupported.


### Loaded People presence (September 10, 2026)

The open guild People pane consumes standalone PRESENCE_UPDATE for users in its current
100-row subscription mirror. [Discord's presence event](https://docs.discord.com/developers/events/gateway-events#presence-update)
documents partial user objects and online/idle/dnd/offline status. The pinned normal-user
[dispatcher](https://github.com/dolfies/discord.py-self/blob/2ba64a9a997e151a9c259984e0a179b1fdf4aff4/discord/state.py#L2040)
allows an optional guild scope. Serein ignores guildless updates and does not request a
friends/global directory or additional subscription flags. These are primary wire references,
not proof that a normal-user account receives these updates through this client's subscription.

Absent status preserves the known value; explicit null or unknown strings clear it to
Presence unavailable. Online, Away, Do not disturb and Offline are service-reported labels;
Offline does not distinguish an invisible user. Custom activity type 4 is normalized using the
same parser as member snapshots: at most 128 characters / 512 UTF-8 bytes, with optional Unicode
emoji and no controls. Omitted activities preserve known custom text; null, empty or no custom
activity clears it. These absent/null choices are defensive client policy, not a documented
normal-user delivery guarantee. Other activities, partial profiles and device status are
discarded; the existing subscription still sends activities=false. Bursts coalesce within a fixed 100-ms window; stale request/session/access
updates cannot modify the pane. Self-session DND notification suppression keeps its separate
existing path. Synthetic localhost Gateway, reducer and headless UI tests supply local evidence;
normal-account delivery and native screenshots remain owner-controlled validation gates.


### Keyboard conversation navigation (September 10, 2026)

Find conversation / Ctrl+K (Command+K) is a local picker over existing loaded navigation and
current VIEW decisions. It adds no Discord route, subscription or relationship lookup. Selection
reuses the same history/roster path as the sidebar, including cancellation and service-authoritative
permission failures. A voice result only opens the roster and does not join. The query is session
UI state and never enters message content, REST requests or SQLite. Headless keyboard/IME checks
are synthetic; real platform input, screen readers and live navigation remain unverified.

## September 10: native system-message descriptions

Discord's [documented message type IDs](https://docs.discord.com/developers/resources/message#message-types) were checked on 2026-09-10. The decoder now retains the type through REST/Gateway messages and the local history cache. The timeline describes joins/welcomes, recipient changes, calls, channel name/icon changes, pins, boosts/tiers, channel follows, discovery notices, threads, invite reminders, AutoMod, subscriptions/offers, Stage events, incident alerts, purchases and poll results. Original content/embeds/attachments still render separately. Copy and loaded reply previews include the description. Unknown types keep an explicit placeholder; legacy cached unsupported rows remain unknown until history revalidation.

These are native textual descriptions, not full interactive cards or proof of normal-user protocol compatibility. Call outcome/duration, subscription details, missing thread content and poll votes are not inferred. Search/pins snapshot excerpts remain their existing content-only previews; opening a hit loads the described timeline message. No live account, call or microphone validation was performed. Open PR #27 adds the external fallback and #28 adds unsupported payload markers; neither implemented these descriptions. Integration must retain their controls/markers without restoring a generic system placeholder for recognized types.


### Reply targets (2026-09-10)

[Discord's message reference documentation](https://docs.discord.com/developers/resources/message#message-reference-structure)
distinguishes an absent referenced_message (unknown) from explicit null (deleted), and supplies
channel IDs on received references. This client accepts navigation for type 19 replies and type
23 context-menu message references only, with default reference type 0, the same channel and a
positive earlier message ID. Crossposts, forwards, thread-parent references and unknown reference
shapes stay non-navigable, retaining their system description or unsupported fallback. Nested original bodies are discarded with a
bounded object visitor rather than recursively hydrated. One existing history page before
target+1 retrieves an unloaded original; there is no bulk search or background traversal.
This is public protocol evidence, not proof of normal-user endpoint acceptance. Live validation
is unperformed; offline regressions cover reference shape, missing/null data and deletion races.


### Server member identity after hydration (September 10, 2026)

A guild refresh triggered by subscribing can recreate channel navigation objects without
the READY-only member-list ID. Member requests now compute that ID from the existing
bounded role/overwrite mirror, so GUILD_CREATE, newly delivered/restored channels and
Reload people use current metadata. A change in list identity retires the active request
and lets the visible pane request again; unchanged metadata preserves pending replies.
Missing metadata still means unavailable; threads retain their separate-protocol limitation.
The shared hash accepts the same u128 permission values as the permission parser.

The original [subscription lifecycle](https://github.com/dolfies/discord.py-self/blob/2ba64a9a997e151a9c259984e0a179b1fdf4aff4/discord/state.py)
and [list identity algorithm](https://github.com/dolfies/discord.py-self/blob/2ba64a9a997e151a9c259984e0a179b1fdf4aff4/discord/abc.py)
were rechecked. This changes local identity selection, not the opcode, requested ranges,
permissions or account access. Synthetic regression tests establish the hydration bug
and its repair, not acceptance by Discord; owner-operated live verification remains unrun.


### Member list row headers and refresh bursts (September 10, 2026)

The original [Gateway wire types](https://github.com/dolfies/discord.py-self/blob/2ba64a9a997e151a9c259984e0a179b1fdf4aff4/discord/types/gateway.py)
distinguish top-level group summaries (ID plus count) from group items inside SYNC ranges
(ID only). Serein incorrectly required a count in every group item, rejecting the complete
member update. It now reads only the group ID needed to recognize an index placeholder;
counts are not fabricated. An exact synthetic ID-only-header regression loads the following
member at the correct position. The owner-run redacted trace confirms that member replies
were rejected by the decoder; the owner subsequently confirmed the repaired list loads.

GUILD_CREATE can emit a synchronous channel-navigation burst. The previous eight-item
reliable queue could terminate the session before the UI drained it. Admission now shares
the same 32 MiB estimated byte budget across up to 4,008 events, preserving FIFO order,
per-item limits and failure on real exhaustion. Full-burst/byte-exhaustion/cleanup tests
are synthetic; this does not promise every large account fits existing account budgets.

The owner confirmed the affected server member list loads after restarting the repaired
diagnostic build on September 10, 2026. This does not establish general live compatibility
or long-running capacity behavior.

### Member role display

The active server member pane groups loaded online members by their highest hoisted role,
then shows ungrouped Online and Offline sections. Heading counts cover loaded members, not
the entire server; the existing partial-list hint remains. Highest nonzero role color sets
online names independently of the hoisted role. Offline names remain muted. Unknown roles
fall back to ordinary names/groups. Role changes/removals reuse the live permission mirror,
and member list SYNC/UPDATE supplies role membership. No directory fetch was added.

Role name, position, hoist and primary color are documented fields in
[Discord's role object](https://docs.discord.com/developers/topics/permissions#role-object).
Modern `colors.primary_color` takes precedence over legacy `color`; role gradients are not
rendered. Equal positions favor the lower role ID, consistent with
[discord.py role comparison](https://github.com/Rapptz/discord.py/blob/master/discord/role.py).
Names retain hue when readable; the theme adjusts insufficient contrast, including hover.
Member list subscriptions remain unofficial. Synthetic role evidence does not establish
live role behavior for every account.
