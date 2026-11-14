#!/bin/sh
# Packs the native release build into an Anylinux AppImage in dist/.
#
# Usage: scripts/package-appimage.sh <platform> <version>
#   scripts/package-appimage.sh anylinux-x86_64 0.1.0
#
# Produces dist/<app>-<platform>.AppImage and, when GITHUB_REPOSITORY is
# set, the .zsync its self-updater reads. Runs as root on Arch with the
# Anylinux tools and the debloated packages installed, after `cargo build
# --release --locked`; the release workflow sets that up. The image
# carries glibc, the loader and every library the binary loads, all from
# the build host, and the GL stack as the app's deploy.env asks.
set -eu
cd "$(dirname "$0")/.."

# Detection cannot see a stack the app reaches by name rather than by
# linking, such as the audio servers behind libasound. An app that needs
# one names the variables in packaging/appimage/deploy.env. It is sourced
# before this script sets its own variables, so the script's values win
# over anything the file happens to set, and with `set -a` in force, so
# quick-sharun, a child process, receives them. The packages quick-sharun
# then bundles come from .github/pacman-packages.txt.
deploy_env=packaging/appimage/deploy.env
if [ -f "$deploy_env" ]; then
    set -a
    # shellcheck disable=SC1090 # the app supplies this file
    . "$deploy_env"
    set +a
fi

platform=$1
version=$2
app=$(sed -n 's/^name = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)

# quick-sharun traces the binary where it is installed.
install -Dm755 "target/release/$app" "/usr/bin/$app"

export APPDIR="$PWD/dist/AppDir"
export OUTPATH="$PWD/dist"
export OUTNAME="$app-$platform.AppImage"
export VERSION="$version"
export DESKTOP="$PWD/packaging/linux/app.desktop"
export ICON="$PWD/assets/icons/256.png"

if [ -n "${GITHUB_REPOSITORY:-}" ]; then
    export UPINFO="gh-releases-zsync|${GITHUB_REPOSITORY%/*}|${GITHUB_REPOSITORY#*/}|latest|*$platform.AppImage.zsync"
    export ADD_HOOKS=self-updater.hook
fi

rm -rf "$APPDIR"
quick-sharun "/usr/bin/$app"

# An app that asks for the GL stack gets all of it or no image: a host with
# Mesa makes up for a bundle without it, so no test run would catch these
# missing.
if [ "${DEPLOY_OPENGL:-0}" = 1 ] && {
    [ -z "$(find "$APPDIR" -name libEGL.so.1)" ] ||
        [ -z "$(find "$APPDIR" -type d -name dri)" ]
}; then
    echo "the AppDir has no libEGL.so.1 or no dri/" >&2
    exit 1
fi

quick-sharun --make-appimage
rm -rf "$APPDIR" dist/appinfo
ls "dist/$OUTNAME"
