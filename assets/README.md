No Discord logos, proprietary fonts or copied client assets are bundled. The interface leads proportional text with three unmodified SIL Open Font License 1.1 Inter faces (Regular, Medium, SemiBold; the heavier two form the `medium`/`semibold` families because egui has no synthetic bold), keeps egui's default faces as fallbacks, and appends two unmodified OFL fallback faces (CJK, Arabic) to every family. They are embedded once in the executable; there is no runtime font download, system-font scan, or render-time filesystem access.

| Asset | Upstream version | Bytes | Copyright and license |
|---|---|---:|---|
| `fonts/NotoSansCJKjp-Regular.otf` | 2.004 | 16,467,736 | © 2014–2021 Adobe; [OFL 1.1](fonts/NotoSansCJK-LICENSE.txt) |
| `fonts/NotoSansArabic.ttf` | 2.012 | 844,676 | Copyright 2022 The Noto Project Authors; [OFL 1.1](fonts/NotoSansArabic-OFL.txt) |
| `fonts/Inter-Regular.otf` | 3.19 | 258,992 | Copyright (c) 2016-2020 The Inter Project Authors; [OFL 1.1](fonts/Inter-OFL.txt) |
| `fonts/Inter-Medium.otf` | 3.19 | 269,692 | Copyright (c) 2016-2020 The Inter Project Authors; [OFL 1.1](fonts/Inter-OFL.txt) |
| `fonts/Inter-SemiBold.otf` | 3.19 | 270,760 | Copyright (c) 2016-2020 The Inter Project Authors; [OFL 1.1](fonts/Inter-OFL.txt) |

Downloaded September 10, 2026 from pinned upstream sources:

- [Noto CJK font](https://github.com/notofonts/noto-cjk/blob/f8d157532fbfaeda587e826d4cd5b21a49186f7c/Sans/OTF/Japanese/NotoSansCJKjp-Regular.otf), [license](https://github.com/notofonts/noto-cjk/blob/f8d157532fbfaeda587e826d4cd5b21a49186f7c/Sans/LICENSE). SHA-256: `68a3fc98800b2a27b371f2fb79991daf3633bd89309d4ffaa6946fd587f375b5`.
- [Noto Sans Arabic font](https://github.com/google/fonts/blob/334b789e33413f3aba4264d9aa6c97f7b94c5a2f/ofl/notosansarabic/NotoSansArabic%5Bwdth%2Cwght%5D.ttf), [license](https://github.com/google/fonts/blob/334b789e33413f3aba4264d9aa6c97f7b94c5a2f/ofl/notosansarabic/OFL.txt). The upstream variable-font filename is shortened locally; font bytes are unchanged. SHA-256: `63111b5b2e074dd48cc67692e0a2726d86ee94c1c37fe8598257b7b4e87e869e`.

- [Inter 3.19](https://github.com/rsms/inter/tree/v3.19/docs/font-files) static OTF instances `Inter-Regular.otf`, `Inter-Medium.otf`, `Inter-SemiBold.otf` and [license](https://github.com/rsms/inter/blob/v3.19/LICENSE.txt), fetched through the jsDelivr GitHub mirror of that tag on September 10, 2026. SHA-256: Regular `a7e791e8f5a0fb02b65663f7fca73e1d1ca9543f772ad480cbd76f4e3fe3f8cc`, Medium `99dab2bdcb613c4c8264000a94351d1227f74dc95a86d1249493aeee0c0179c4`, SemiBold `8c1990b6012254ea2b487161697d107357dd0ee55811cfd91c8c11227bbef457`.

The five font blobs total **18,111,856 bytes (17.27 MiB)**, below the 18 MiB asset ceiling asserted by `cargo test -p ui bundled_fallbacks`. This raw size is separate from compressed distribution size, font-parser/layout memory and GPU glyph-atlas allocations. Package the license files and third-party copyright notices with the executable.

The focused test checks egui glyph availability for synthetic Japanese kana/kanji, simplified/traditional Chinese, Korean, Arabic, Latin and combining accents in both font families. It does not prove complete Unicode coverage, correct Arabic shaping/bidirectional editing, actual IME behavior, screen-reader behavior, or platform rendering. The single CJK face uses Japanese regional Han forms; locale-specific forms and extended emoji remain incomplete. Fallback glyphs in code blocks are not guaranteed to have the primary monospace font's cell width.

Test detail: egui 0.36.2's [`Font::has_glyph`](https://github.com/emilk/egui/blob/49682f8baa058bf49e011035cfbd6e825f88a5ef/crates/epaint/src/text/font.rs#L663) compares the selected face with the replacement face. Our initial test observed a false negative for `H`; inspecting that implementation explains why valid glyphs on the replacement face fail this query. The test therefore uses egui's already-resolved Skrifa parser to check every sample scalar against the actual configured face charmaps, then separately checks egui's installed CJK/Arabic fallback path. It does not skip missing sample glyphs.
