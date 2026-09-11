# Inline video attachments (Windows)

MP4 and other attachments identified by video MIME type or a common video extension
show an inline player. Playback starts only after Play. Existing attachment controls
provide pause/resume, seek, volume, replay, download and explicit external opening.
Spoiler consent still applies. Scrolling the card off screen, navigating away,
deleting/changing its attachment, hiding the app or logging out stops playback.
Only one audio or video attachment plays at a time.

The Windows implementation uses the system Media Foundation Source Reader through
the already locked `windows` crate. H.264/AAC MP4 is the synthetic verification target.
Other containers/codecs depend on installed Windows media components; file extensions
do not guarantee support. Clips requiring rotation metadata are explicitly rejected
with a download fallback; already portrait-encoded clips are supported. macOS/Linux show the card and a clear unsupported-playback
message with the existing download/open controls. External video embeds are unchanged.
Native Source Reader provides presentation timestamps and seeking:
[Microsoft Source Reader documentation](https://learn.microsoft.com/en-us/windows/win32/medfound/processing-media-data-with-the-source-reader),
[Windows media formats](https://learn.microsoft.com/en-us/windows/win32/medfound/supported-media-formats-in-media-foundation).

The existing single media worker downloads and decodes; the UI only uploads prepared
frames. The existing CPAL output/resampler and bounded PCM queue supply the playback
clock. Silent video and the tail after audio EOF use a paused-aware monotonic clock.
Seeking drops the previous output queue, seeks the native decoder and discards preroll
before resuming. A replaceable latest-frame slot avoids an accumulating render queue.

## Limits and storage

- Desktop preview: 20 MiB encoded file, ten minutes, up to 1920x1080 or 1080x1920.
- One credential-free download from the existing validated Discord attachment URL;
  no redirects, URL logging, account authorization or decoder-owned network requests.
  The entire bounded clip downloads before playback; no persistent video cache is added.
- The native memory stream copies the encoded file once during initialization; the
  Rust input buffer is then released. No application-created temporary video files.
- Decoded queue: at most eight frames and 32 MiB, plus one preroll frame, one latest
  frame for the UI, transient conversion buffers and one GPU texture.
- Audio: at most one second of stereo float PCM (1.5 MiB at 192 kHz), plus one bounded
  decoded sample. Native codec/framework allocations are additional and not a claimed
  hard whole-process memory cap.
- Highly unusual stream interleaving, unsupported formats and malformed inputs fail
  visibly with a download fallback. No decoder or queue limit is silently disabled.
- Each decoder permits at most 72,000 video frames and 360,000 total samples across
  repeated seeks; Replay creates a new bounded session.
- Media Foundation may block within a native decode call; cancellation invalidates the
  request immediately and the worker releases native resources when that call returns.

No crate versions or bundled codec binaries are added. New Windows feature flags expose
the OS media APIs. Existing dependency notices remain applicable.

## Reproduction

`cargo run --locked -p serein --features demo -- --demo --demo-video` opens the synthetic
card. Play previews a locally generated clip, not Discord content. On Windows,
`cargo run --locked -p serein --features demo -- --demo --demo-video-check` runs an
explicit debug check using the speaker output and generated media, never the microphone.
Native decoder tests use the same fixtures without opening audio devices.

Live Discord downloads and native visual/accessibility behavior require separate
verification. Successful synthetic decoding is not proof of live service interoperability.
