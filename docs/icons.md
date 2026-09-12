# Application icon

Desktop application artwork comes from the owner-supplied `Serein-icon-pack-v2`.
Windows embeds the multi-resolution ICO in the executable using the Windows SDK
resource compiler (or windres for GNU builds); the window and tray use its PNG.
Linux installs the supplied hicolor PNG/SVG icons with the Debian package and
references `serein` in the desktop launcher. Wayland uses `org.serein.desktop`
as the application ID, matching the installed `org.serein.desktop.desktop` launcher.

macOS packaging runs `packaging/macos/compile-icon.sh` with Xcode's `actool` to
compile `Serein.icon` into `Assets.car`. The native icon has default, dark, and
system monochrome appearances; dark uses Apple’s native dark background.
macOS controls appearance selection. The supplied ICNS remains the fallback
for older systems. Bare `cargo run` has no app bundle and cannot demonstrate
the native Dock/Finder appearance variants; use the packaged app for that.
The icon compilation requires full Xcode with Icon Composer support.

# Server icons and service images

Implemented September 10, 2026. Server names/icons load from the existing account's navigation snapshot. `GUILD_UPDATE` applies name/icon patches without dropping omitted fields; explicit null removes the icon, invalid hashes fall back to initials, and a new hash requests a new image key. Unknown-guild and old-session patches cannot create navigation entries. Both full names and selection are exposed to accessibility.

The documented flat guild shape is accepted. The normal-user `READY.guilds[].properties` variant is also accepted for name/icon metadata, with nested fields overriding flat fields only when present. Public implementation evidence is `parse_ready_supplemental` in [discord.py-self state.py](https://github.com/dolfies/discord.py-self/blob/master/discord/state.py#L1751-L1752), which merges that object into the guild. Its [guild update handler](https://github.com/dolfies/discord.py-self/blob/master/discord/state.py#L2943-L2950) processes ordinary guild objects. This is unofficial protocol evidence, not an approved normal-user API or a live acceptance test. New guild admission and complete guild availability reconciliation remain outside this change.

[Discord's image reference](https://docs.discord.com/developers/reference#image-formatting) documents the guild icon CDN path and static PNG format. Icon downloads are constructed as `https://cdn.discordapp.com/icons/{id}/{validated_hash}.png?size=128`. Animated hashes deliberately select a static image.

Visible embed images reuse the same credential-free worker and account cache as profile pictures and server icons. Only HTTPS image paths on `cdn.discordapp.com`, `media.discordapp.net`, and the two named Discord external-image proxies are accepted. Requests never carry account authorization/cookies, use system HTTP proxies, follow redirects, or fetch the original external host. [Discord's message reference](https://docs.discord.com/developers/resources/message#embed-object) documents media `proxy_url`; PNG conversion and bounded size query parameters are unofficial behavior. [A public converter implementation](https://github.com/TMAFE/discord-webp-converter) documents `format=png`. Rejected or unsupported images remain visible placeholders with the embed's explicit open action; no automatic video/audio/animation decoder is added.

The shared disk directory retains its existing name: `serein/avatars/{account_id}` under the platform application-data directory. It has a 1 GiB / 4,096-file / 90-day retention limit, atomic replacement, account separation, and the existing clear-cache/logout cleanup. Embed filenames are SHA-256 digests of the exact source key, including query parameters; signed URLs do not appear in filenames or diagnostics. Old icon/image files age out under these limits.

The worker admits 128 request keys, each at most 2,054 bytes, has two decoded-result slots, and decodes one image at a time. Downloads stop at 2 MiB; avatars/icons additionally reject encoded images over 512 KiB. PNG input dimensions are capped at 256 per axis for icons/avatars and 1,024 for embeds; decoder allocation limits are 1 MiB and 8 MiB respectively. Output is at most 128 square or 512 square. The shared texture LRU retains at most 64 entries and 16 MiB of RGBA texture payload, releasing handles on eviction. These component ceilings exclude allocator, decoder intermediates, HTTP/TLS, renderer and GPU-driver overhead; they are not process-memory measurements.

Offline checks cover nested/flat metadata, absent/null/changed/hostile icon patches, stale-session rejection, URL restrictions, redirects, body/dimension limits, cache reopen/isolation/eviction/cancellation, texture eviction, and request-free synthetic icons. The native demo's three-stripe icon is generated from original pixels; its embed landscape is generated from native drawing primitives. Real account image delivery and Windows/Linux presentation remain unverified.

Activity cards reuse this loader for static application assets and Discord media-proxy images.
An activity with only an application ID performs one bounded, credential-free public application
metadata lookup before loading its Discord CDN icon. Metadata is never cached; the PNG shares
normal cache/clear/logout limits. The card reserves a 64px slot during loading and retains the
text if artwork is missing, unsupported, or fails. Source and compatibility notes are in
[discord-compatibility.md](discord-compatibility.md#received-rich-presence-september-10-2026).
