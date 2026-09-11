# Synthetic audio fixture

`audio-tone.mp3` is an original 0.2-second 440 Hz sine wave, generated locally for offline
decoder tests. It contains no user recording or service content. Same MIT OR Apache-2.0
license as Serein. Generation command (FFmpeg is development-only, never bundled):

```sh
ffmpeg -f lavfi -i 'sine=frequency=440:sample_rate=24000:duration=0.2' -ac 1 -c:a libmp3lame -b:a 64k -map_metadata -1 -write_xing 0 audio-tone.mp3
```


`video.mov` is an original three-second 320x180/24fps animated test pattern with
440 Hz mono AAC audio. It uses the same MIT OR Apache-2.0 license as Serein,
contains no account media, and is included only in tests/demo builds. Reproduce:

```sh
ffmpeg -f lavfi -i testsrc2=size=320x180:rate=24 -f lavfi -i sine=frequency=440:sample_rate=48000 -t 3 -c:v libx264 -pix_fmt yuv420p -c:a aac -movflags +faststart video.mov
```

The `video-silent.mov` variant remuxes `video.mov` with `-an -c:v copy`.
`video-short-audio.mov` keeps its three-second video and substitutes a generated
0.5-second 440 Hz AAC tone. Both check that video continues when audio is absent or ends first.
