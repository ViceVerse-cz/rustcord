# Emoji picker names

`names.tsv` contains 3,953 Unicode 17.0 fully qualified emoji and components
that have artwork in the bundled Twemoji atlas. Columns are the original
fully qualified Unicode sequence and its lowercase English name, in Unicode's
recommended CLDR keyboard order. This preserves presentation selectors when
inserting into a draft. The 56 remaining atlas entries are not listed as RGI
emoji/components in the Unicode 17.0 test data; the message renderer still
supports them.

Source: <https://www.unicode.org/Public/17.0.0/emoji/emoji-test.txt>, downloaded
September 10, 2026. Source SHA-256:
`1d8a944f88d7952f7ef7c5167fef3c67995bcae24543949710231b03a201acda`.
Copyright © 2025 Unicode, Inc. Used under the [Unicode License v3](LICENSE-UNICODE).

To reproduce: parse each non-comment source line's codepoints, status and
English name; retain `fully-qualified` and `component` statuses whose sequence
matches `index.tsv` after removing U+FE0F; write the original sequence, a tab,
and the lowercased name. Keep source order. No runtime network lookup or new
runtime dependency is needed.
