# Build this personal OpenPakon on an Intel Mac

This copy is OpenPakon 1.1.0 plus the sensor-edge trim. The app you want is
an unsigned `Pakon.app` that runs on an Intel Mac (x86_64), including
macOS 26. The upstream script `packaging/macos/build.sh` refuses to run
unless `uname -m` is `arm64`, and it builds a universal binary with `lipo`.
Do not use it on an Intel Mac. Use the script below, which compiles the
native x86_64 binary and assembles the same kind of bundle.

The result is ad-hoc signed so it runs on the Mac that built it. It is not
notarized. It does not include Sparkle, so it will not replace itself with
an upstream release. Scans and settings still go to the usual Pakon folders
(`~/pakon`, `~/Library/Application Support/Pakon`).

The Intel build was confirmed on the user's Mac. Two things that build
turned up are in the prerequisites and in `build-intel.sh`: QEMU's
configure needs `pkg-config` on `PATH`, and current Cargo writes the
Unicorn build under `build/unicorn-engine-sys/<hash>/out`.

## 1. Prerequisites

macOS 13 or later. Xcode's command-line tools, CMake, pkg-config, Rust,
Node.js 22.12 or newer (20.19 or newer also works), and Pillow for the icon.

QEMU's configure, run while `unicorn-engine-sys` builds, requires a
`pkg-config` binary on `PATH`. Xcode's command-line tools do not provide
one. `brew install pkg-config` is the normal fix. Any `pkg-config` is
enough: a stub that prints a version for `--version` and exits 1 otherwise
was enough for that configure check.

```sh
xcode-select --install
brew install cmake pkg-config node@22
# If brew's node is keg-only, put it on PATH, for example:
# export PATH="$(brew --prefix node@22)/bin:$PATH"
node --version    # v22.12 or newer, or v20.19 or newer

curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustup default stable
rustup update stable
rustc --version    # 1.85 or newer. Cargo.lock uses Rust 2024 crates.
rustup target add x86_64-apple-darwin

python3 -m pip install --user pillow
```

`xcode-select --install` provides `clang`, the macOS SDK, `libclang` (the
Unicorn crate needs it), `iconutil`, `codesign`, and `hdiutil`. Confirm the
machine is the Intel one:

```sh
uname -m    # x86_64
```

## 2. Build

From the root of this tree:

```sh
packaging/macos/build-intel.sh
```

That one command does the rest. The first build compiles Unicorn from
source; that is the long step. Later builds reuse it.

What the script runs, in order:

```sh
export MACOSX_DEPLOYMENT_TARGET=13.0
export SDKROOT="$(xcrun --sdk macosx --show-sdk-path)"

# page, into web/static
packaging/build-web.sh

# native Intel binary
cd rust
cargo build --release --locked -p pakon-app --target x86_64-apple-darwin
```

It then copies that binary and `web/static`, `resources/`, `firmware/`,
`profiles/`, `oem/`, `LICENSE`, and `NOTICE` into a bundle, writes
`Info.plist` with version `1.1.0-trim` (so the in-app update check does not
treat an upstream release as a newer version of this build), and ad-hoc
signs the app with `packaging/macos/entitlements.plist`. The entitlements
allow the JIT Unicorn uses to run Kodak's colour code. Without them a
hardened-runtime signature would stop scans from rendering.

## 3. Where the app is

```text
dist/macos/Pakon.app
dist/macos/Pakon.dmg
```

The disk image is a convenience copy of the same app plus an Applications
shortcut. Install by copying the app:

```sh
cp -R dist/macos/Pakon.app /Applications/
open /Applications/Pakon.app
```

Or open `dist/macos/Pakon.app` directly. If macOS says it cannot verify the
developer, use the app's context menu and choose Open, then Open again.
That prompt is the unsigned local build. A copy you made yourself is not
quarantined the way a browser download is.

Confirm the binary is Intel:

```sh
file dist/macos/Pakon.app/Contents/MacOS/Pakon
# Mach-O 64-bit executable x86_64
```

## 4. Run from the checkout instead of a bundle

This skips the `.app` and opens a window straight from the source. The page
must already be built (`packaging/build-web.sh`).

```sh
cd rust
export MACOSX_DEPLOYMENT_TARGET=13.0
cargo run --release --locked -p pakon-app
```

There is no version in that window, and it does not check for upstream
updates. Settings and scans still use the Pakon folders above.

## 5. The trim after it is built

Open **Frames** or **Settings**. **Sensor trim** starts at 0. For the white
line at Base 8, set **Top** to `10` px (or about `0.7` %) and leave the
other edges at 0. The boxes on the strip move in by that amount. Adjust and
Export, including the contact sheet, use it. The value is kept in
`~/Library/Application Support/Pakon/settings.json` under `edge_trim`.
