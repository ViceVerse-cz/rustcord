# Unread badges and system notifications

Implemented September 10, 2026; normal-user live interoperability remains unverified.

Incoming DMs appear as avatar shortcuts in the left rail, with red badges. Servers and
channels use unread dots and red mention badges; numbers cap visually at `99+`.
Counts combine service-provided badge counts with bounded newly observed messages. They
are lower bounds after eviction or an ACK without a refreshed count, never a subtraction
of message IDs. Unknown remote read state can still show session-observed activity.

Viewing the focused timeline at its latest known message acknowledges it through the
existing read-state adapter. Historical views, unfocused windows and merely browsing a
server do not acknowledge messages. Failed automatic ACKs get one attempt per latest
message/view; the existing manual Mark read action can retry. A newer service ACK wins
against an older HTTP response. Self messages, history loads and duplicate/replayed
messages do not produce notification storms. Opening a DM shortcut selects that conversation.

Settings → System notifications enables generic OS alerts for the current session.
The default is off; no notification thread or OS call starts before opt-in. Alert text
contains only “Serein” and “You have a new message.” It includes no names, message bodies,
channel identifiers, avatars or attachments. The focused latest conversation suppresses
alerts. Mute, category/channel overrides, all-messages/mentions-only/nothing settings and
DND are checked from available normal-user Gateway preferences. Unknown preferences fail
closed. Role/everyone mentions, blocked relationships and complete protobuf user settings
are not modeled; a protobuf settings update invalidates notification preferences until
another READY snapshot. This is not complete Discord notification-setting parity.

Notification metadata is session-only. Read-state navigation maps are limited by the
4,000-item navigation bound. Observed activity retains at most 4,096 fixed-size records /
128 KiB, and notification candidates at most 32 records / 16 KiB. Preference snapshots
have a 512-KiB bound and at most 4,000 settings/overrides. The OS worker queue holds eight
fixed commands / 128 bytes and at most one generic outstanding notification. Overload
can skip alerts; it does not drop message state. Generation changes invalidate queued
alerts on logout, preference changes and successful read acknowledgements. Outstanding
alerts are dismissed where supported. OS notification history may persist outside the
client; clearing an alert does not guarantee erasure of OS records.

## Platform use and verification

macOS requires the packaged `.app` and OS notification permission. Linux requires a
running desktop notification service. Windows uses Serein's own AppUserModelID; follow
[the per-user shortcut setup](../packaging/windows/README.md) after extracting the package.
No PowerShell application identity is impersonated and no background residency or
autostart is added. Windows/Linux native delivery still requires platform testing.

The default `--demo` and all automated tests never send OS alerts. For an explicit,
synthetic local platform check launch the package with
`--demo --demo-notifications --demo-system-notifications`, enable session notifications
in Settings, then press **Send generic test notification**. This mode never connects to
Discord or opens credential storage. Disabling the toggle tests dismissal. The test
button is only available with this explicit demo flag.

Protocol sources checked September 10, 2026: the original
[discord.py-self read-state implementation](https://github.com/dolfies/discord.py-self/blob/master/discord/read_state.py),
[dispatch and settings reducer](https://github.com/dolfies/discord.py-self/blob/master/discord/state.py),
and [Gateway wire types](https://github.com/dolfies/discord.py-self/blob/master/discord/types/gateway.py).
These are unofficial normal-user implementation evidence, not documented service approval.
OS integration uses [notify-rust 4.18.0](https://docs.rs/notify-rust/4.18.0/notify_rust/)
and its modern macOS UserNotifications backend.

Native macOS validation observed the OS permission prompt, the enabled state and
the generic “Serein / You have a new message.” alert in Notification Center.
Disabling returned the app to off. OS-side dismissal was not conclusively verified
because the notification window was no longer accessible to automation.
The adapter awaits the macOS async show API on its worker because the library's
blocking wrapper can mistake a busy AppKit run loop for an inactive one. Clicking
an OS alert does not yet navigate to a particular conversation; use the DM rail.
