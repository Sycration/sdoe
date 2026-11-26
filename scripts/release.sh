#!/usr/bin/env bash
# Builds the sdoe release assets into dist/v<version>/:
#
#   sdoe-<v>-linux-x86_64.AppImage     desktop application, Qt bundled
#   sdoe-<v>-windows-x86_64.zip        desktop application, Qt bundled
#   libsdoe-<v>-linux-x86_64.tar.gz    shared library (glibc 2.28 or newer)
#   libsdoe-<v>-linux-aarch64.tar.gz   shared library (glibc 2.28 or newer)
#   libsdoe-<v>-windows-x86_64.zip     DLL with MinGW and MSVC import libraries
#   libsdoe-<v>-android.zip            jniLibs/<abi>/libsdoe.so for 4 ABIs
#   libsdoe-<v>-wasm.zip               WebAssembly module with the JavaScript interface
#   SHA256SUMS
#
# and draft release notes into dist/v<version>-notes.md.
#
# Usage: scripts/release.sh [--skip-tests] [--add] [step...]
# Steps (default: all): lib-linux-x86_64 lib-linux-aarch64 lib-windows-x86_64
#                       lib-android lib-wasm app-linux-x86_64 app-windows-x86_64
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
#   app-linux-*:   qt6-qtbase-devel, qt6-qtdeclarative-devel, curl (downloads
#                  linuxdeploy into build/tools on first use)
#   app-windows-*: mingw64-gcc-c++, mingw64-qt6-qtbase, mingw64-qt6-qtdeclarative,
#                  mingw64-qt6-qtsvg, mingw64-qt6-qtimageformats
#
# Settings (environment):
#   ANDROID_NDK_HOME  NDK directory (default: newest in ~/Android/ndk or ~/Android/Sdk/ndk)
#   ANDROID_API       minimum Android API level (default 21)
#   GLIBC             minimum glibc of the Linux libraries (default 2.28)
#   QMAKE             qmake of the Linux Qt (default qmake6)
#   MINGW_QMAKE       qmake of the MinGW Qt (default x86_64-w64-mingw32-qmake-qt6)
#   MINGW_ROOT        MinGW sysroot (default /usr/x86_64-w64-mingw32/sys-root/mingw)
#   LINUXDEPLOY_TAG, LINUXDEPLOY_QT_TAG  release tags of the linuxdeploy tools
#   PATCHELF_VERSION  patchelf release used by linuxdeploy (default 0.19.3)

set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$ROOT"

ALL_STEPS=(lib-linux-x86_64 lib-linux-aarch64 lib-windows-x86_64 lib-android lib-wasm app-linux-x86_64 app-windows-x86_64)
ANDROID_ABIS=(arm64-v8a armeabi-v7a x86_64 x86)
ANDROID_RUST_TARGETS=(aarch64-linux-android armv7-linux-androideabi x86_64-linux-android i686-linux-android)

ANDROID_API=${ANDROID_API:-21}
GLIBC=${GLIBC:-2.28}
QMAKE=${QMAKE:-qmake6}
MINGW_QMAKE=${MINGW_QMAKE:-x86_64-w64-mingw32-qmake-qt6}
MINGW_ROOT=${MINGW_ROOT:-/usr/x86_64-w64-mingw32/sys-root/mingw}
LINUXDEPLOY_TAG=${LINUXDEPLOY_TAG:-1-alpha-20251107-1}
LINUXDEPLOY_QT_TAG=${LINUXDEPLOY_QT_TAG:-continuous}
PATCHELF_VERSION=${PATCHELF_VERSION:-0.19.3}

# Release builds use their own target directory, stripped binaries, and zlib
# instead of zstd for Qt resources (the MinGW Qt reads only zlib).
export CARGO_TARGET_DIR="$ROOT/target/dist"
export CARGO_PROFILE_RELEASE_STRIP=symbols
export CXX_QT_AUTORCC_OPTIONS="--compress-algo:zlib"

say() { printf '\n\033[1m==> %s\033[0m\n' "$*" >&2; }
die() { printf '\033[31merror:\033[0m %s\n' "$*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || die "$1 not found${2:+ ($2)}"; }

usage() { sed -n '2,/^$/p' "$0" | sed 's/^# \{0,1\}//'; exit "${1:-0}"; }

SKIP_TESTS=0
ADD=0
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
if [ "$ADD" = 1 ]; then
    [ -d "$DIST" ] || die "--add needs an existing $DIST"
else
    [ ! -e "$DIST" ] || die "$DIST already exists; remove it, change the version or use --add"
fi

need cargo
need rustup
need zip
need tar
need sha256sum
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
if wants app-linux-x86_64; then
    need "$QMAKE" "dnf install qt6-qtbase-devel qt6-qtdeclarative-devel"
    need curl
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
WORK=$(mktemp -d "$ROOT/build/release-$VERSION.XXXXXX")
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

app_linux() {
    say "Application: Linux x86_64 (AppImage)"
    QMAKE="$QMAKE" cargo build --release -p sdoe-app
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
    (
        cd "$WORK"
        export PATH="$bin:$PATH"
        export APPIMAGE_EXTRACT_AND_RUN=1
        # The strip of linuxdeploy predates .relr.dyn sections; distribution
        # libraries come stripped already.
        export NO_STRIP=1
        export PATCHELF="$patchelf"
        export QMAKE
        export QML_SOURCES_PATHS="$ROOT/app/qml"
        export EXTRA_PLATFORM_PLUGINS="libqwayland.so;libqoffscreen.so"
        export EXTRA_QT_MODULES="svg"
        export LINUXDEPLOY_OUTPUT_VERSION="$VERSION"
        export LDAI_OUTPUT="$OUT/sdoe-$VERSION-linux-x86_64.AppImage"
        "$ld" --appdir "$WORK/AppDir" \
            --executable "$CARGO_TARGET_DIR/release/sdoe" \
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

# ---------------------------------------------------------------------------

wants lib-linux-x86_64 && lib_linux x86_64
wants lib-linux-aarch64 && lib_linux aarch64
wants lib-windows-x86_64 && lib_windows
wants lib-android && lib_android
wants lib-wasm && lib_wasm
wants app-linux-x86_64 && app_linux
wants app-windows-x86_64 && app_windows

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
            *.AppImage) what="Desktop application for Linux x86_64 (make it executable and run it)" ;;
            sdoe-*windows*) what="Desktop application for Windows x86_64 (unzip and run sdoe.exe)" ;;
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
