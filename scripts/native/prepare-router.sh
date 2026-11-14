#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/../.."
root="$PWD"
output="$root/target/native-router"
sources="$output/SOURCES"
mkdir -p "$sources"

fetch() {
    local url="$1" destination="$2" checksum="$3"
    if [ -f "$destination" ] && [ "$(shasum -a 256 "$destination" | cut -d ' ' -f 1)" = "$checksum" ]; then return; fi
    curl --fail --location --retry 3 "$url" -o "$destination"
    printf '%s  %s\n' "$checksum" "$destination" | shasum -a 256 --check
}
fetch 'https://codeload.github.com/TypeFox/libavoid-server/tar.gz/refs/tags/v0.3.1' \
    "$sources/libavoid-server-v0.3.1.tar.gz" f34567f66d67bdb58f1557c45b89f5247eec9008492000aa2fba30b20ef1192f
fetch 'https://codeload.github.com/mjwybrow/adaptagrams/tar.gz/fcac991f9f7d959e8326ae0567ce122839fbd4be' \
    "$sources/adaptagrams-fcac991.tar.gz" 3ea743821e2caa3bf5e6d7a700368ee84547ce8befe8c3a5e61377240c1a81f6
tar -xOf "$sources/libavoid-server-v0.3.1.tar.gz" libavoid-server-0.3.1/LICENSE.md > "$output/LICENSE-libavoid-server.txt"
tar -xOf "$sources/adaptagrams-fcac991.tar.gz" adaptagrams-fcac991f9f7d959e8326ae0567ce122839fbd4be/cola/LICENSE > "$output/LICENSE-libavoid.txt"
cp docs/ROUTER-NOTICES.txt "$output/"
cp scripts/native/prepare-router.{ps1,sh} "$sources/"

if [ "$(uname -s)" != Darwin ]; then
    echo 'This build script targets macOS. Other hosts can set FACTORY_LIBAVOID_SERVER to a compatible upstream v0.3.1 binary.' >&2
    exit 1
fi
if [ -x "$output/libavoid-server" ]; then
    codesign --force --sign - "$output/libavoid-server"
    exit 0
fi
# Requires Xcode Command Line Tools and `brew install automake libtool pkg-config`.
# Compile both architectures so the router also works inside the universal app bundle.
for architecture in arm64 x86_64; do
    work="$output/build-$architecture"
    mkdir -p "$work"
    tar -xzf "$sources/libavoid-server-v0.3.1.tar.gz" -C "$work"
    tar -xzf "$sources/adaptagrams-fcac991.tar.gz" -C "$work"
    cola="$work/adaptagrams-fcac991f9f7d959e8326ae0567ce122839fbd4be/cola"
    server="$work/libavoid-server-0.3.1"
    host="$architecture-apple-darwin"
    if [ "$architecture" = arm64 ]; then host=aarch64-apple-darwin; fi
    (
        cd "$cola"
        mkdir -p m4
        autoreconf --force --install
        ./configure --disable-shared --host="$host" \
            CC="clang -arch $architecture" CXX="clang++ -arch $architecture" \
            CFLAGS='-O2 -mmacosx-version-min=11.0' CXXFLAGS='-O2 -mmacosx-version-min=11.0'
        make -C libavoid -j2
    )
    clang++ -std=gnu++11 -O2 -arch "$architecture" -mmacosx-version-min=11.0 \
        -I"$cola" -I"$server/include" "$server"/src/*.cpp \
        "$cola/libavoid/.libs/libavoid.a" -o "$output/libavoid-server-$architecture"
done
lipo -create "$output/libavoid-server-arm64" "$output/libavoid-server-x86_64" -output "$output/libavoid-server"
chmod +x "$output/libavoid-server"
# The signed bytes are embedded in the app and remain signed when materialized in its cache.
codesign --force --sign - "$output/libavoid-server"
