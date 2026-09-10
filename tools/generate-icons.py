#!/usr/bin/env python3
"""Rebuild the bundled Phosphor icon atlas (development only).

Downloads the version-pinned SVG sources, verifies every file hash, composes one SVG grid and
rasterizes it with the `resvg` command-line tool. Requires `resvg` 0.45.1 on PATH or `--resvg`:

    cargo install resvg --version 0.45.1 --root /tmp/resvg-tool
    python3 tools/generate-icons.py --resvg /tmp/resvg-tool/bin/resvg
"""

import argparse
import hashlib
from pathlib import Path
import subprocess
import urllib.request

VERSION = "2.1.1"  # @phosphor-icons/core on npm, MIT
BASE = f"https://cdn.jsdelivr.net/npm/@phosphor-icons/core@{VERSION}"
COLUMNS, CELL, PAD = 8, 64, 4
GLYPH = CELL - 2 * PAD

# name, upstream asset, SHA-256 of the unmodified SVG file.
ICONS = [
    ("caret-down", "bold/caret-down-bold.svg", "76a97545e1b923bc13bcc15d7bcbb7f5530105e6eaa98a18c1e30d23e3622843"),
    ("caret-right", "bold/caret-right-bold.svg", "03cadd956d715541432ec8dc2eda1c53ca341af7d3ccecb8dd32c5b9747e290f"),
    ("gear", "fill/gear-six-fill.svg", "dc303bf88571d9aa8d56c30d2630303aa7aba4ca21c21010be3549d6c17666ce"),
    ("microphone", "fill/microphone-fill.svg", "cd8446012357f05a70679ecc792402aa6e49f1fdc820b04035a97b213a2f17a6"),
    ("microphone-slash", "fill/microphone-slash-fill.svg", "6970ccb81d74ffe2165904bc4ed09e0dc738c5d4930192426afd24a5457a2202"),
    ("headphones", "fill/headphones-fill.svg", "35db98ea0b59f4c8c81902d066a8f6db58680f99e48d1a36e6e167e3ef1a6751"),
    ("headphones-slash", "fill/headphones-fill.svg", "35db98ea0b59f4c8c81902d066a8f6db58680f99e48d1a36e6e167e3ef1a6751"),
    ("push-pin", "fill/push-pin-fill.svg", "b3a7a363401c6e09ccd0fc5a7f6813c3183dab69b07eaca3419acf2fafe942b3"),
    ("users", "fill/users-fill.svg", "6104c0634ed5b574e261acc05a3575db8abfb4ccbcb3bf12f5359a431303783b"),
    ("user-plus", "fill/user-plus-fill.svg", "07fe0181040b8dd7956f8067f13f83d81ec395d54b1760392a5daf5692232652"),
    ("user-circle", "fill/user-circle-fill.svg", "4e16cdba116195993e6398bb097e7c4b354fb129fafdf2b56fd4b7c069f440d3"),
    ("magnifying-glass", "bold/magnifying-glass-bold.svg", "b73e393b20bff0aaee96b9e325d0276fe1ab5fc81b5080633a95827bd14ebae6"),
    ("plus", "bold/plus-bold.svg", "3d20a4b2e00657baeb922bed94f13fbfa288968b738991d047dd252cb64005d8"),
    ("plus-circle", "fill/plus-circle-fill.svg", "7e6b31bae705c568cb7ce2b42fca9d65f85da725bac30f207ee2ce6368abb6c5"),
    ("x", "bold/x-bold.svg", "d540487912a267d83c495954b24ca07981002fda05ee2ea0b492d8fc188d1c3e"),
    ("smiley", "fill/smiley-fill.svg", "2f993043ef4566931f5ad81be6d1030734d2dbad5e5d7c5d380debfeaeeb9399"),
    ("bell", "fill/bell-fill.svg", "26437565edc7418a2c326a74078fb4519aa2316e84fcf1adefbdd1b2c89f4ac5"),
    ("phone", "fill/phone-fill.svg", "4546c9d7d0a26f08ec5a35f2ca3e12d8ef76767ba60c2effa6f214d7f070dcb6"),
    ("phone-call", "fill/phone-call-fill.svg", "6d67e611de99d8a33ba45f5d2a9e56d4123dcb9395020998a3b7250d3a08e36a"),
    ("phone-disconnect", "fill/phone-disconnect-fill.svg", "15996d65a74424921e84c8e40e94fb43da650183476abb00b3d1498d2400ffc4"),
    ("video-camera", "fill/video-camera-fill.svg", "daf72fb4e3f7978254c2ac1959db6d855f12bb0d9b7504309d2e6270b7192933"),
    ("video-camera-slash", "fill/video-camera-slash-fill.svg", "1cb4bf1be6de46f024be49c324641afe36cfa5807f4846691851d8c954a5d0e4"),
    ("monitor-arrow-up", "fill/monitor-arrow-up-fill.svg", "9519c0c4c734553167b598b03d493cdb6ef14f8df8c7283a1847699bcc33a0a8"),
    ("rocket-launch", "fill/rocket-launch-fill.svg", "fed02fcf57da975870e6898085b8e469a695dafac38ddae2d92b169678c661a9"),
    ("waveform", "fill/waveform-fill.svg", "fbbca82dbcc988184e314a671ac08517b4099540ab97605ae186998d9c0d3187"),
    ("arrow-bend-up-left", "bold/arrow-bend-up-left-bold.svg", "b59c6a1dea610f669a69920bece2138108a54d88eaa9a1d0a20f82e1657c9f25"),
    ("pencil-simple", "fill/pencil-simple-fill.svg", "b78e7b71cb19d43e235983a883d50ab78cf9227f9864106e1aa4f366ee317aa8"),
    ("dots-three", "bold/dots-three-bold.svg", "5bd06c9754d075b409e3259350c9abc9ec106ffb3b6f3cd451f6419271163bc1"),
    ("tray", "fill/tray-fill.svg", "8e0ed750696c01ef6e418436275501e4467c66b6f88688e1d0ac86e84a296b47"),
    ("question", "fill/question-fill.svg", "91390e2c8d076dedcf73ec4ffc5ac6531dbf93aadac7e0907ba1cfb9e95a145b"),
    ("arrow-clockwise", "bold/arrow-clockwise-bold.svg", "773d814cdf564a4d0db6e0408a60e5bd97c76c90293aaf383c72399ce2e034a1"),
    ("chats", "fill/chats-fill.svg", "c88539cf3c42ef3a42f90d10e9c4736cbad59e2639948dbe647227e56160447b"),
    ("speaker-high", "fill/speaker-high-fill.svg", "c266c407915382db3866570f816934b804c1b84bd47d971fad0e95ac5107dd6b"),
    ("hash", "bold/hash-bold.svg", "8d2cb5e39903004da3d0a85a919adc6c76adcd65b3c9c1e8e355d8cf6cdf7193"),
    ("chat-centered-text", "fill/chat-centered-text-fill.svg", "cc2c02e50a62aa98aae8634d518b2cc311413765ddfddae5908bf9168113d0a7"),
    ("paper-plane-right", "fill/paper-plane-right-fill.svg", "a8e6f3a92755f1bc79bcfd691e794709bec6b13d68b948adc1eb035a2ddd8fff"),
    ("arrow-square-out", "bold/arrow-square-out-bold.svg", "68348bacc7f539b8d73de92d7c120f22e424b4364374ccab01719ada8c8a12aa"),
]
LICENSE_SHA256 = "ddbe6082ec3cf979db47e5af549d2849c5d6182b3e005ef91ce1dbb9eb122f11"


def fetch(path):
    with urllib.request.urlopen(f"{BASE}/{path}", timeout=60) as response:
        return response.read(1024 * 1024)


def inner_svg(svg):
    start = svg.index(">", svg.index("<svg")) + 1
    end = svg.rindex("</svg>")
    return svg[start:end]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--resvg", default="resvg", help="path to the resvg executable")
    parser.add_argument("--print-hashes", action="store_true", help="print SHA-256 values instead of checking")
    args = parser.parse_args()
    destination = Path(__file__).resolve().parents[1] / "assets" / "icons"
    destination.mkdir(parents=True, exist_ok=True)

    license_text = fetch("LICENSE")
    if args.print_hashes:
        print("LICENSE", hashlib.sha256(license_text).hexdigest())
    elif hashlib.sha256(license_text).hexdigest() != LICENSE_SHA256:
        raise ValueError("LICENSE SHA-256 mismatch")

    names = [name for name, _, _ in ICONS]
    assert len(set(names)) == len(names)
    rows = (len(ICONS) + COLUMNS - 1) // COLUMNS
    width, height = COLUMNS * CELL, rows * CELL
    parts = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">']
    parts.append(
        '<defs><mask id="slash" maskUnits="userSpaceOnUse" x="0" y="0" width="256" height="256">'
        '<rect width="256" height="256" fill="#fff"/>'
        '<path d="M40 24 L232 216" stroke="#000" stroke-width="44" stroke-linecap="round"/></mask></defs>'
    )
    index = []
    for cell, (name, asset, sha256) in enumerate(ICONS):
        svg = fetch(f"assets/{asset}")
        digest = hashlib.sha256(svg).hexdigest()
        if args.print_hashes:
            print(name, asset, digest)
        elif digest != sha256:
            raise ValueError(f"{asset} SHA-256 mismatch: {digest}")
        text = svg.decode("utf-8")
        assert 'viewBox="0 0 256 256"' in text, asset
        body = inner_svg(text)
        x = (cell % COLUMNS) * CELL + PAD
        y = (cell // COLUMNS) * CELL + PAD
        scale = GLYPH / 256
        if name.endswith("-slash") and "slash" not in asset:
            # Phosphor has no slashed headphones; compose the upstream glyph with a knocked-out
            # diagonal in the style of its own `*-slash` icons.
            body = (
                f'<g mask="url(#slash)">{body}</g>'
                '<path d="M40 24 L232 216" stroke="#fff" stroke-width="16" stroke-linecap="round"/>'
            )
        parts.append(f'<g transform="translate({x} {y}) scale({scale:.6f})" fill="#fff">{body}</g>')
        index.append((name, cell))
    parts.append("</svg>")
    if args.print_hashes:
        return
    atlas_svg = destination / "atlas.svg"
    atlas_svg.write_text("".join(parts), encoding="utf-8")
    subprocess.run([args.resvg, str(atlas_svg), str(destination / "atlas.png")], check=True)
    atlas_svg.unlink()
    (destination / "index.tsv").write_text("".join(f"{name}\t{cell}\n" for name, cell in index), encoding="utf-8")
    (destination / "LICENSE").write_bytes(license_text)
    print(f"{len(ICONS)} icons; {width}x{height}; atlas {(destination / 'atlas.png').stat().st_size} bytes")
    for file in ["atlas.png", "index.tsv", "LICENSE"]:
        print(file, hashlib.sha256((destination / file).read_bytes()).hexdigest())


if __name__ == "__main__":
    main()
