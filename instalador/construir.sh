#!/bin/bash
# Arma AIcontrol.app, el zip GOBMAC01 y el pkg. Se corre en un Mac.
set -euo pipefail
raiz="$(cd "$(dirname "$0")/.." && pwd)"
salida="${SALIDA:-$raiz/dist}"
if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "El .app y el pkg se arman en un Mac. Aquí se puede probar el daemon con: cargo test --manifest-path \"$raiz/daemon/Cargo.toml\""
  exit 1
fi
bin=""
while IFS= read -r linea; do
  case "$linea" in
    *'"executable"'*)
      bin="$(printf '%s' "$linea" | sed -n 's/.*"executable":"\([^"]*\)".*/\1/p')"
      ;;
  esac
done < <(cargo build --release --manifest-path "$raiz/daemon/Cargo.toml" --message-format=json)
if [[ -z "$bin" || ! -f "$bin" ]]; then
  echo "no se compiló el daemon"
  exit 1
fi
rm -rf "$salida/pkgroot"
mkdir -p "$salida/pkgroot/Applications/AIcontrol.app/Contents/MacOS" "$salida/pkgroot/Applications/AIcontrol.app/Contents/Resources"
app="$salida/pkgroot/Applications/AIcontrol.app"
swiftc -O -framework AppKit -framework Foundation -o "$app/Contents/MacOS/AIcontrol" "$raiz/app/Sources/AIcontrol/main.swift"
cp "$bin" "$app/Contents/MacOS/AIcontrol-daemon"
cp "$raiz/app/Info.plist" "$app/Contents/Info.plist"
version=$(sed -n 's/^version = "\([^"]*\)".*/\1/p' "$raiz/daemon/Cargo.toml" | head -1)
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $version" -c "Set :CFBundleVersion $version" "$app/Contents/Info.plist"
cp "$raiz/instalador/local.itsec.aicontrol.plist" "$app/Contents/Resources/local.itsec.aicontrol.plist"
cp "$raiz/instalador/local.itsec.aicontrol.menu.plist" "$app/Contents/Resources/local.itsec.aicontrol.menu.plist"
cp "$raiz/app/Resources/logo.bgra" "$app/Contents/Resources/logo.bgra"
chmod 755 "$app/Contents/MacOS/AIcontrol" "$app/Contents/MacOS/AIcontrol-daemon" "$raiz/instalador/scripts/preinstall" "$raiz/instalador/scripts/postinstall"
"$app/Contents/MacOS/AIcontrol-daemon" empaquetar --dir "$app" --salida "$salida/AIcontrol-actualizacion.zip"
pkgbuild --root "$salida/pkgroot" --identifier local.itsec.aicontrol --version "$version" --install-location / --scripts "$raiz/instalador/scripts" "$salida/AIcontrol.pkg"
if [[ -n "${SIGN_IDENTITY:-}" ]]; then
  codesign --force --deep --sign "$SIGN_IDENTITY" "$app"
  productsign --sign "$SIGN_IDENTITY" "$salida/AIcontrol.pkg" "$salida/AIcontrol-firmado.pkg"
fi
echo "$salida"
