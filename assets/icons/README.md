# Interface icons

[Phosphor Icons](https://phosphoricons.com) by Helena Zhang and Tobias Fried, Copyright (c)
2023 Phosphor Icons, licensed under the [MIT License](LICENSE). Source: npm package
`@phosphor-icons/core` version 2.1.1 (repository https://github.com/phosphor-icons/core),
fetched through the jsDelivr npm mirror on September 10, 2026. The unmodified license is
`LICENSE` and is staged in both packages as `licenses/Phosphor-Icons-MIT.txt`.

Sixty-eight unmodified `assets/fill/*.svg` and `assets/bold/*.svg` files are scaled to 56×56
pixels, filled white, and rasterized by `resvg` 0.45.1 into one transparent PNG atlas with
64×64 cells (8 columns, 9 rows). `headphones-slash` is derived from `headphones-fill.svg` by
masking a diagonal knockout and adding a 16-unit round-capped stroke, matching the style of
Phosphor's own `*-slash` icons. The application tints glyphs at draw time; no icon font,
JavaScript or per-icon file is bundled.

- `atlas.png`: 512×576 RGBA, 77,000 bytes; decoded 1,179,648 bytes.
  SHA-256 `56b0b6b937208f1360eee2c408c104f71803c3fb3cd42f84e060f87423bf8c5b`.
- `index.tsv`: icon name, tab, zero-based cell; SHA-256
  `afd69ab8243cff5cf690e2517ca3da432f86fe8572bc5665bfd4212267c1ecda`.
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
