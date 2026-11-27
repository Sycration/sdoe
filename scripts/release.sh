#!/usr/bin/env bash
# Builds the sdoe release assets into dist/v<version>/:
#
#   sdoe-<v>-linux-x86_64.AppImage     desktop application, Qt bundled (glibc 2.28 or newer)
#   sdoe-<v>-windows-x86_64.zip        desktop application, Qt bundled
#   sdoe-<v>-macos.zip                 desktop application (universal), Qt bundled
#   sdoe-web-<v>-<platform>[.exe]      web server with the WebAssembly build of the
#                                      application built in (Linux x86_64 and
#                                      aarch64, static; Windows x86_64; macOS universal)
#   libsdoe-<v>-linux-x86_64.tar.gz    shared library (glibc 2.28 or newer)
#   libsdoe-<v>-linux-aarch64.tar.gz   shared library (glibc 2.28 or newer)
#   libsdoe-<v>-windows-x86_64.zip     DLL with MinGW and MSVC import libraries
#   libsdoe-<v>-android.zip            jniLibs/<abi>/libsdoe.so for 4 ABIs
#   libsdoe-<v>-macos.tar.gz           dynamic library (universal: arm64 and x86_64)
#   libsdoe-<v>-wasm.zip               WebAssembly module with the JavaScript interface
#   SHA256SUMS
#
# and draft release notes into dist/v<version>-notes.md.
#
# Usage: scripts/release.sh [--skip-tests] [--add] [step...]
# Steps (default: all): lib-linux-x86_64 lib-linux-aarch64 lib-windows-x86_64
#                       lib-macos lib-android lib-wasm app-linux-x86_64
#                       app-windows-x86_64 app-macos web
#
# --add puts the assets of the given steps into an existing dist/v<version>/
# (existing files stay and are not replaced) and writes new checksums and
# notes for the whole directory.
#
# Requirements (Fedora package names):
#   all:      rustup, zip, tar, sha256sum
#   lib-linux-*:   cargo-zigbuild and zig
#   lib-windows-*: mingw64-gcc, zig (MSVC import library)
#   lib-android:   cargo-ndk and the Android NDK (ANDROID_NDK_HOME)
#   lib-wasm:      nodejs (tests)
#   *-macos:       osxcross (its target/bin on PATH, or OSXCROSS_BIN), python3
#   app-macos:     the host Qt (QMAKE) and Qt for macOS of the same version
#                  (MACOS_QT; by default downloaded with aqtinstall into
#                  build/tools on first use)
#   web:           the host Qt, Qt for WebAssembly of the same version
#                  (WASM_QT; downloaded like MACOS_QT), emscripten in the version
#                  of that Qt (emcc on PATH, or EMSDK), and what lib-linux-*,
#                  lib-windows-* and *-macos need
#   app-linux-*:   podman or docker, python3, curl; downloads Qt 6.8 LTS and
#                  linuxdeploy into build/tools on first use, and builds the
#                  application in an AlmaLinux 8 container, so that it runs on
#                  glibc 2.28 and newer (newer of the host's glibc is not used)
#   app-windows-*: mingw64-gcc-c++, mingw64-qt6-qtbase, mingw64-qt6-qtdeclarative,
#                  mingw64-qt6-qtsvg, mingw64-qt6-qtimageformats
#
# Settings (environment):
#   ANDROID_NDK_HOME  NDK directory (default: newest in ~/Android/ndk or ~/Android/Sdk/ndk)
#   ANDROID_API       minimum Android API level (default 21)
#   GLIBC             minimum glibc of the Linux libraries (default 2.28)
#   QMAKE             qmake of the host Qt, for macOS and WebAssembly (default qmake6)
#   MINGW_QMAKE       qmake of the MinGW Qt (default x86_64-w64-mingw32-qmake-qt6)
#   MINGW_ROOT        MinGW sysroot (default /usr/x86_64-w64-mingw32/sys-root/mingw)
#   LINUXDEPLOY_TAG, LINUXDEPLOY_QT_TAG  release tags of the linuxdeploy tools
#   PATCHELF_VERSION  patchelf release used by linuxdeploy (default 0.19.3)
#   LINUX_QT_VERSION  Qt LTS release of the Linux application (default 6.8.3;
#                     6.8 is the newest LTS whose official binaries need glibc 2.28)
#   LINUX_QT          Qt for Linux directory (default: downloaded into
#                     build/tools/qt-linux like MACOS_QT)
#   LINUX_IMAGE       build container image (default localhost/sdoe-linux-build:8
#                     with a hash of packaging/linux/Containerfile; built when
#                     that image is missing)
#   LINUX_CONTAINER   podman or docker (default: podman if present, then docker)
#   OSXCROSS_BIN      osxcross target/bin directory (default: from o64-clang on PATH)
#   MACOS_QT          Qt for macOS directory, e.g. .../6.11.2/macos
#   WASM_QT           Qt for WebAssembly directory, e.g. .../6.11.2/wasm_singlethread
#   EMSDK             emsdk directory (default: build/tools/emsdk if it exists)
#   MACOSX_DEPLOYMENT_TARGET  minimum macOS of the macOS binaries (default 11.0;
#                     the Qt of the application sets its own minimum)

set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$ROOT"

# The Linux build container has the newer compiler of a toolset, which keeps
# the runtime glibc of the container (2.28): the system GCC of AlmaLinux 8 is
# too old for the Qt 6.8 headers.
if [ -n "${SDOE_LINUX_CONTAINER:-}" ] && [ -f /opt/rh/gcc-toolset-11/enable ]; then
    . /opt/rh/gcc-toolset-11/enable
fi

ALL_STEPS=(lib-linux-x86_64 lib-linux-aarch64 lib-windows-x86_64 lib-macos lib-android lib-wasm app-linux-x86_64 app-windows-x86_64 app-macos web)
ANDROID_ABIS=(arm64-v8a armeabi-v7a x86_64 x86)
ANDROID_RUST_TARGETS=(aarch64-linux-android armv7-linux-androideabi x86_64-linux-android i686-linux-android)

ANDROID_API=${ANDROID_API:-21}
GLIBC=${GLIBC:-2.28}
APP_GLIBC=2.28
QMAKE=${QMAKE:-qmake6}
MINGW_QMAKE=${MINGW_QMAKE:-x86_64-w64-mingw32-qmake-qt6}
MINGW_ROOT=${MINGW_ROOT:-/usr/x86_64-w64-mingw32/sys-root/mingw}
LINUXDEPLOY_TAG=${LINUXDEPLOY_TAG:-1-alpha-20251107-1}
LINUXDEPLOY_QT_TAG=${LINUXDEPLOY_QT_TAG:-continuous}
PATCHELF_VERSION=${PATCHELF_VERSION:-0.19.3}
LINUX_QT_VERSION=${LINUX_QT_VERSION:-6.8.3}
LINUX_IMAGE=${LINUX_IMAGE:-}
LINUX_IMAGE_DEFAULT=localhost/sdoe-linux-build:8
MACOSX_DEPLOYMENT_TARGET=${MACOSX_DEPLOYMENT_TARGET:-11.0}

# Release builds use their own target directory, stripped binaries, and zlib
# instead of zstd for Qt resources (the MinGW Qt reads only zlib). A container
# build overrides the target directory, since host-glibc objects must not mix
# with the ones built for the old glibc.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target/dist}"
export CARGO_PROFILE_RELEASE_STRIP=symbols
export CXX_QT_AUTORCC_OPTIONS="--compress-algo:zlib"

say() { printf '\n\033[1m==> %s\033[0m\n' "$*" >&2; }
die() { printf '\033[31merror:\033[0m %s\n' "$*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || die "$1 not found${2:+ ($2)}"; }

usage() { sed -n '2,/^$/p' "$0" | sed 's/^# \{0,1\}//'; exit "${1:-0}"; }

SKIP_TESTS=0
ADD=0
# Set by app_linux when it re-runs this script inside the build container: the
# nested run builds the asset into SDOE_WORK_DIR and leaves checksums and notes
# to the outer run.
IN_LINUX_CONTAINER=${SDOE_LINUX_CONTAINER:-0}
SKIP_FINALIZE=${SDOE_SKIP_FINALIZE:-0}
STEPS=()
for arg in "$@"; do
    case "$arg" in
        --skip-tests) SKIP_TESTS=1 ;;
        --add) ADD=1 ;;
        -h|--help) usage ;;
        *)
            [[ " ${ALL_STEPS[*]} " == *" $arg "* ]] || { echo "unknown step: $arg" >&2; usage 1; }
            STEPS+=("$arg")
            ;;
    esac
done
[ ${#STEPS[@]} -gt 0 ] || STEPS=("${ALL_STEPS[@]}")
wants() { [[ " ${STEPS[*]} " == *" $1 "* ]]; }

# ---------------------------------------------------------------------------
# Version, output directories, prerequisites
# ---------------------------------------------------------------------------

version_of() { sed -n 's/^version = "\(.*\)"$/\1/p' "$1" | head -1; }
VERSION=$(version_of core/Cargo.toml)
[ -n "$VERSION" ] || die "no version in core/Cargo.toml"
[ "$(version_of app/Cargo.toml)" = "$VERSION" ] || die "core and app versions differ"
grep -q "\"version\": \"$VERSION\"" js/package.json || die "the version in js/package.json differs from core"

DIST="$ROOT/dist/v$VERSION"
NOTES="$ROOT/dist/v$VERSION-notes.md"
if [ "$SKIP_FINALIZE" = 0 ]; then
    if [ "$ADD" = 1 ]; then
        [ -d "$DIST" ] || die "--add needs an existing $DIST"
    else
        [ ! -e "$DIST" ] || die "$DIST already exists; remove it, change the version or use --add"
    fi
fi

if [ "$IN_LINUX_CONTAINER" = 0 ]; then
    need cargo
    need rustup
    need zip
    need tar
    need sha256sum
fi
if wants lib-linux-x86_64 || wants lib-linux-aarch64; then
    need cargo-zigbuild "cargo install cargo-zigbuild"
    need zig
fi
if wants lib-windows-x86_64; then
    need x86_64-w64-mingw32-gcc "dnf install mingw64-gcc"
    need x86_64-w64-mingw32-objdump "dnf install mingw64-binutils"
    need zig
fi
if wants lib-android; then
    need cargo-ndk "cargo install cargo-ndk"
    if [ -z "${ANDROID_NDK_HOME:-}" ]; then
        ANDROID_NDK_HOME=$(ls -d "$HOME"/Android/ndk/*/ "$HOME"/Android/Sdk/ndk/*/ 2>/dev/null | sort -V | tail -1 || true)
        ANDROID_NDK_HOME=${ANDROID_NDK_HOME%/}
    fi
    [ -n "$ANDROID_NDK_HOME" ] && [ -d "$ANDROID_NDK_HOME" ] || die "Android NDK not found; set ANDROID_NDK_HOME"
    export ANDROID_NDK_HOME
fi
if wants lib-wasm && [ "$SKIP_TESTS" = 0 ]; then
    need node "dnf install nodejs"
fi
if wants web; then
    need "$QMAKE" "dnf install qt6-qtbase-devel qt6-qtdeclarative-devel"
    need cargo-zigbuild "cargo install cargo-zigbuild"
    need zig
    need x86_64-w64-mingw32-gcc "dnf install mingw64-gcc"
    need x86_64-w64-mingw32-objdump "dnf install mingw64-binutils"
    # Only the WebAssembly build gets emcc on its PATH.
    if command -v emcc >/dev/null 2>&1; then
        EMCC_DIR=$(dirname "$(command -v emcc)")
    else
        EMSDK=${EMSDK:-$ROOT/build/tools/emsdk}
        EMCC_DIR="$EMSDK/upstream/emscripten"
        [ -x "$EMCC_DIR/emcc" ] || die "emscripten not found: put emcc on PATH or set EMSDK"
    fi
fi
if wants lib-macos || wants app-macos || wants web; then
    need python3
    if [ -z "${OSXCROSS_BIN:-}" ]; then
        need o64-clang "install osxcross and put its target/bin on PATH, or set OSXCROSS_BIN"
        OSXCROSS_BIN=$(dirname "$(command -v o64-clang)")
    fi
fi
if wants app-macos; then
    need "$QMAKE" "dnf install qt6-qtbase-devel qt6-qtdeclarative-devel"
fi
if wants app-linux-x86_64; then
    if [ "$IN_LINUX_CONTAINER" = 0 ]; then
        need python3
    else
        need "$QMAKE" "the Linux Qt used inside the build container"
        need curl
    fi
fi
if wants app-windows-x86_64; then
    need x86_64-w64-mingw32-g++ "dnf install mingw64-gcc-c++"
    need x86_64-w64-mingw32-objdump "dnf install mingw64-binutils"
    need "$MINGW_QMAKE" "dnf install mingw64-qt6-qtbase mingw64-qt6-qtdeclarative mingw64-qt6-qtsvg mingw64-qt6-qtimageformats"
    [ -d "$MINGW_ROOT/lib/qt6/qml/QtQuick" ] || die "QtQuick for MinGW not found (dnf install mingw64-qt6-qtdeclarative)"
fi

rust_target() {
    rustup target list --installed | grep -qx "$1" || { say "Adding Rust target $1"; rustup target add "$1"; }
}

if [ -n "$(git status --porcelain 2>/dev/null)" ]; then
    printf '\033[33mwarning:\033[0m the working tree has uncommitted changes; the assets will not match a tagged commit.\n'
fi

mkdir -p "$ROOT/build" "$ROOT/dist"
if [ -n "${SDOE_WORK_DIR:-}" ]; then
    WORK=$SDOE_WORK_DIR
    mkdir -p "$WORK"
else
    WORK=$(mktemp -d "$ROOT/build/release-$VERSION.XXXXXX")
fi
OUT="$WORK/assets"
mkdir -p "$OUT"
echo "Version $VERSION; working directory $WORK"

# Reproducible archive metadata.
export SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH:-$(git log -1 --format=%ct 2>/dev/null || date +%s)}
pack_tar() { tar -C "$WORK" --sort=name --owner=0 --group=0 --numeric-owner --mtime="@$SOURCE_DATE_EPOCH" -czf "$OUT/$1.tar.gz" "$1"; }
pack_zip() { (cd "$WORK" && find "$1" -exec touch -h -d "@$SOURCE_DATE_EPOCH" {} + && find "$1" | LC_ALL=C sort | zip -qX "$OUT/$1.zip" -@); }

if [ "$SKIP_TESTS" = 0 ]; then
    say "Testing the engine"
    cargo test --release -p sdoe-core
fi

# ---------------------------------------------------------------------------
# Library
# ---------------------------------------------------------------------------

# Creates the library staging directory with the header, API.md and LICENSE.
lib_stage() {
    local d="$WORK/libsdoe-$VERSION-$1"
    mkdir -p "$d/include"
    cp include/sdoe.h "$d/include/"
    cp API.md LICENSE "$d/"
    echo "$d"
}

lib_linux() {
    local arch=$1 triple="$1-unknown-linux-gnu"
    say "Library: Linux $arch (glibc $GLIBC)"
    rust_target "$triple"
    cargo zigbuild --release -p sdoe-core --lib --target "$triple.$GLIBC"
    local d
    d=$(lib_stage "linux-$arch")
    mkdir -p "$d/lib"
    cp "$CARGO_TARGET_DIR/$triple/release/libsdoe.so" "$d/lib/"
    pack_tar "libsdoe-$VERSION-linux-$arch"
}

lib_windows() {
    local triple=x86_64-pc-windows-gnu
    say "Library: Windows x86_64"
    rust_target "$triple"
    local d
    d=$(lib_stage windows-x86_64)
    mkdir -p "$d/bin" "$d/lib"
    # The export list goes to a fixed path, so that it stays valid when cargo
    # finds the DLL up to date and does not link again.
    local rel="$CARGO_TARGET_DIR/$triple/release"
    cargo rustc --release -p sdoe-core --lib --crate-type cdylib --target "$triple" \
        -- -C "link-arg=-Wl,--output-def,$rel/sdoe.def"
    cp "$rel/sdoe.def" "$d/lib/"
    cp "$rel/sdoe.dll" "$d/bin/"
    cp "$rel/libsdoe.dll.a" "$d/lib/"
    # MSVC import library from the export list.
    zig dlltool -m i386:x86-64 -D sdoe.dll -d "$d/lib/sdoe.def" -l "$d/lib/sdoe.lib"
    # The DLL must depend on Windows system DLLs only.
    local dep
    for dep in $(x86_64-w64-mingw32-objdump -p "$d/bin/sdoe.dll" | sed -n 's/^\s*DLL Name: //p'); do
        if find "$MINGW_ROOT/bin" -maxdepth 1 -iname "$dep" | grep -q .; then
            die "sdoe.dll depends on $dep, which Windows does not provide"
        fi
    done
    pack_zip "libsdoe-$VERSION-windows-x86_64"
}

lib_android() {
    say "Library: Android (${ANDROID_ABIS[*]}, API $ANDROID_API)"
    local t
    for t in "${ANDROID_RUST_TARGETS[@]}"; do rust_target "$t"; done
    local d args=()
    d=$(lib_stage android)
    for t in "${ANDROID_ABIS[@]}"; do args+=(-t "$t"); done
    cargo ndk -P "$ANDROID_API" "${args[@]}" -o "$d/jniLibs" build --release -p sdoe-core --lib
    for t in "${ANDROID_ABIS[@]}"; do
        [ -f "$d/jniLibs/$t/libsdoe.so" ] || die "libsdoe.so for $t is missing"
    done
    pack_zip "libsdoe-$VERSION-android"
}

# Sets up the osxcross compilers and linker for $1 (x86_64 or aarch64) in
# the environment. $2 is an optional linker to use instead of the osxcross
# clang. Sets OSX_PREFIX to the tool prefix, e.g. .../aarch64-apple-darwin21.2-.
macos_toolchain() {
    local cpu=$1 clang
    clang=$(compgen -G "$OSXCROSS_BIN/$cpu-apple-darwin*-clang" | head -n1) ||
        die "no $cpu-apple-darwin*-clang in $OSXCROSS_BIN"
    OSX_PREFIX=${clang%clang}
    export "CC_${cpu}_apple_darwin=${OSX_PREFIX}clang" "CXX_${cpu}_apple_darwin=${OSX_PREFIX}clang++" \
        "AR_${cpu}_apple_darwin=${OSX_PREFIX}ar" "RANLIB_${cpu}_apple_darwin=${OSX_PREFIX}ranlib" \
        "CARGO_TARGET_${cpu^^}_APPLE_DARWIN_LINKER=${2:-${OSX_PREFIX}clang}"
}

# Fails unless every arm64 slice of the given Mach-O files carries a valid
# signature (the linker signs ad hoc; arm64 Macs run nothing else).
macos_check_signatures() {
    python3 -I "$ROOT/scripts/macho-signature.py" "$@" || die "invalid code signatures"
}

lib_macos() {
    say "Library: macOS (universal, macOS $MACOSX_DEPLOYMENT_TARGET)"
    local cpu slices=()
    for cpu in x86_64 aarch64; do
        rust_target "$cpu-apple-darwin"
        macos_toolchain "$cpu"
        # The install name is set at link time, which keeps the linker's
        # signature valid.
        PATH="$OSXCROSS_BIN:$PATH" MACOSX_DEPLOYMENT_TARGET="$MACOSX_DEPLOYMENT_TARGET" \
            cargo rustc --release -p sdoe-core --lib --crate-type cdylib --target "$cpu-apple-darwin" \
            -- -C link-arg=-Wl,-install_name,@rpath/libsdoe.dylib
        slices+=("$CARGO_TARGET_DIR/$cpu-apple-darwin/release/libsdoe.dylib")
    done
    local d
    d=$(lib_stage macos)
    mkdir -p "$d/lib"
    "$OSXCROSS_BIN/lipo" -create -output "$d/lib/libsdoe.dylib" "${slices[@]}"
    macos_check_signatures "$d/lib/libsdoe.dylib"
    pack_tar "libsdoe-$VERSION-macos"
}

lib_wasm() {
    local triple=wasm32-unknown-unknown
    say "Library: WebAssembly with the JavaScript interface"
    rust_target "$triple"
    cargo build --release -p sdoe-core --lib --target "$triple"
    local wasm="$CARGO_TARGET_DIR/$triple/release/sdoe.wasm"
    if [ "$SKIP_TESTS" = 0 ]; then
        say "Testing the WebAssembly build against the native engine"
        cargo build --release -p sdoe-core --example replay
        node js/test.mjs "$wasm" "$CARGO_TARGET_DIR/release/examples/replay"
    fi
    local d="$WORK/libsdoe-$VERSION-wasm"
    mkdir -p "$d"
    cp js/sdoe.js js/sdoe.d.ts js/package.json API.md LICENSE "$wasm" "$d/"
    pack_zip "libsdoe-$VERSION-wasm"
}

# ---------------------------------------------------------------------------
# Application
# ---------------------------------------------------------------------------

fetch_tool() {
    local file=$1 url=$2 tools="$ROOT/build/tools"
    mkdir -p "$tools"
    if [ ! -x "$tools/$file" ]; then
        say "Downloading $file"
        curl -fL --retry 3 -o "$tools/$file.part" "$url"
        chmod +x "$tools/$file.part"
        mv "$tools/$file.part" "$tools/$file"
    fi
    echo "$tools/$file"
}

# Fails unless no ELF file in the AppImage needs a glibc newer than $2.
appimage_check_glibc() {
    local appimage=$1 max=$2 dir="$WORK/appimage-glibc" f v
    rm -rf "$dir"
    mkdir -p "$dir"
    (cd "$dir" && "$appimage" --appimage-extract >/dev/null)
    while IFS= read -r -d '' f; do
        v=$(objdump -T "$f" 2>/dev/null | grep -o 'GLIBC_[0-9][0-9.]*' | sed 's/GLIBC_//' | sort -Vu | tail -1 || true)
        [ -n "$v" ] || continue
        if [ "$(printf '%s\n%s\n' "$max" "$v" | sort -V | tail -1)" != "$max" ]; then
            die "${f#"$dir"/squashfs-root/} needs glibc $v; the AppImage targets $max"
        fi
    done < <(find "$dir/squashfs-root" -type f -print0)
}

app_linux() {
    say "Application: Linux x86_64 (AppImage, glibc $APP_GLIBC)"
    # Qt 6.8 LTS is the newest LTS whose official Linux binaries need only
    # glibc 2.28. The application is built in an AlmaLinux 8 container so that
    # the C++ objects and the bundled libraries also stay at that glibc,
    # whatever the host distribution is.
    if [ "$IN_LINUX_CONTAINER" = 0 ]; then
        need objdump "dnf install binutils"
        linux_qt
        local runtime
        runtime=$(container_runtime)
        linux_container_image "$runtime"
        local cw="$WORK/appimage"
        linux_container_run "$cw" "$ROOT/scripts/release.sh" --skip-tests app-linux-x86_64
        local appimage="$cw/work/assets/sdoe-$VERSION-linux-x86_64.AppImage"
        [ -f "$appimage" ] || die "the Linux container did not produce the AppImage"
        appimage_check_glibc "$appimage" "$APP_GLIBC"
        cp "$appimage" "$OUT/"
        return
    fi
    # The AppImage runs on the system Qt when that is as new or newer (see
    # packaging/linux/AppRun). Ahead-of-time compiled QML calls private Qt
    # API of one minor version, so qmlcachegen produces byte code only
    # (newer Qt versions compile the embedded QML sources instead).
    local qt_root qt_plugins tools="$WORK/qt-bytecode-tools" libexec
    qt_root=$(dirname "$(dirname "$QMAKE")")
    qt_plugins=$("$QMAKE" -query QT_INSTALL_PLUGINS)
    # Distributions and the official Qt installs name the Wayland platform
    # plugin differently (one libqwayland, or libqwayland-egl and
    # libqwayland-generic).
    local -a platform_plugins=()
    for p in libqwayland.so libqwayland-egl.so libqwayland-generic.so libqoffscreen.so; do
        [ -f "$qt_plugins/platforms/$p" ] && platform_plugins+=("$p")
    done
    libexec=$("$QMAKE" -query QT_HOST_LIBEXECS)
    mkdir -p "$tools"
    for t in "$libexec"/*; do ln -s "$t" "$tools/"; done
    rm "$tools/qmlcachegen"
    printf '#!/bin/sh\nexec "%s/qmlcachegen" --only-bytecode "$@"\n' "$libexec" > "$tools/qmlcachegen"
    chmod +x "$tools/qmlcachegen"
    cat > "$WORK/qmake-bytecode" <<QMAKE_EOF
#!/bin/sh
if [ "\$1" = -query ]; then
    case "\${2:-}" in
        QT_HOST_LIBEXECS|QT_HOST_LIBEXECS/get|QT_INSTALL_LIBEXECS|QT_INSTALL_LIBEXECS/get) echo "$tools"; exit ;;
    esac
fi
exec "$QMAKE" "\$@"
QMAKE_EOF
    chmod +x "$WORK/qmake-bytecode"
    QMAKE="$WORK/qmake-bytecode" cargo build --release -p sdoe-app
    if objdump -T "$CARGO_TARGET_DIR/release/sdoe" | grep -q PRIVATE_API; then
        die "the Linux application uses private Qt API; it would not run on newer system Qt versions"
    fi
    local ld ldqt
    ld=$(fetch_tool "linuxdeploy-$LINUXDEPLOY_TAG-x86_64.AppImage" \
        "https://github.com/linuxdeploy/linuxdeploy/releases/download/$LINUXDEPLOY_TAG/linuxdeploy-x86_64.AppImage")
    ldqt=$(fetch_tool "linuxdeploy-plugin-qt-$LINUXDEPLOY_QT_TAG-x86_64.AppImage" \
        "https://github.com/linuxdeploy/linuxdeploy-plugin-qt/releases/download/$LINUXDEPLOY_QT_TAG/linuxdeploy-plugin-qt-x86_64.AppImage")
    # The patchelf inside linuxdeploy (0.15), and 0.18 as well, corrupt
    # libraries with RELR relocations (Fedora 44 and other current
    # distributions); 0.19.3 handles them.
    local patchelf="$ROOT/build/tools/patchelf-$PATCHELF_VERSION"
    if [ ! -x "$patchelf" ]; then
        say "Downloading patchelf $PATCHELF_VERSION"
        local pe="$WORK/patchelf"
        mkdir -p "$pe"
        curl -fL --retry 3 "https://github.com/NixOS/patchelf/releases/download/$PATCHELF_VERSION/patchelf-$PATCHELF_VERSION-x86_64.tar.gz" |
            tar -xz -C "$pe" ./bin/patchelf
        mv "$pe/bin/patchelf" "$patchelf"
    fi
    # linuxdeploy finds its plugins on PATH under the name linuxdeploy-plugin-<name>.
    local bin="$WORK/linuxdeploy-bin"
    mkdir -p "$bin"
    ln -s "$ldqt" "$bin/linuxdeploy-plugin-qt"
    # linuxdeploy-plugin-qt deploys the Wayland shell and decoration plugins
    # but not the client buffer integrations; without wayland-egl, Qt Quick
    # finds no OpenGL context on Wayland and the application aborts.
    local gfx="$WORK/AppDir/usr/plugins/wayland-graphics-integration-client"
    mkdir -p "$gfx"
    cp "$qt_plugins"/wayland-graphics-integration-client/*.so "$gfx/"
    [ -f "$gfx/libqt-plugin-wayland-egl.so" ] || die "the Wayland EGL client plugin of Qt was not found"
    (
        cd "$WORK"
        # The Qt plugin of linuxdeploy looks up qmake and the Qt tools on PATH;
        # the Qt libraries and their dependencies must be resolvable for the
        # "ldd" that linuxdeploy runs on the executable.
        export PATH="$(dirname "$QMAKE"):$bin:$PATH"
        export LD_LIBRARY_PATH="$qt_root/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
        export APPIMAGE_EXTRACT_AND_RUN=1
        # The strip of linuxdeploy predates .relr.dyn sections; distribution
        # libraries come stripped already.
        export NO_STRIP=1
        export PATCHELF="$patchelf"
        export QMAKE
        export QML_SOURCES_PATHS="$ROOT/app/qml"
        EXTRA_PLATFORM_PLUGINS=$(IFS=';'; echo "${platform_plugins[*]}")
        export EXTRA_PLATFORM_PLUGINS
        export EXTRA_QT_MODULES="svg"
        export LINUXDEPLOY_OUTPUT_VERSION="$VERSION"
        export LDAI_OUTPUT="$OUT/sdoe-$VERSION-linux-x86_64.AppImage"
        "$ld" --appdir "$WORK/AppDir" \
            --executable "$CARGO_TARGET_DIR/release/sdoe" \
            --deploy-deps-only "$gfx" \
            --desktop-file "$ROOT/packaging/linux/sdoe.desktop" \
            --icon-file "$ROOT/packaging/linux/sdoe.svg" \
            --plugin qt --output appimage
    )
    [ -f "$OUT/sdoe-$VERSION-linux-x86_64.AppImage" ] || die "linuxdeploy did not produce the AppImage"
}

# Copies the MinGW DLLs that the executables and DLLs in $1 load, until
# nothing is missing. Windows system DLLs are not in the MinGW sysroot and
# stay out.
copy_mingw_dlls() {
    local dir=$1 added=1 dep src
    while [ "$added" = 1 ]; do
        added=0
        while IFS= read -r dep; do
            find "$dir" -maxdepth 1 -iname "$dep" | grep -q . && continue
            src=$(find "$MINGW_ROOT/bin" -maxdepth 1 -iname "$dep" -print -quit)
            if [ -n "$src" ]; then
                cp "$src" "$dir/"
                added=1
            fi
        done < <(find "$dir" -type f \( -iname '*.dll' -o -iname '*.exe' \) -print0 |
            xargs -0 x86_64-w64-mingw32-objdump -p | sed -n 's/^\s*DLL Name: //p' | sort -u)
    done
}

app_windows() {
    local triple=x86_64-pc-windows-gnu
    say "Application: Windows x86_64"
    rust_target "$triple"
    QMAKE="$MINGW_QMAKE" cargo build --release -p sdoe-app --target "$triple"
    local d="$WORK/sdoe-$VERSION-windows-x86_64" qt="$MINGW_ROOT/lib/qt6"
    mkdir -p "$d/plugins/platforms" "$d/qml"
    cp "$CARGO_TARGET_DIR/$triple/release/sdoe.exe" "$d/"
    cp "$qt/plugins/platforms/qwindows.dll" "$d/plugins/platforms/"
    local p
    for p in styles imageformats iconengines; do
        [ -d "$qt/plugins/$p" ] && cp -r "$qt/plugins/$p" "$d/plugins/"
    done
    for p in QtQuick QtQml QtCore QML Qt; do
        cp -r "$qt/qml/$p" "$d/qml/"
    done
    cat > "$d/qt.conf" <<'EOF'
[Paths]
Prefix = .
Plugins = plugins
QmlImports = qml
Qml2Imports = qml
EOF
    cp LICENSE "$d/LICENSE.txt"
    copy_mingw_dlls "$d"
    pack_zip "sdoe-$VERSION-windows-x86_64"
}

# Writes a qmake stand-in to $2 for cxx-qt: headers and libraries of the
# Qt in $1 (for another platform), host tools of the Linux Qt.
qmake_standin() {
    cat > "$2" <<QMAKE_EOF
#!/bin/sh
value() {
    case "\${1%/get}" in
        QT_INSTALL_PREFIX|QT_INSTALL_ARCHDATA|QT_INSTALL_DATA) echo "$1" ;;
        QT_INSTALL_HEADERS) echo "$1/include" ;;
        QT_INSTALL_LIBS) echo "$1/lib" ;;
        QT_INSTALL_PLUGINS) echo "$1/plugins" ;;
        QT_INSTALL_QML) echo "$1/qml" ;;
        QT_INSTALL_TRANSLATIONS) echo "$1/translations" ;;
        QT_INSTALL_LIBEXECS) "$QMAKE" -query QT_HOST_LIBEXECS ;;
        QT_INSTALL_BINS) "$QMAKE" -query QT_HOST_BINS ;;
        *) "$QMAKE" -query "\${1%/get}" ;;
    esac
}
if [ "\$1" = -query ] && [ -n "\${2:-}" ]; then value "\$2"; exit; fi
exec "$QMAKE" "\$@"
QMAKE_EOF
    chmod +x "$2"
}

# Downloads Qt $2 for the aqtinstall host $1 and architecture $3 into
# build/tools/$4 unless it is there.
aqt_install() {
    local venv="$ROOT/build/tools/aqt-venv"
    [ -x "$venv/bin/aqt" ] || { python3 -m venv "$venv" && "$venv/bin/pip" install -q aqtinstall; }
    # aqt writes its log to the current directory.
    (cd "$ROOT/build/tools" && "$venv/bin/aqt" install-qt "$1" desktop "$2" "$3" -m qtimageformats -O "$4")
}

# Qt version of an installation, from its qconfig.pri.
qt_version_of() { sed -n 's/^QT_VERSION *= *//p' "$1/mkspecs/qconfig.pri" | head -1; }

# The container runtime that builds the Linux application.
container_runtime() {
    if [ -n "${LINUX_CONTAINER:-}" ]; then
        command -v "$LINUX_CONTAINER" >/dev/null 2>&1 || die "$LINUX_CONTAINER not found"
        echo "$LINUX_CONTAINER"
    elif command -v podman >/dev/null 2>&1; then
        echo podman
    elif command -v docker >/dev/null 2>&1; then
        echo docker
    else
        die "podman or docker is required to build the Linux application"
    fi
}

# The official Qt for Linux of LINUX_QT_VERSION, whose binaries need glibc
# 2.28. Downloaded into build/tools like the Qt for macOS.
linux_qt() {
    if [ -z "${LINUX_QT:-}" ]; then
        LINUX_QT="$ROOT/build/tools/qt-linux/$LINUX_QT_VERSION/gcc_64"
        if [ ! -x "$LINUX_QT/bin/qmake" ]; then
            say "Downloading Qt $LINUX_QT_VERSION for Linux"
            aqt_install linux "$LINUX_QT_VERSION" linux_gcc_64 qt-linux
        fi
    fi
    [ -x "$LINUX_QT/bin/qmake" ] || die "no Qt for Linux in $LINUX_QT"
    local version
    version=$(qt_version_of "$LINUX_QT")
    [ "$version" = "$LINUX_QT_VERSION" ] || die "Qt for Linux is $version, not $LINUX_QT_VERSION"
}

# The image of a given Containerfile revision, so that a changed
# Containerfile is not hidden by an old image of the same name.
linux_image_name() {
    if [ -n "${LINUX_IMAGE:-}" ]; then
        echo "$LINUX_IMAGE"
    else
        echo "$LINUX_IMAGE_DEFAULT-$(sha256sum "$ROOT/packaging/linux/Containerfile" | cut -c1-12)"
    fi
}

# Builds the image from packaging/linux/Containerfile when it is missing.
linux_container_image() {
    local image
    image=$(linux_image_name)
    if ! "$1" image inspect "$image" >/dev/null 2>&1; then
        say "Building the Linux build image $image"
        "$1" build -t "$image" -f "$ROOT/packaging/linux/Containerfile" "$ROOT/packaging/linux"
    fi
}

# Runs the nested release script for the Linux application in the container.
# The checkout, the cargo home and the host Rust toolchain are mounted at the
# same paths, so the nested run needs no path translation, and the container
# cannot pick up host-glibc objects.
linux_container_run() {
    local cw=$1; shift
    local runtime cargo_home sysroot
    runtime=$(container_runtime)
    cargo_home=${CARGO_HOME:-$HOME/.cargo}
    sysroot=$(rustc --print sysroot)
    local -a opts=(
        --rm --security-opt label=disable
        -v "$ROOT:$ROOT"
        -v "$cargo_home:/cargo"
        -v "$sysroot:$sysroot:ro"
        -w "$ROOT"
        -e HOME=/tmp
        -e CARGO_HOME=/cargo
        -e PATH="$sysroot/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
        -e RUSTC="$sysroot/bin/rustc"
        -e CARGO_TARGET_DIR="$ROOT/target/linux-appimage"
        -e CARGO_PROFILE_RELEASE_STRIP=symbols
        -e CXX_QT_AUTORCC_OPTIONS=--compress-algo:zlib
        -e QMAKE="$LINUX_QT/bin/qmake"
        -e SDOE_LINUX_CONTAINER=1
        -e SDOE_SKIP_FINALIZE=1
        -e SDOE_WORK_DIR="$cw/work"
        -e GLIBC="$GLIBC"
        -e PATCHELF_VERSION="$PATCHELF_VERSION"
        -e LINUXDEPLOY_TAG="$LINUXDEPLOY_TAG"
        -e LINUXDEPLOY_QT_TAG="$LINUXDEPLOY_QT_TAG"
        -e SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-}"
    )
    case "$runtime" in
        podman) opts+=(--userns=keep-id) ;;
        *) opts+=(--user "$(id -u):$(id -g)") ;;
    esac
    # The nested run writes its assets here; create the directory on the host
    # so that the bind mount does not leave it root-owned.
    mkdir -p "$cw/work"
    "$runtime" run "${opts[@]}" "$(linux_image_name)" "$@"
}

# Qt for macOS of the same version as the host Qt, whose moc, rcc and QML
# tools generate the code.
macos_qt() {
    local version
    version=$("$QMAKE" -query QT_VERSION)
    if [ -z "${MACOS_QT:-}" ]; then
        MACOS_QT="$ROOT/build/tools/qt-macos/$version/macos"
        if [ ! -d "$MACOS_QT/lib/QtCore.framework" ]; then
            say "Downloading Qt $version for macOS"
            aqt_install mac "$version" clang_64 qt-macos
        fi
    fi
    [ -d "$MACOS_QT/lib/QtCore.framework" ] || die "no Qt for macOS in $MACOS_QT"
    local mac_version
    mac_version=$(qt_version_of "$MACOS_QT")
    [ "$mac_version" = "$version" ] || die "Qt for macOS is $mac_version, the host Qt is $version"
}

# Lists the Mach-O files of a bundle: executables, dylibs and framework binaries.
macos_binaries() {
    find "$1" -type f \( -path '*/MacOS/*' -o -name '*.dylib' -o -regex '.*/\([^/]*\)\.framework/Versions/A/\1' \) | LC_ALL=C sort
}

# Copies the Qt frameworks that the binaries in the bundle contents $1 load,
# until nothing is missing. Each framework keeps only Versions/A without the
# headers: dyld loads it by that path, and the bundle stays free of symlinks.
macos_frameworks() {
    local c=$1 added=1 name
    while [ "$added" = 1 ]; do
        added=0
        while IFS= read -r name; do
            [ -e "$c/lib/$name.framework" ] && continue
            [ -d "$MACOS_QT/lib/$name.framework/Versions/A" ] || die "Qt for macOS has no $name.framework"
            mkdir -p "$c/lib/$name.framework/Versions"
            tar -C "$MACOS_QT/lib/$name.framework/Versions" --exclude=A/Headers -cf - A |
                tar -C "$c/lib/$name.framework/Versions" -xf -
            added=1
        done < <(macos_binaries "$c" | xargs "$OTOOL" -L | sed -n 's|^\s*@rpath/\([^/]*\)\.framework/.*|\1|p' | sort -u)
    done
}

app_macos() {
    say "Application: macOS (universal)"
    macos_qt
    local mac="$WORK/macos" cpu clang slices=()
    mkdir -p "$mac"
    qmake_standin "$MACOS_QT" "$mac/qmake"
    for cpu in x86_64 aarch64; do
        rust_target "$cpu-apple-darwin"
        # cxx-qt adds an rpath to the Qt download; the linker wrapper drops
        # it, and the executable finds Qt in the bundle.
        clang=$(compgen -G "$OSXCROSS_BIN/$cpu-apple-darwin*-clang" | head -n1) || die "no $cpu-apple-darwin*-clang in $OSXCROSS_BIN"
        cat > "$mac/link-$cpu" <<LINK_EOF
#!/bin/sh
for a; do
    shift
    [ "\$a" = "-Wl,-rpath,$MACOS_QT/lib" ] || set -- "\$@" "\$a"
done
exec "$clang" "\$@"
LINK_EOF
        chmod +x "$mac/link-$cpu"
        macos_toolchain "$cpu" "$mac/link-$cpu"
        PATH="$OSXCROSS_BIN:$PATH" QMAKE="$mac/qmake" MACOSX_DEPLOYMENT_TARGET="$MACOSX_DEPLOYMENT_TARGET" \
            cargo rustc --release -p sdoe-app --bin sdoe --target "$cpu-apple-darwin" \
            -- -C link-arg=-Wl,-rpath,@executable_path/../lib
        slices+=("$CARGO_TARGET_DIR/$cpu-apple-darwin/release/sdoe")
    done
    OTOOL="${OSX_PREFIX}otool"

    # The bundle copies the layout of the Qt installation (lib, plugins,
    # qml), so that the relative rpaths in the Qt binaries resolve and these
    # stay unmodified, with their signatures.
    local d="$WORK/sdoe-$VERSION-macos" c
    c="$d/sdoe.app/Contents"
    mkdir -p "$c/MacOS" "$c/Resources" "$c/lib" "$c/plugins/platforms" "$c/qml"
    "$OSXCROSS_BIN/lipo" -create -output "$c/MacOS/sdoe" "${slices[@]}"
    sed "s/@VERSION@/$VERSION/g" packaging/macos/Info.plist > "$c/Info.plist"
    printf 'APPL????' > "$c/PkgInfo"
    cp packaging/macos/sdoe.icns "$c/Resources/"
    # Relative paths in qt.conf of a bundle start at Contents.
    printf '[Paths]\nPlugins = plugins\nQmlImports = qml\n' > "$c/Resources/qt.conf"
    local p
    cp "$MACOS_QT/plugins/platforms/libqcocoa.dylib" "$c/plugins/platforms/"
    for p in styles imageformats iconengines; do
        [ -d "$MACOS_QT/plugins/$p" ] && cp -r "$MACOS_QT/plugins/$p" "$c/plugins/"
    done
    for p in QtQuick QtQml QtCore QML Qt; do
        cp -r "$MACOS_QT/qml/$p" "$c/qml/"
    done
    macos_frameworks "$c"
    cp LICENSE "$d/LICENSE.txt"

    # Checks: every @rpath library is in Contents/lib, every other library
    # is part of macOS, there are no symlinks, and the signatures are valid.
    local f dep missing=0
    while IFS= read -r f; do
        while IFS= read -r dep; do
            case "$dep" in
                @rpath/*) [ -e "$c/lib/${dep#@rpath/}" ] || { echo "${f#"$c"/}: $dep is not in the bundle" >&2; missing=1; } ;;
                /System/*|/usr/lib/*) ;;
                *) echo "${f#"$c"/}: $dep is outside macOS" >&2; missing=1 ;;
            esac
        done < <("$OTOOL" -L "$f" | sed -n 's/^\s*\(.*\) (compatibility.*/\1/p' | grep -vx "@rpath/${f#"$c"/lib/}")
    done < <(macos_binaries "$c")
    [ "$missing" = 0 ] || die "the bundle is incomplete"
    [ -z "$(find "$d" -type l)" ] || die "the bundle contains symlinks"
    if "$OTOOL" -l "$c/MacOS/sdoe" | grep -qF "$MACOS_QT"; then die "the executable refers to $MACOS_QT"; fi
    mapfile -t slices < <(macos_binaries "$c")
    macos_check_signatures "${slices[@]}"
    pack_zip "sdoe-$VERSION-macos"
}

# The application for WebAssembly (Qt for WebAssembly, emscripten) and the
# sdoe-web server with it built in, for Linux, Windows and macOS.
web() {
    say "Application: WebAssembly"
    local version
    version=$("$QMAKE" -query QT_VERSION)
    if [ -z "${WASM_QT:-}" ]; then
        WASM_QT="$ROOT/build/tools/qt-wasm/$version/wasm_singlethread"
        if [ ! -f "$WASM_QT/lib/libQt6Core.a" ]; then
            say "Downloading Qt $version for WebAssembly"
            # aqtinstall names the WebAssembly host all_os.
            local venv="$ROOT/build/tools/aqt-venv"
            [ -x "$venv/bin/aqt" ] || { python3 -m venv "$venv" && "$venv/bin/pip" install -q aqtinstall; }
            (cd "$ROOT/build/tools" && "$venv/bin/aqt" install-qt all_os wasm "$version" wasm_singlethread -m qtimageformats -O qt-wasm)
        fi
    fi
    [ -f "$WASM_QT/lib/libQt6Core.a" ] || die "no Qt for WebAssembly in $WASM_QT"
    [ "$(qt_version_of "$WASM_QT")" = "$version" ] || die "Qt for WebAssembly is $(qt_version_of "$WASM_QT"), the host Qt is $version"
    local want_emcc have_emcc
    want_emcc=$(sed -n 's/^QT_EMCC_VERSION *= *//p' "$WASM_QT/mkspecs/qconfig.pri" | head -1)
    have_emcc=$("$EMCC_DIR/emcc" --version | sed -n '1s/.* \([0-9][0-9.]*\) (.*/\1/p')
    [ "$have_emcc" = "$want_emcc" ] || die "Qt $version for WebAssembly needs emscripten $want_emcc, emcc is $have_emcc"

    local w="$WORK/web" app="$WORK/web/app"
    mkdir -p "$w" "$app"
    qmake_standin "$WASM_QT" "$w/qmake"
    rust_target wasm32-unknown-emscripten
    # Rust code panics with abort here (see app/build.rs on exceptions).
    PATH="$EMCC_DIR:$PATH" QMAKE="$w/qmake" CARGO_PROFILE_RELEASE_PANIC=abort \
        cargo build --release -p sdoe-app --target wasm32-unknown-emscripten
    local rel="$CARGO_TARGET_DIR/wasm32-unknown-emscripten/release"
    cp "$rel/sdoe.js" "$rel/sdoe.wasm" "$WASM_QT/plugins/platforms/qtloader.js" "$app/"

    say "Web server: Linux, Windows, macOS"
    export SDOE_WEB_APP="$app"
    local arch
    for arch in x86_64 aarch64; do
        rust_target "$arch-unknown-linux-musl"
        cargo zigbuild --release -p sdoe-web --target "$arch-unknown-linux-musl"
        cp "$CARGO_TARGET_DIR/$arch-unknown-linux-musl/release/sdoe-web" "$OUT/sdoe-web-$VERSION-linux-$arch"
    done
    rust_target x86_64-pc-windows-gnu
    cargo build --release -p sdoe-web --target x86_64-pc-windows-gnu
    local exe="$OUT/sdoe-web-$VERSION-windows-x86_64.exe" dep
    cp "$CARGO_TARGET_DIR/x86_64-pc-windows-gnu/release/sdoe-web.exe" "$exe"
    for dep in $(x86_64-w64-mingw32-objdump -p "$exe" | sed -n 's/^\s*DLL Name: //p'); do
        if find "$MINGW_ROOT/bin" -maxdepth 1 -iname "$dep" | grep -q .; then
            die "sdoe-web.exe depends on $dep, which Windows does not provide"
        fi
    done
    local cpu slices=()
    for cpu in x86_64 aarch64; do
        rust_target "$cpu-apple-darwin"
        macos_toolchain "$cpu"
        PATH="$OSXCROSS_BIN:$PATH" MACOSX_DEPLOYMENT_TARGET="$MACOSX_DEPLOYMENT_TARGET" \
            cargo build --release -p sdoe-web --target "$cpu-apple-darwin"
        slices+=("$CARGO_TARGET_DIR/$cpu-apple-darwin/release/sdoe-web")
    done
    "$OSXCROSS_BIN/lipo" -create -output "$OUT/sdoe-web-$VERSION-macos" "${slices[@]}"
    macos_check_signatures "$OUT/sdoe-web-$VERSION-macos"
    unset SDOE_WEB_APP
}

# ---------------------------------------------------------------------------

wants lib-linux-x86_64 && lib_linux x86_64
wants lib-linux-aarch64 && lib_linux aarch64
wants lib-windows-x86_64 && lib_windows
wants lib-macos && lib_macos
wants lib-android && lib_android
wants lib-wasm && lib_wasm
wants app-linux-x86_64 && app_linux
wants app-windows-x86_64 && app_windows
wants app-macos && app_macos
wants web && web

# The nested container run only builds its step; the outer run finalizes.
if [ "$SKIP_FINALIZE" = 1 ]; then
    exit 0
fi

say "Checksums and release notes"
NEW=()
for f in "$OUT"/*; do
    NEW+=("$(basename "$f")")
    [ ! -e "$DIST/$(basename "$f")" ] || die "$DIST/$(basename "$f") already exists"
done
if [ "$ADD" = 1 ]; then
    for f in "${NEW[@]}"; do mv "$OUT/$f" "$DIST/"; done
else
    mv "$OUT" "$DIST"
fi
(cd "$DIST" && find . -maxdepth 1 -type f ! -name SHA256SUMS -printf '%f\0' | LC_ALL=C sort -z | xargs -0 sha256sum --) > "$WORK/SHA256SUMS"
mv "$WORK/SHA256SUMS" "$DIST/SHA256SUMS"

last_tag=$(git describe --tags --abbrev=0 2>/dev/null || true)
{
    echo "## sdoe $VERSION"
    echo
    if [ -n "$last_tag" ]; then
        echo "Changes since $last_tag:"
        echo
        git log --format='- %s' "$last_tag..HEAD"
    else
        echo "First release."
    fi
    echo
    echo "### Downloads"
    echo
    echo "| File | Contents |"
    echo "|---|---|"
    for f in "$DIST"/*; do
        name=$(basename "$f")
        case "$name" in
            sdoe-web-*linux-x86_64) what="Web server with the application for the browser: Linux x86_64 (static)" ;;
            sdoe-web-*linux-aarch64) what="Web server with the application for the browser: Linux ARM64 (static)" ;;
            sdoe-web-*windows*) what="Web server with the application for the browser: Windows x86_64" ;;
            sdoe-web-*macos) what="Web server with the application for the browser: macOS $MACOSX_DEPLOYMENT_TARGET or newer, universal" ;;
            *.AppImage) what="Desktop application for Linux x86_64, glibc $APP_GLIBC or newer (make it executable and run it)" ;;
            sdoe-*windows*) what="Desktop application for Windows x86_64 (unzip and run sdoe.exe)" ;;
            sdoe-*macos*) what="Desktop application for macOS 13 or newer, Apple silicon and Intel (see below)" ;;
            libsdoe-*macos*) what="Dynamic library for macOS $MACOSX_DEPLOYMENT_TARGET or newer, universal (arm64 and x86_64)" ;;
            libsdoe-*linux-x86_64*) what="Shared library for Linux x86_64, glibc $GLIBC or newer" ;;
            libsdoe-*linux-aarch64*) what="Shared library for Linux ARM64, glibc $GLIBC or newer" ;;
            libsdoe-*windows*) what="DLL for Windows x86_64 with MinGW (libsdoe.dll.a) and MSVC (sdoe.lib) import libraries" ;;
            libsdoe-*android*) what="Shared libraries for Android API $ANDROID_API+ (${ANDROID_ABIS[*]})" ;;
            libsdoe-*wasm*) what="WebAssembly module with the JavaScript interface \`sdoe.js\` and TypeScript definitions (browsers, Node.js 18+, Deno)" ;;
            SHA256SUMS) what="SHA-256 checksums" ;;
            *) what="" ;;
        esac
        echo "| \`$name\` | $what |"
    done
    echo
    echo "The native library archives contain the library, the C header \`include/sdoe.h\`, \`API.md\` and \`LICENSE\`."
    echo "The WebAssembly archive contains \`sdoe.wasm\`, \`sdoe.js\`, \`sdoe.d.ts\`, \`package.json\`, \`API.md\` and \`LICENSE\`."
    if compgen -G "$DIST/sdoe-web-*" >/dev/null; then
        echo
        echo "\`sdoe-web\` is a single executable that serves the application to browsers; the application runs in the browser. Start it and open http://127.0.0.1:8080/ (\`--listen ADDRESS:PORT\` chooses another address, \`--open\` opens the browser)."
    fi
    if [ -e "$DIST/sdoe-$VERSION-macos.zip" ]; then
        echo
        echo "The macOS application is not signed with an Apple Developer ID, so macOS blocks its first start. After moving \`sdoe.app\` to Applications, either allow it under System Settings → Privacy & Security (\"Open Anyway\"), or run \`xattr -dr com.apple.quarantine /Applications/sdoe.app\`."
    fi
} > "$WORK/notes.md"

mv "$WORK/notes.md" "$NOTES"

say "Done"
ls -l "$DIST"
echo
echo "Release notes: $NOTES"
rel=${DIST#$ROOT/}
if [ "$ADD" = 1 ]; then
    echo "Add to the release with:"
    echo "  gh release upload v$VERSION$(printf " $rel/%s" "${NEW[@]}") $rel/SHA256SUMS --clobber"
    echo "  gh release edit v$VERSION --notes-file \"${NOTES#$ROOT/}\""
else
    echo "Publish with:"
    echo "  gh release create v$VERSION --draft --title \"sdoe $VERSION\" --notes-file \"${NOTES#$ROOT/}\" $rel/*"
fi
