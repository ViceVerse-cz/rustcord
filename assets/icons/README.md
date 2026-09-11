# Interface icons

[Phosphor Icons](https://phosphoricons.com) by Helena Zhang and Tobias Fried, Copyright (c)
2023 Phosphor Icons, licensed under the [MIT License](LICENSE). Source: npm package
`@phosphor-icons/core` version 2.1.1 (repository https://github.com/phosphor-icons/core),
fetched through the jsDelivr npm mirror on September 10, 2026. The unmodified license is
`LICENSE` and is staged in both packages as `licenses/Phosphor-Icons-MIT.txt`.

Seventy-six unmodified `assets/fill/*.svg` and `assets/bold/*.svg` files are scaled to 56×56
pixels, filled white, and rasterized by `resvg` 0.45.1 into one transparent PNG atlas with
64×64 cells (8 columns, 10 rows). `headphones-slash` is derived from `headphones-fill.svg` by
masking a diagonal knockout and adding a 16-unit round-capped stroke, matching the style of
Phosphor's own `*-slash` icons. The application tints glyphs at draw time; no icon font,
JavaScript or per-icon file is bundled.

- `atlas.png`: 512×640 RGBA, 84,996 bytes; decoded 1,310,720 bytes.
  SHA-256 `a7b509057e75a9290bced0a0b4e24de8e8c5b42083817a508c75bc7732fc75d1`.
- `index.tsv`: icon name, tab, zero-based cell; SHA-256
  `77955c8b4dba7fa480c809fb7c37c4f0e83e685bed728995a5c009637fd3c2f8`.
- `LICENSE`: SHA-256 `ddbe6082ec3cf979db47e5af549d2849c5d6182b3e005ef91ce1dbb9eb122f11`.

Every upstream SVG's SHA-256 is pinned in `tools/generate-icons.py`, which refuses to build
from mismatching files. Regenerate from the repository root:

```sh
cargo install resvg --version 0.45.1 --root /tmp/resvg-tool
python3 tools/generate-icons.py --resvg /tmp/resvg-tool/bin/resvg
```

PNG compression bytes can vary with the resvg/png versions; decoded pixels and cell indices
are deterministic. `cargo test -p ui icons` checks that every `Icon` variant maps to a
distinct, non-blank cell and that the atlas stays below 256 KiB.
