# Screen-sharing preview evidence

`before.png` is the unmodified `609f8bf` voice release package with `--demo --demo-voice`; its Share button is disabled. `after.png` is the changed voice release package at feature commit `fb3736d` (before integrating unrelated main UI changes from `afac0ee`) with the same synthetic fixture and the source/settings dialog open. Both are native application-window exports at 1120×760; no real user content, screen source enumeration or media capture was used. The popup's sources are explicitly synthetic and Share remains disabled in the demo.

Normal dialog layout and 1080p/60 fps selection were inspected. Automation-driven resizing/key delivery did not reliably repaint the preview, so full native narrow/keyboard validation is not claimed. Screenshots do not verify Discord interoperability or native capture permission behavior.
