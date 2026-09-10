# User mentions

Implemented September 10, 2026. Type `@` in the native composer to search names/IDs already present in the selected conversation: recipients, the loaded member pane, and the retained timeline. The candidate set is capped at 256 users and the menu at eight suggestions. Arrow keys select, Tab/Enter inserts the exact `<@id>` token without sending, and Escape dismisses. IME composition never invokes mention shortcuts. This makes no directory-search request and does not scrape members; people not loaded locally cannot be discovered through autocomplete.

Incoming `<@id>` and legacy `<@!id>` render as clickable native profile links. The message's bounded mention metadata supplies display names, including users absent from the people pane; unknown IDs display `@id` until their profile is explicitly opened. Code, escaped syntax, HTML entities and Markdown-link labels stay literal. Descriptions and values in embeds share the same renderer. Each formatted part limits interactive mentions to 100. Original message/draft text is retained separately; no HTML or scripts execute.

Send/edit requests explicitly allow only the user IDs actually present in the submitted mention syntax, deduplicated and capped at 100. `@everyone`, `@here`, role mentions and automatic reply notifications remain disabled. Discord remains authoritative for notification/permission behavior; production normal-user acceptance has not been validated.

Message mention metadata is bounded to 100 user summaries, included in RAM/event budgets, reconciled on partial/null message updates, and cached in SQLite schema 6. Existing histories gain an empty-default column without losing drafts. Cached mention JSON is bounded to 128 KiB per message and included in the existing global disk budget. No extra dependency or remote account action is introduced.

Sources checked September 10, 2026: Discord's [message formatting](https://docs.discord.com/developers/reference#message-formatting) and [message/allowed-mention structures](https://docs.discord.com/developers/resources/message#allowed-mentions-object). These are public developer wire references; this client's normal-user protocol use remains unofficial. Offline tests cover precision/bounds, code/escape safety, keyboard profile activation, Unicode cursor insertion, Enter not sending during selection, partial-update/history ordering, and SQLite migration/round-trip/rejection. Live notification delivery and native accessibility remain unverified.


Composer update (September 10, 2026): known mention tokens display as `@name`, Unicode
emoji use the bundled Twemoji atlas, and custom emoji show static server artwork (or
`:name:` while unavailable). The native editor keeps the original wire characters for
copy, sending, draft storage and undo. Arrow navigation and deletion treat each rendered
mention/emoji as one token; unknown mentions remain editable literal markup. Message
editing uses this same composer, including mention suggestions and both emoji pickers.
The ordinary unsent draft remains separate while editing; Save edit and Cancel edit
return to it. Network failure keeps the edit available for review/retry.
