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

These steps were run and verified on 21 Nov 2026 on the owner's Intel
MacBook (i5-1038NG7, macOS 26.7.2). The result was installed as
`/Applications/Pakon Trim.app` and works.

### Why this app is smaller than the official one

The official `Pakon.app` is about 64 MB and this build is about 38 MB, but
nothing is missing. The official binary is universal: it carries both an
Intel and an Apple silicon copy of the program in one file (about 51 MB).
This build contains only the Intel copy. The official app also bundles the
Sparkle auto-updater framework (about 2.6 MB), which this build leaves out
on purpose. Resources, firmware, profiles, and OEM files are the same.

## 1. Prerequisites

### What is actually installed on the owner's Mac (no Homebrew, no sudo)

Homebrew needs an admin password, so everything was installed inside the
home folder instead. Before building, load it all with:

```sh
source "$HOME/.cargo/env"     # rustc 1.100.0 + x86_64-apple-darwin target
export PATH="$HOME/.local/bin:$HOME/.local/node/bin:$(python3 -m site --user-base)/bin:$PATH"
# ~/.local/node      Node.js v22 (official tarball)
# user-base bin      cmake 4.x and pillow, from `pip3 install --user cmake pillow`
# ~/.local/bin       pkg-config stub (see below)
```

Xcode command-line tools were already installed.

**pkg-config is required.** QEMU's `configure` inside the
`unicorn-engine-sys` crate aborts with `pkg-config binary 'pkg-config' not
found` if there is none on `PATH`. A symptom is an empty `config-host.h` and
then `typedef redefinition ... Int128 vs __int128` errors. Unicorn never
actually queries any package, so this stub is enough:

```sh
mkdir -p ~/.local/bin
printf '#!/bin/sh\ncase "$1" in --version) echo 0.29.2;; *) exit 1;; esac\n' > ~/.local/bin/pkg-config
chmod +x ~/.local/bin/pkg-config
```

After a failed Unicorn build, delete only its cache before retrying:
`rm -rf rust/target/x86_64-apple-darwin/release/build/unicorn-engine-sys`.

### Generic instructions (machine with Homebrew)

macOS 13 or later. Xcode's command-line tools, CMake, Rust, Node.js 22.12
or newer (20.19 or newer also works), and Pillow for the icon.

```sh
xcode-select --install
brew install cmake node@22
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
# On the owner's Mac it is installed under a separate name:
ditto dist/macos/Pakon.app "/Applications/Pakon Trim.app"
xattr -dr com.apple.quarantine "/Applications/Pakon Trim.app"
open "/Applications/Pakon Trim.app"
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

## 6. What this fork changes (vs upstream jorshhh/open-pakon v1.1.0)

Branch `sensor-trim` on https://github.com/bruhuhuhu/open-pakon, two
commits on top of upstream v1.1.0 (`eb8003c`) plus this notes update:

1. **Sensor-edge trim.** The owner's Pakon has a fixed ~10 px white band
   along one long edge of the sensor (hardware alignment; confirmed by
   flipping the film, the band stays on the same scanner side). OpenPakon
   rotates frames upright, so the band used to land on different sides.
   The trim is applied in sensor coordinates, *before* rotation.
   - `rust/crates/pakon-roll/src/trim.rs` (new): per-edge trim (Top,
     Bottom, Leading, Trailing; px or %) and optional auto blank-edge
     detection (max 2 % per side).
   - `rust/crates/pakon-roll/src/lib.rs`, `pakon-web/src/{files,jobs,lib}.rs`:
     applied to JPEG, TIFF, raw TIFF export and the contact sheet; stored in
     `settings.json` as `edge_trim`; API tests in `pakon-web/tests/api.rs`.
   - `web/src/components/EdgeTrimFields.vue` (new), `SettingsSpace.vue`,
     `FramesSpace.vue`, `AdjustSpace.vue`, `web/src/state/{trim,app,roll}.ts`,
     `main.css`: the Sensor trim controls and inset frame boxes.
2. **Intel build.** `packaging/macos/build-intel.sh` (new) builds a native
   x86_64 app, version `1.1.0-trim`, no Sparkle. Its Unicorn check looks in
   `build/unicorn-engine-sys/*/out` (current Cargo layout).

## 7. Updating when upstream releases a new OpenPakon

```sh
cd ~/src/open-pakon-fork                     # git clone of the fork
git remote add upstream https://github.com/jorshhh/open-pakon.git 2>/dev/null
git fetch upstream --tags
git checkout sensor-trim
git rebase upstream/main                     # or the new release tag
# resolve conflicts if any (likely spots: pakon-web/src/jobs.rs, SettingsSpace.vue)
# bump the version string in build-intel.sh, e.g. 1.2.0-trim
packaging/macos/build-intel.sh               # with the PATH setup from section 1
ditto dist/macos/Pakon.app "/Applications/Pakon Trim.app"
xattr -dr com.apple.quarantine "/Applications/Pakon Trim.app"
git push --force-with-lease origin sensor-trim
```

Then re-check Sensor trim is still Top = 10 px in Settings.
