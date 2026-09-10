# Rich presence evidence

Baseline: `5f11cb92644d06e2302678211ac21d9445fad692`, Windows release text-only,
`--demo --demo-profile`, default1120x760 client viewport and Wgpu, dark theme.
`before.png` shows the profile plus member list with custom status only;
`before-dm.png` shows the DM sidebar/header without activity. Captured directly from the
synthetic app window with @oai/sky, inspected, JPEG output losslessly converted to PNG.
No unrelated windows or account content are included.

After native capture is blocked: Computer Use reported owner physical Escape. No further
UI automation was attempted. Do not present the baseline as after evidence. Optional light
baseline was not captured; no after light/narrow/long-text, keyboard or scrolling claim.

Reproduce after when desktop automation is enabled: launch the new text executable with
`--demo --demo-profile`. Robin's member row reads Playing Stardew Valley; the profile
activity section shows Tending the synthetic farm / Spring - Day 12. Click Message, choose
Home then Robin in the DM sidebar to inspect the activity in sidebar, header, People and
profile. Repeat at narrow size and in `--demo-light`, check profile scrolling and Escape.
These are fixtures, never evidence of live Discord presence.
