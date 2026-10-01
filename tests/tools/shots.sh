#!/usr/bin/env bash
# Regenera as imagens de docs/img a partir do próprio binário (hub, guia, seletor de papel de parede, fetch,
# prancha de papéis de parede e a marca). Usa uma home descartável, então não toca no seu ~.
#
#   tests/tools/shots.sh [--font /caminho/GeistMono-Regular.ttf] [--bold /caminho/GeistMono-Bold.ttf]
#
# Requer: cargo, python3 com playwright (chromium) e pillow.
set -euo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FONT_ARGS=("$@")
OUT="$REPO/docs/img"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

cargo build --release --quiet --manifest-path "$REPO/Cargo.toml"
CLIOS=("$REPO/target/release/clios" --root "$REPO")
png() { python3 "$REPO/tests/tools/ansi2png.py" "$1" "$2" "${FONT_ARGS[@]}" >/dev/null; echo "  $(basename "$2")"; }

# hub: escuro (ember) e claro (azure)
H="$TMP/dark"
"${CLIOS[@]}" --home "$H" theme set --mode dark --accent ember >/dev/null
"${CLIOS[@]}" --home "$H" hub --demo --snapshot 84x24 > "$TMP/a.ansi";                      png "$TMP/a.ansi" "$OUT/hub.png"
"${CLIOS[@]}" --home "$H" hub --demo --snapshot 84x24 --query "ar" > "$TMP/a.ansi";         png "$TMP/a.ansi" "$OUT/hub-search.png"
"${CLIOS[@]}" --home "$H" hub --demo --snapshot 84x24 --query ">acento" > "$TMP/a.ansi";    png "$TMP/a.ansi" "$OUT/hub-actions.png"
"${CLIOS[@]}" --home "$H" hub --demo --snapshot 84x24 --query "+mus" > "$TMP/a.ansi";          png "$TMP/a.ansi" "$OUT/hub-install.png"
L="$TMP/light"
"${CLIOS[@]}" --home "$L" theme set --mode light --accent azure >/dev/null
"${CLIOS[@]}" --home "$L" hub --demo --snapshot 84x24 > "$TMP/a.ansi";                      png "$TMP/a.ansi" "$OUT/hub-light.png"

# guia de boas-vindas
for page in inicio atalhos apps sistema dicas sobre; do
  "${CLIOS[@]}" --home "$H" welcome --page "$page" --demo --snapshot 104x32 $([ "$page" = dicas ] && echo "--select 4") > "$TMP/a.ansi"
  png "$TMP/a.ansi" "$OUT/welcome-$page.png"
done

# seletor de papel de parede
CLIOS_WALLPAPER_SIZE=960x540 "${CLIOS[@]}" --home "$H" wallpaper --snapshot 100x30 --select aneis > "$TMP/a.ansi"
png "$TMP/a.ansi" "$OUT/wallpaper-picker.png"

# a prancha dos oito estilos
python3 "$REPO/tests/tools/wallpaper_sheet.py" "$REPO/target/release/clios" "$REPO" "$OUT/wallpapers.png"
# a shell (barra, OSD, notificações): vem do harness de QML, se o PySide6 estiver instalado
if python3 -c "import PySide6" 2>/dev/null; then
  FONT_DIR=""
  for a in "${FONT_ARGS[@]}"; do [[ "$a" == *.ttf ]] && FONT_DIR="$(dirname "$a")"; done
  python3 "$REPO/tests/qml/render.py" --out "$TMP/qml" ${FONT_DIR:+--font "$FONT_DIR"} >/dev/null
  cp "$TMP/qml/shell-dark-ember-full.png" "$OUT/shell-dark.png"
  cp "$TMP/qml/shell-light-azure-full.png" "$OUT/shell-light.png"
  echo "  shell-dark.png, shell-light.png"
fi
echo "pronto: $OUT"
