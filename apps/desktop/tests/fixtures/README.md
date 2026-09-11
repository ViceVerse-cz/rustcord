# Synthetic audio fixture

`audio-tone.mp3` is an original 0.2-second 440 Hz sine wave, generated locally for offline
decoder tests. It contains no user recording or service content. Same MIT OR Apache-2.0
license as Serein. Generation command (FFmpeg is development-only, never bundled):

```sh
ffmpeg -f lavfi -i 'sine=frequency=440:sample_rate=24000:duration=0.2' -ac 1 -c:a libmp3lame -b:a 64k -map_metadata -1 -write_xing 0 audio-tone.mp3
```

## Synthetic video fixtures

The MP4 fixtures contain three seconds of locally generated 320x180 testsrc2 motion at
24 fps, encoded as H.264 with two B-frames. video-bars.mp4 includes quiet 48 kHz mono
AAC; video-silent.mp4 has no audio; video-short-audio.mp4 ends its audio after one second.
These are original MIT OR Apache-2.0 test assets, with no user or Discord content.
FFmpeg is development-only and is not bundled with the application.

`video-rotated.mp4` adds rotation side data to the silent clip to verify the explicit
unsupported-orientation fallback. Generate with FFmpeg 7.1 from this directory:

```sh
ffmpeg -f lavfi -i testsrc2=size=320x180:rate=24:duration=3 -f lavfi -i sine=frequency=440:sample_rate=48000:duration=3 -vf drawbox=x=0:y=0:w=iw:h=ih:color=black@0.08:t=fill -c:v libx264 -pix_fmt yuv420p -bf 2 -g 24 -crf 30 -c:a aac -b:a 48k -af volume=0.02 -movflags +faststart -map_metadata -1 -shortest video-bars.mp4
ffmpeg -i video-bars.mp4 -an -c:v copy video-silent.mp4
ffmpeg -i video-bars.mp4 -af atrim=duration=1 -c:v copy -c:a aac -b:a 64k video-short-audio.mp4
ffmpeg -display_rotation:v:0 90 -i video-silent.mp4 -c copy video-rotated.mp4
```
