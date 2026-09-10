# Serein interface direction

A quiet native home for existing Discord conversations. Graphite surfaces, a restrained teal accent, and warm off-white in light mode. The conversation and the next action have priority over diagnostics.

- Shared palette/type/spacing live in `crates/ui/src/design.rs`; installed for both egui themes and restored after logout resets egui memory. Body 15px, small 12px, headings 22px; clear accent focus borders and 32px normal interaction height.
- Sign-in: clear main action and owner acknowledgement, optional interface preview explicitly labeled synthetic, secondary compatibility/storage details. Two columns on wide windows; one column below 900px available width, scrollable on short screens.
- Messaging: 64px guild rail, resizable channel sidebar, selected-channel accent, compact conversation header, account/settings footer, and framed multiline composer. Appearance, cache clear and logout are grouped in Settings. Offline/experimental state stays visible.
- Rows: circular profile pictures with initials while loading/unavailable, author/body hierarchy, whitespace instead of row dividers, one accessible message menu. Existing virtualization, anchors, spoiler consent, safe links and pending-delivery behavior are retained.

An optional People pane shows the active conversation’s members or DM participants, with presence or custom status under each name. Narrow windows use a compact People window. Clicking a user opens a 300-point profile popout beside the click (banner or accent strip, overlapping avatar with presence dot, custom status bubble, display name with server tag, username and pronouns, badge artwork, bio/membership/connections/mutual servers panel and a Message action); it uses the account's two theme colors as a gradient when set, otherwise the shared palette, and closes on Escape or an outside click. Member data is bounded and never represented as a complete guild directory when only a partial window is known.

Native macOS previews were inspected at about 1088px and 786px widths in dark/light modes. Settings/theme changes and a visible message menu/Reply action were exercised with synthetic data. Sign-in was viewed in isolated offline mode with authentication disabled; the real login window was left undisturbed. The attempted short-height resize did not change the window, so minimum-height visual behavior remains unverified. Actual screen-reader/IME and Windows/Linux visual checks remain open.

Measured palette contrasts: muted text/canvas 7.22:1 dark, 5.48:1 light; primary-button text/fill 8.19:1 dark, 6.45:1 light. These are solid-color calculations, not certification of every state or rendered pixel.
