# DM and server voice

The optional `voice` build implements native audio calls in existing one-to-one Discord DMs and guild voice channels. It uses the owner's existing account, Discord signaling/voice servers, Opus and DAVE version 1. There is no bot, project relay, separate account, recording service or webview call UI. **Live Discord interoperability and physical microphone/speaker behavior have not been tested; milestone 4 has not passed.**

```sh
cargo run --locked --features voice
cargo run --locked --features voice -- --demo  # offline UI; calling/device access disabled
cargo xtask package-voice                     # separate artifact under dist/voice
```

Default builds remain text-only. Voice source builds additionally require CMake for bundled static libopus; Linux needs ALSA development headers. See [platform requirements](platform-support.md) and the [voice adapter README](../crates/discord-voice/README.md) for dependencies, exact resource limits and protocol tests.

## Implemented behavior and limits

Start calls the selected existing DM; incoming calls require Answer or Decline. One active call is retained while navigating text conversations. Start rings once after Discord voice transport allocation is confirmed; Answer never rings. Required DAVE group readiness and native device readiness precede the connected-audio state. An allocation with no endpoint waits within the deadline; incompatible states fail visibly. Hangup closes local audio immediately and sends departure; another call waits for the service's departure acknowledgment. No uncertain ring write or failed main Gateway session automatically starts another call.

Mute/deafen, session-local input/output selection and focused V push-to-talk are implemented. Push-to-talk releases when focus is lost and is disabled while text entry has focus. It is not a global hotkey. Devices are initialized only following an explicit call and encrypted readiness; no microphone test runs at startup. Headphones are recommended because acoustic echo cancellation is absent. Device loss requires selecting a usable device and calling again; there is no automatic device fallback.

DM calls accept only their expected peer. Server calls support up to 64 total participants, with independent bounded decoder/jitter state and mixed mono playback. Only DAVE version 1 is accepted; encryption downgrades and group identities outside the authenticated participant roster fail closed. Group DMs, Stage channels, recording, video and screen sharing are unsupported. Voice WebSocket resumption has a finite retry budget; failed resumption or main Gateway disconnect requires an explicit new call. Voice credentials, ephemeral DAVE identities and audio stay in bounded session memory. The displayed privacy code applies to the current group epoch; identities are not remembered across calls. Comparing codes does not establish long-term identity verification or text-message encryption.

## Protocol classification

| Area | Evidence / classification | Verification here |
|---|---|---|
| DM entry, incoming call events and ringing | [discord.py-self Gateway](https://github.com/dolfies/discord.py-self/blob/master/discord/gateway.py), [dispatch](https://github.com/dolfies/discord.py-self/blob/master/discord/state.py), [HTTP](https://github.com/dolfies/discord.py-self/blob/master/discord/http.py): unofficial normal-user behavior | Real local WebSocket op13/op4 join/leave and local HTTP ring/decline tests; no Discord call |
| Voice WebSocket, UDP discovery, RTP and codec negotiation | [Discord voice documentation](https://docs.discord.com/developers/topics/voice-connections): documented transport, not an approval of normal-user clients | Synthetic loopback voice event loop, authenticated RTP and Opus tests |
| Required end-to-end encryption | [Discord DAVE protocol](https://daveprotocol.com/): documented; [Davey](https://github.com/Snazzah/davey): unofficial implementation, not an independent security-audit claim | Synthetic two-party MLS/DAVE exchange, tamper/replay rejection and encrypted audio across local sockets |
| Microphone, playback, resampling and devices | CPAL/native platform APIs | Device-free capture/resampling tests only; physical audio and permission dialogs unverified |

The [compatibility matrix](discord-compatibility.md) distinguishes this from restricted OAuth/RPC capabilities. No OAuth voice grant or bot connection substitutes for the user's session.

## Owner-controlled live gate

Run only when the owner explicitly elects to test and controls both sides of a private one-to-one DM. Ordinary tests/CI never access Discord or audio devices. Do not put credentials in chat, command-line arguments, screenshots, fixtures or reports.

1. Build `cargo run --locked --features voice,developer-session`. Complete the [normal-user text gate](authentication.md) with the owner's Serein session and an official Discord client. Prefer Serein's own official-login webview; do not extract another application's credential.
2. With headphones on both sides, open the existing private DM and deliberately select Start. Verify that the official client rings, Answer there, and wait for encrypted audio readiness. Compare the displayed current-epoch privacy codes where available. If login, transport, DAVE or permissions fail, record the redacted failure and stop that attempt; do not bypass it.
3. Speak short test phrases in both directions. Confirm intelligibility, latency and absence of unexpected echo. A connected label, participant list or socket handshake alone is not success. Check mute, deafen, focused V press/release, focus loss and navigation to another text conversation.
4. Hang up and verify both clients leave, microphone access ends and another call can start after departure acknowledgment. Reverse direction: call from the official client, test Decline, then a separate Answer. Confirm there is no automatic answer or retry.
5. Deliberately test selected device changes/loss, network interruption and bounded resume, logout during a call, normal exit and repeated join/leave. Check audio-device/task cleanup and retained memory. An abrupt process exit still needs real service departure and platform-cleanup verification.
6. Record date, OS/hardware/build features, which cases passed, redacted failures and measured resource use. Do not retain voices or private conversation contents. Update progress only for observed behavior; Windows, macOS and Linux require separate physical tests.

Until actual two-way official-client audio and the relevant encryption/teardown cases pass, the release voice gate remains blocked. Offline encrypted transport tests are useful implementation evidence, not that gate.


## Server channel workflow and live gate

Select an existing server voice channel to inspect its roster, then explicitly Join. Browsing alone never opens media devices. Participant rows show names/avatars and separate mute/deafen states; the connected channel shows elapsed local connection time. Mute/deafen, audio settings and Leave remain available while reading other channels. Server-enforced mute/deafen cannot be overridden locally. To switch rooms, leave the current room and join the next after departure is acknowledged. A rejected/full/inaccessible room fails visibly after the bounded allocation deadline.

An authenticated empty room displays “Connected · waiting for others”; audio devices stay closed until another participant joins and DAVE is secured. The client does not transmit unencrypted microphone audio to make an empty room appear connected. A server move, changed voice endpoint/session or main Gateway failure requires an explicit rejoin. The roster is session-only, bounded to 4,096 entries and 1 MiB, and is cleared on fresh login/resync and relevant access invalidation; during a resumable disconnect it is labeled last-known until missed events replay. Missing user details use a fallback identity rather than fetching a whole guild directory.

`cargo run --locked --features voice -- --demo --demo-voice` shows a separately labeled synthetic roster/call scene, including long names and mute/deafen states. It cannot connect, ring, or access devices. The ordinary `--demo` fixture remains the before/after comparison scenario.

For live verification, the owner must explicitly enable `voice,developer-session` and control a private guild voice channel and the participating official clients. In addition to the DM gate above: join empty then add two official-client participants; verify actual intelligible audio in every direction and simultaneous speech; exercise encrypted joins/leaves and the last peer leaving/rejoining; check self mute/deafen, server mute/deafen, denied Connect/Speak, full room, deliberate switching, a server move/disconnect and voice region migration; verify devices/keys/tasks are released on Leave/logout/exit. Never record participants or publish private account/channel data. None of these live outcomes is established by the synthetic roster screenshot.
