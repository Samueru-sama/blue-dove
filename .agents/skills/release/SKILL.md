---
name: release
description: Explain, operate and troubleshoot the release pipeline of a pkgforge-apps app (release-plz on GITHUB_TOKEN, release assets, onelf bundle, crates.io trusted publishing). Use when cutting a release, when a release or release PR misbehaves, when adding a build target, or when an app should be published to crates.io.
---

# Release pipeline

Everything runs on `GITHUB_TOKEN`. Releases, release PRs and their
commits belong to github-actions[bot]. Never add a personal access token
or a registry token to the repository secrets.

The jobs live in reusable workflows in
[pkgforge-apps/.github](https://github.com/pkgforge-apps/.github), shared
by every app. The app carries two small callers, synced from there and
pinned to the revision they were synced from:

| App workflow | Calls | Started by |
|--------------|-------|------------|
| `release-plz.yml` | `app-release-plz.yml` | every push to `main` |
| `release.yml` | `app-release.yml` with a `tag` input | `release-plz.yml`, or by hand |

A change to how apps release is made in pkgforge-apps/.github and
reaches every app through its sync pull request; see the
`sync-guidelines` skill.

## How a release happens

1. A push to `main` runs the `release` job. Nothing is pending, so it
   does nothing. Then `release-pr` opens or updates the release PR: the
   version bump in `Cargo.toml` and `Cargo.lock`, and the new
   `CHANGELOG.md` section, computed from conventional commits since the
   last tag.
2. A PR pushed with `GITHUB_TOKEN` does not trigger `pull_request`
   workflows, so `release-pr` dispatches `ci.yml` on the PR branch
   itself. Its checks show on the PR as usual.
3. Merging the release PR pushes to `main`. `release` now tags the
   version (`v<version>`) and creates the GitHub release
   with the changelog section, as a draft (`git_release_draft` in
   `.release-plz.toml`).
4. A release created with `GITHUB_TOKEN` cannot trigger `on: release`
   workflows, but `workflow_dispatch` is exempt, so `release` starts
   `release.yml` with the new tag. It builds the assets in the `build`,
   `onelf` and, when the app ships one, `appimage` jobs, and `publish`
   uploads them all at once, with build provenance attestations, then
   publishes the release.

The organization enforces immutable releases: once published, a release
takes no more assets and none can be replaced. That is why it stays a
draft until `publish` has attached everything, and why a published
release is never rebuilt; a fix ships as a new release.

To release, merge the release PR. To change the next version, edit it in
the release PR, or use `feat!:` and `BREAKING CHANGE:` for a major bump.

## Assets

| Job | Runner | Output |
|-----|--------|--------|
| build | ubuntu-22.04 (low glibc floor), ubuntu-22.04-arm | `<app>-<arch>-linux.tar.xz` |
| build | macos-latest, both arches | `<app>-<arch>-macos.dmg` with the `.app` |
| build | windows-latest | `<app>-x86_64-windows.exe`, the C runtime linked in |
| onelf | Arch containers | `<app>-<arch>-linux.onelf` |
| appimage | Arch containers, when opted in | `<app>-anylinux-<arch>.AppImage` and its `.zsync` |

Packaging is in `scripts/package.sh`, `scripts/package-onelf.sh` and
`scripts/package-appimage.sh`, so it runs the same locally. No `.sha256`
files: users verify with

```sh
gh attestation verify <file> -R pkgforge-apps/<name> --signer-repo pkgforge-apps/.github
```

The attestation is signed by the shared workflow in
pkgforge-apps/.github, so `-R` alone, which expects the app's own
workflow, refuses it.

### The onelf bundle

`onelf.toml` packs with the stipplefs payload, strips libraries, scans
the binary for dlopen'd names, keeps GTK and libdecor out (winit never
uses them) and force-includes the X11 libraries winit loads by name. The
bundle is built on Arch, because glibc bundled from Ubuntu did not run
everywhere, and the job smoke-runs it under Xvfb.

It also bundles Mesa (`gl` and `dri`). Without it the app fails on any
host where onelf finds no usable GL: Alpine and other musl
distributions, whose drivers a glibc process cannot load, and minimal
VMs and containers without Mesa. NVIDIA's proprietary driver is still
taken from the host when installed.

The Mesa is [onelf-pkgs/mesa](https://github.com/onelf-pkgs/mesa)'s,
built without LLVM, not Arch's, whose `libLLVM` took the bundle from
about 7 MB to about 60 MB; this one adds about 8 MB. When `onelf.toml`
turns on `gl` or `dri`, the job removes the files of Arch's `mesa`
package, keeping the libraries Mesa needs, and copies in the release's
GL tree, so onelf bundles it as the build host's Mesa. Its software
renderer is softpipe, slower than llvmpipe, which needs LLVM; hosts with
a GPU use their driver as before.

The same step puts [onelf-pkgs/opus](https://github.com/onelf-pkgs/opus)
in place of Arch's `opus` when the build installed it, as PulseAudio
does. Arch's carries the models of Opus's neural network features, over
4 MB a bundle would carry unused.

When the app starts loading another library at runtime that the scan
cannot see, add its soname to `include`. To check what a bundle carries:

```sh
onelf list dist/<app>-x86_64-linux.onelf | grep '\.so'
```

The `onelf` skill covers debugging a bundle, and the `test-linux-bundle`
skill checks one on the hosts users have. Run the latter before merging
any change to `onelf.toml`.

To bump onelf, update `PINNED_ONELF_VERSION` and both `onelf-sha256` values in
`.github/workflows/app-release.yml` of pkgforge-apps/.github. It is
shared, so the bump reaches every app. GitHub lists the digests:

```sh
gh api repos/QaidVoid/onelf/releases/tags/<version> \
  --jq '.assets[] | "\(.name) \(.digest)"'
```

Then build the bundle with the new onelf and run the
`test-linux-bundle` skill on it before merging.

To bump Mesa or Opus, edit its row in the table of the "Install
onelf-pkgs builds in place of Arch's" step in the same file: the
release and the two SHA-256 values, from the trees of the new
onelf-pkgs release. The asset prefix is `gl` for Mesa and `opus` for
Opus:

```sh
gh api repos/onelf-pkgs/<mesa or opus>/releases/tags/<tag> \
  --jq '.assets[] | select(.name | test("^(gl|opus)-(x86_64|aarch64)\\.tar\\.zst$")) | "\(.name) \(.digest)"'
```

Test a bundle built with it the same way.

To try another onelf on a release that is still a draft without
changing the pin, build its assets with the `onelf-version` input:

```sh
gh workflow run release.yml -f tag=v<version> -f onelf-version=<onelf version>
```

That version is checked against the SHA-256 digest GitHub lists for its
release asset rather than a pinned one. Releases started by release-plz
always use the pinned version.

### Self-update

Every onelf bundle updates itself: `<bundle> --onelf-update` fetches the
same asset from the app's latest release and downloads only the blocks
that changed. `scripts/package-onelf.sh` adds the `[update]` section at
build time, with that asset's URL and a revision derived from the
version (1.2.3 is 1002003), so a client never moves to an older release.
Do not add `[update]` to `onelf.toml` yourself; packaging refuses it.

Updates are signed once the app has its key:

1. bootstrap-app creates the `release` environment, usable by `main`
   alone.
2. The `provision-apps` workflow of pkgforge-apps/.github generates a
   keypair, stores the secret there as `ONELF_SIGNING_KEY`, and opens a
   pull request committing the public half as
   `packaging/onelf/update.pub`. It runs daily, or by hand:
   `gh workflow run provision-apps.yml -R pkgforge-apps/.github`.
3. Once that merges, the release job signs each bundle and publishes
   `<asset>.onelf.sig` beside it, which is where the runtime looks.
   Installed bundles accept only signed updates from then on.

Until then a bundle updates unsigned: it trusts HTTPS and GitHub alone,
and onelf says so on every update. The key is never replaced
automatically, since every bundle released with it embeds it; replacing
it takes a keyring, as onelf's self-update guide describes.

### The AppImage

An app ships an Anylinux AppImage next to the onelf bundle by setting
this in its `Cargo.toml`:

```toml
[package.metadata.pkgforge]
appimage = true
```

The `appimage` job is skipped otherwise. The `anylinux-appimage` skill
covers building, checking and debugging the image.

## Troubleshooting

- **`the release environment has no ONELF_SIGNING_KEY`.** The app has
  `packaging/onelf/update.pub` but its secret is gone. Restore it, or
  rotate the key through a keyring by hand; a new key alone would stop
  every installed bundle from updating.
- **The onelf job waits or fails on its environment.** The job uses the
  `release` environment, which only `main` may use, so a release run
  started from another branch is refused.
- **No release PR appears.** The repository or organization does not let
  Actions create pull requests. See step 7 of `bootstrap-app`.
- **A build job failed.** Re-run the failed jobs of the `release` run.
  `publish` waits for every build, so the release stays a draft without
  assets until a run succeeds, and never ships with platforms missing.
  When the fix needs a code change the tag does not include, delete the
  draft and its tag and cut a new release.
- **A draft release has no assets.** The dispatch did not happen or
  failed. Build them by hand:
  `gh workflow run release.yml -f tag=v<version>`.
- **`is published, so it is immutable`.** The run was started for a
  release that is already out. Published releases cannot change; ship
  the fix as a new release. Deleting the release does not free its
  version either: a tag that belonged to a published immutable release
  can never be created again. A draft's tag can, which is why a failed
  build is fixed by deleting the draft and its tag.
- **`Cannot upload assets to an immutable release`.** The release was
  published before its assets were attached: `git_release_draft = true`
  is missing from `.release-plz.toml`.
- **The push that merged the release PR did not start the workflow.**
  Run it manually: `gh workflow run release-plz.yml --ref main`.
- **The release PR repeats old history.** A tag is missing. Tags are the
  source of truth while `git_only = true`.

## Publishing to crates.io

Apps are git-only by default: release-plz reads versions from git tags
and never runs `cargo publish`. An app can also publish to crates.io, so
it installs with `cargo install`. The shared workflows need no change:
the `release` job already has `id-token: write`, and without a
`CARGO_REGISTRY_TOKEN` release-plz uses crates.io trusted publishing to
get a publish token for each run. Never add that secret.

crates.io cannot create a crate through trusted publishing, so the first
version is published by hand, and it has to happen before the switch
lands. Otherwise release-plz's next `release` run tries to publish a
crate that does not exist yet and fails. In order:

1. Check the crate name is free on crates.io. If it is taken, the crate
   needs another name, which is a larger change; stop and ask.
2. Check the crate packages: drop `publish = false` from `Cargo.toml`
   locally, without committing, and run
   `cargo publish --dry-run --locked`.
3. Publish the version `main` is at, which is the last release, from
   that same local change: the user runs `cargo publish --locked` with a
   short-lived crates.io token of their own, scoped to `publish-new`.
   Do not store the token anywhere. The registry and the git tags then
   agree on the current version.
4. On crates.io, under the crate's Settings > Trusted Publishing, add
   GitHub: owner `pkgforge-apps`, repository `<name>`, workflow
   `release-plz.yml`, no environment. crates.io checks the workflow the
   run started from, which is the app's caller, not the shared
   reusable workflow.
5. Commit the switch as `chore: publish to crates.io`: remove
   `git_only = true` from `.release-plz.toml` and `publish = false` from
   `Cargo.toml`. From the next release on, release-plz publishes the
   crate, then tags and releases it as before.
6. Point the install instructions at crates.io:
   `cargo install --locked <name>` in the README and
   `docs/guide/installation.md`.

## Adding a target

Targets are shared. Add a row to the `build` matrix of
`.github/workflows/app-release.yml` in pkgforge-apps/.github with the
platform name, runner and Rust target, and make sure
`app/scripts/package.sh` there handles the target's OS. Then add the
matching link in each app's
`docs/.vitepress/theme/components/Downloads.vue` and README.
