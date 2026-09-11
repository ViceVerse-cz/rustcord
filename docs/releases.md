# Manual releases

Actions → **Release (manual)** → **Run workflow**, branch `main`, channel
`nightly` or `production`. There is no schedule or tag/push release trigger.
The selected commit is checked, packaged and released; normal push/PR CI remains separate.

- Semantic-release uses the same Conventional Commit approach as AICaller:
  `fix`/`perf` → patch, `feat` → minor, `!`/`BREAKING CHANGE` → major.
  Documentation and chores alone do not create a release. With no stable tags,
  semantic-release starts at `1.0.0`; the existing Cargo `0.1.0` is not a release tag.
- Nightly: `vX.Y.Z-nightly.RUN.ATTEMPT`, where X.Y.Z is semantic-release's next
  production version. Notes cover changes since the last production release.
  GitHub CLI publishes these calculated prereleases, never marked latest; they do
  not commit versions or consume changes intended for production.
- Production: semantic-release updates and commits `Cargo.toml`, both Cargo lockfiles
  and the Mac bundle version, creates `vX.Y.Z`, and publishes the generated notes
  and packages as latest. No manual version bump is needed.
- Both use `main`; there is no nightly branch. The workflow serializes both channels.
  Planning creates no tags/commits. Every builder receives the exact planned version
  files, and publishing rechecks the plan. If main advances during a build, rerun
  from the new head rather than publishing mismatched artifacts.
- Both build separate text and voice packages on the existing hosted macOS,
  Windows and Ubuntu runners. Asset names include the actual runner OS/architecture.
  Mac ZIPs contain a signed/notarized app; Windows ZIPs and Linux DEBs are unsigned.
  Linux packages target the build runner's distribution, not every Linux system.
  These are host-architecture builds, not universal Mac binaries.
- Every build must pass before publishing. Uploads go to a draft first, then the
  workflow publishes it. An upload/publish failure may leave a draft for inspection.
  Assets include `SHA256SUMS.txt`; reruns never replace a production release.
  A failed publication after tag creation may require repairing that draft manually.
  `GITHUB_TOKEN` needs contents-write access; branch rules must permit the release
  version commit. The workflow does not bypass protections or post issue/PR comments.

Release-only Node dependencies are pinned in `.github/release/package-lock.json`;
CI uses Node 24.19.0 and `npm ci`. They are not application/runtime dependencies.
The small offline check is `node .github/release/check.mjs` (Node 24.19.0,
Python 3.11+; set `PYTHON` if needed). It checks commit bumps/notes, nightly tag
isolation, version updates on temporary copies, and YAML/shell syntax.

## Mac signing setup

Use a **Developer ID Application** certificate for distribution outside the App Store.
Apple Development and Apple Distribution certificates are not substitutes.
Export only that identity and its private key as a password-protected `.p12` from
Keychain Access. Keep the export outside the repository. Configure these repository
Actions secrets (Settings → Secrets and variables → Actions):

| Secret | Value |
| --- | --- |
| `MACOS_CERTIFICATE_BASE64` | Base64-encoded Developer ID Application `.p12`, including its private key |
| `MACOS_CERTIFICATE_PASSWORD` | Password protecting that export |
| `MACOS_SIGNING_IDENTITY` | Full `Developer ID Application: … (…)` identity name |
| `APPLE_ID` | Apple account email authorized for notarization |
| `APPLE_TEAM_ID` | Developer Program team ID |
| `APPLE_APP_SPECIFIC_PASSWORD` | Apple app-specific password for notarization |

For the certificate, `base64 -i /private/path/developer-id.p12 | gh secret set
MACOS_CERTIFICATE_BASE64` uploads directly without printing it. Use interactive
`gh secret set NAME` for the remaining values; never put secrets in commits or chat.
Creating the signing identity and app-specific password requires owner-operated
Apple account setup. An Apple certificate download alone contains no private key.

The signing step uses a temporary keychain, hardened runtime, secure timestamps,
Apple notarization, ticket stapling and signature/Gatekeeper verification. Voice
adds only the microphone entitlement. It deletes the temporary identity and
notarization archive on exit. Missing credentials or failed verification block
the release; there is no unsigned Mac fallback. Signing does not verify Discord
compatibility or grant App Store approval.

Signing setup (September 11, 2026): a Developer ID Application certificate was
issued with owner approval, expires September 12, 2031, and is installed with its
private key in the local login Keychain. Its public certificate was downloaded to
`~/Downloads/developerID_application.cer`. All six repository Actions secrets are set:
`APPLE_ID`, `APPLE_TEAM_ID`, `MACOS_CERTIFICATE_BASE64`,
`MACOS_CERTIFICATE_PASSWORD`, `MACOS_SIGNING_IDENTITY`, and
`APPLE_APP_SPECIFIC_PASSWORD`.
The temporary private-key/export files were deleted after import and upload.
The owner created the app-specific password, which was transferred directly to
GitHub without displaying it in chat. Notarization and a complete signed release
have not been exercised. Release workflows run only when manually dispatched.

References: [GitHub certificate import](https://docs.github.com/en/actions/how-tos/deploy/deploy-to-third-party-platforms/sign-xcode-applications),
[Apple Developer ID certificates](https://developer.apple.com/help/account/certificates/create-developer-id-certificates/),
[Apple notarization](https://developer.apple.com/documentation/security/customizing-the-notarization-workflow).
