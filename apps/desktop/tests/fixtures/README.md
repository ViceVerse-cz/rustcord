# Synthetic audio fixture

`audio-tone.mp3` is an original 0.2-second 440 Hz sine wave, generated locally for offline
decoder tests. It contains no user recording or service content. Same MIT OR Apache-2.0
license as Serein. Generation command (FFmpeg is development-only, never bundled):

```sh
ffmpeg -f lavfi -i 'sine=frequency=440:sample_rate=24000:duration=0.2' -ac 1 -c:a libmp3lame -b:a 64k -map_metadata -1 -write_xing 0 audio-tone.mp3
```
