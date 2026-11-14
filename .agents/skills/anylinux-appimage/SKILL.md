---
name: anylinux-appimage
description: Ship, build and debug a pkgforge-apps app's Anylinux AppImage, a single file that runs on any Linux distribution, musl and old kernels included. Covers opting in, scripts/package-appimage.sh, the release job, checking the image and the failures seen so far. Use when an app should ship an AppImage, when the appimage release job fails, when the image does not start, or when choosing between the AppImage and the onelf bundle.
---

# Anylinux AppImages

An Anylinux AppImage is one executable that carries the app, glibc, the
dynamic loader and every library the app loads. It runs on any Linux
distribution, musl ones and kernels back to 2.6.17 included, without
FUSE: uruntime falls back to namespaces, then to extract and run. The
payload is DwarFS-compressed.

It is built with [quick-sharun](https://github.com/pkgforge-dev/Anylinux-AppImages/blob/main/useful-tools/quick-sharun.sh),
a wrapper around the [Anylinux sharun fork](https://github.com/pkgforge-dev/Anylinux-sharun)
and [appimagetool](https://github.com/pkgforge-dev/appimagetool). Read
[HOW-TO-MAKE-THESE](https://github.com/pkgforge-dev/Anylinux-AppImages/blob/main/HOW-TO-MAKE-THESE.md)
and the [FAQ](https://github.com/pkgforge-dev/Anylinux-AppImages/blob/main/FAQ.md)
before changing the build.

It is a second Linux format next to the onelf bundle, not a
replacement, for users and AppImage managers that expect it.

## Ship one

Opt in from the app's `Cargo.toml`, where the template sets it to
`false`:

```toml
[package.metadata.pkgforge]
appimage = true
```

The `appimage` job of the shared `app-release.yml` then builds
`<app>-anylinux-<arch>.AppImage` and its `.zsync` for x86_64 and aarch64,
smoke-runs the image under Xvfb, and hands both to `publish` with every
other asset.

The name says what the file is. An Anylinux AppImage does not depend on
the host's libc and runs on any distribution, so a `-linux` suffix
described a narrower thing than the file is, and read as a build for one
distribution rather than for all of them.

Tell users about it in the same change:

- set `appimage: true` in `docs/.vitepress/app.ts`, which adds it to the
  docs site's downloads;
- keep the parts of `README.md` and `docs/guide/installation.md` marked
  `<!-- AppImage` and the README's AppImage table row. An app that opts
  out again deletes them.

The `.zsync` is what keeps installed copies current: the update
information points at `*anylinux-<arch>.AppImage.zsync` on the latest
release, and the self-updater hook reads it. An image published
without it silently stops updating.

### Why the app does not build it

`publish` attaches the image in the same step that publishes the
release, and a published release is immutable, so the image is on the
draft before that step runs. An app's own `appimage.yml` runs beside the
release workflow rather than before it, and whichever of the two
finishes second attaches to a release that has already gone out:
blue-dove v0.1.9 lost that race by twenty-three seconds and got
`HTTP 422: Cannot upload assets to an immutable release`. A release that
misses its image stays without one, so ship the fix as a new release.

Everything the image needs is an app-side file the shared job reads, so
a need that looks like it wants a workflow of its own is a change to the
shared job: `appimage = true` under `[package.metadata.pkgforge]` for
the opt-in, `.github/pacman-packages.txt` for a package quick-sharun has
to find on the build host, and `packaging/appimage/deploy.env` for the
GL choice and a `DEPLOY_*` variable detection misses. With those, the image never has to
leave the release job.

## How it is built

`scripts/package-appimage.sh <platform> <version>` installs the release binary to
`/usr/bin` and runs quick-sharun on it, which traces what the binary
links and opens and deploys it into the AppDir. Never copy libraries or
binaries into the AppDir by hand.

- **Arch only.** quick-sharun bundles the build host's libraries and is
  supported on Arch Linux only. The job runs in
  `ghcr.io/pkgforge-dev/archlinux`, pinned by digest, and
  `pkgforge-dev/anylinux-setup-action` installs the tools.
- **Debloated packages.** `get-debloated-pkgs --add-common --prefer-nano`
  swaps Mesa and other large packages for debloated builds from
  pkgforge-dev, which keeps the image far smaller than Arch's own
  packages would. It runs before quick-sharun traces the binary.
- **The whole GL stack.** Left to detection, quick-sharun bundles
  `libGLX` and gallium but not `libEGL`, the `dri/` drivers or
  `libgbm`, which leaves no Wayland path and no driver behind the GL it
  carries. An app that draws with GL, as an egui app does, sets
  `DEPLOY_OPENGL=1` in `packaging/appimage/deploy.env`, and the script
  then fails when the AppDir has no `libEGL.so.1` or `dri/`. An app
  that draws without GL on a host that has none sets
  `USE_HOST_DRIVERS=1` there instead, so quick-sharun leaves the
  drivers on the host. Set another `DEPLOY_*` variable only
  when detection gets that one wrong, and say why in the app's own
  section of `AGENTS.md`.
- **A stack reached by name.** quick-sharun only detects a deploy mode
  from what the binary links, so a stack it loads through another
  library stays out: an audio app reaches the sound server through an
  ALSA plugin, and nothing links `libpulse`, so `DEPLOY_PULSE` never
  turns on by itself and the image plays no sound. Such an app names the
  variables it needs in `packaging/appimage/deploy.env`, a shell
  fragment:

  ```sh
  # cpal reaches the server through an ALSA plugin, not by linking it.
  DEPLOY_PULSE=1
  ```

  Deploy only what the image's default PCM resolves to. The shared job
  installs PulseAudio, not PipeWire, and where both are present the
  pulse config sorts later, so pulse is the default PCM either way. The
  pulse plugin serves a PulseAudio host and a PipeWire one running
  pipewire-pulse, which is the usual desktop setup. An app that also
  installs the PipeWire packages and asks for `DEPLOY_PIPEWIRE` ships
  its client library and SPA plugins that no config selects.

  `scripts/package-appimage.sh` sources the file before it runs
  quick-sharun and before it sets its own variables, so a value the file
  sets that the script also uses is overwritten rather than obeyed. It
  sources with `set -a`, so a plain assignment in the file is exported
  and reaches quick-sharun, a child process; a file of bare
  `NAME=value` lines is enough. An app without the file gets detection
  alone. The packages the deployment then bundles, such as the server
  itself, belong in `.github/pacman-packages.txt`, since quick-sharun
  copies only what the build host has.
- **Pins.** The job fetches quick-sharun and `get-debloated-pkgs` from
  one Anylinux-AppImages commit; quick-sharun pins sharun and
  appimagetool itself. To bump, change the commit in the three URLs in
  the job's `env` in pkgforge-apps/.github, build an image, and run the
  checks below on it.

## Build one locally

The script expects the container the job runs in. From the app's root:

```sh
podman run --rm --security-opt label=disable -v "$PWD:/src" -w /src \
  ghcr.io/pkgforge-dev/archlinux:latest sh -c '
    set -e
    wget -qO /tmp/setup.sh https://raw.githubusercontent.com/pkgforge-dev/anylinux-setup-action/main/setup.sh
    sh /tmp/setup.sh
    pacman -S --noconfirm --needed rustup wayland libxkbcommon \
      libxkbcommon-x11 libx11 libxcursor libxi libxrandr xorg-xauth
    get-debloated-pkgs --add-common --prefer-nano
    rustup default stable
    cargo build --release --locked
    sh scripts/package-appimage.sh "anylinux-$(uname -m)" 0.0.0
    xvfb-run -a timeout 60 dist/*.AppImage --smoke'
```

Without `GITHUB_REPOSITORY` the image has no update information and no
`.zsync`, which is fine for testing. `target/` and `dist/` end up owned
by root.

## Check the image

The job smoke-runs the image; to check one by hand:

```sh
xvfb-run -a timeout 60 dist/*.AppImage --smoke
```

`--smoke` has to draw and exit 0. quick-sharun's own `--test` wants the
app to stay open for ten seconds, so it does not fit apps here.

A host with Mesa supplies what the image is missing, so an image
without its own GL stack passes every test run on a desktop and fails
on the one host that matters. When `deploy.env` sets `DEPLOY_OPENGL=1`,
the script's check covers `libEGL` and `dri/`. To look further:

```sh
cd "$(mktemp -d)" && /path/to/<app>-anylinux-x86_64.AppImage --appimage-extract > /dev/null
find squashfs-root -name '*.so*' | sort
```

The `test-linux-bundle` skill's script takes an AppImage as well as an
onelf bundle. In its containers, which have no FUSE, the image falls
back to extract and run. Do not install Mesa in a test container: on
Alpine, `xvfb-run` pulls in `mesa-gl`, so the host ends up with a GL
stack of its own.

## Rust and egui

- egui draws through glow, so the image needs the whole GL stack: keep
  the template's `DEPLOY_OPENGL=1` in `packaging/appimage/deploy.env`.
  A smudge app drawing with its CPU renderer does not need it, but
  carrying it does no harm.
- The build host needs the winit libraries: `wayland`, `libxkbcommon`,
  `libxkbcommon-x11`, `libx11`, `libxcursor`, `libxi` and `libxrandr`.
  quick-sharun's strace mode finds the ones winit opens by name.
- A dependency that runs an external helper, such as a file dialog
  backend or a portal, needs that helper in the image. quick-sharun
  deploys libraries, not executables: pass the helper to quick-sharun
  as another argument.
- An app that checks for host libraries before starting fails, because
  the bundled libraries are not on the host loader path. Skip the check
  when `APPDIR` is set. Never make it pass by installing libraries on
  the host.
- The image's install prefix is `$APPDIR` itself, not `$APPDIR/usr`,
  and `$APPDIR/shared` belongs to sharun.

## Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| `ERROR: Missing dependency 'patchelf'` | the build host lacks quick-sharun's tools | run in the Arch image after the setup action |
| `ERROR: Missing '.DirIcon'` | the icon cannot be found | regenerate `assets/icons/` with `scripts/icons.sh` |
| `the AppDir has no libEGL.so.1 or no dri/` | quick-sharun did not deploy the GL stack `deploy.env` asked for | check Mesa is installed in the container and `deploy.env` does not also set `USE_HOST_DRIVERS=1` |
| `error while loading shared libraries` at runtime | a library is missing from the image | raise `STRACE_TIME`, make the app reach the code path that opens it, or pass the library to quick-sharun |
| `symbol lookup error` | a host library was used instead of the bundled one | keep the defaults `ANYLINUX_LIB=1` and `CROSS_LIBC_DLOPEN=1`, and never set `LD_LIBRARY_PATH` |
| works in the build container, fails on a host | a host library check or a hardcoded `/usr` path | skip the check; map the path with `PATH_MAPPING` |
| `Can't find a valid SQUASHFS superblock` | the user runs it through NixOS `appimage-run` | run the AppImage directly; it needs no FHS wrapper |
| no thumbnail in a file manager | DwarFS, not SquashFS | install a DwarFS-aware AppImage thumbnailer |
| the self-updater does nothing | no `.zsync` on the release, or an AppImage manager disabled the updater | check the release has both assets; managers disable it by design |
| the image is much bigger than expected | the debloated packages were not installed, or a dependency pulled in a large tree | check `get-debloated-pkgs` ran, and `cargo tree` for the dependency |
