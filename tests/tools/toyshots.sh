#!/usr/bin/env bash
# Captura os brinquedos de verdade (num pty, com a paleta do CLIOS e o acento escolhido) para site/img/toys.
# Cada um roda pelo `clios play`, então a cor vem do mesmo caminho que o usuário usa.
#
#   tests/tools/toyshots.sh [--font Regular.ttf] [--bold Bold.ttf]
#
# Requer: pyte, playwright, os brinquedos instalados (os que faltarem são pulados) e o `clios` compilado.
set -euo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FONT_ARGS=("$@")
OUT="$REPO/site/img/toys"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
mkdir -p "$OUT"

cargo build --quiet --manifest-path "$REPO/Cargo.toml"
CLIOS=("$REPO/target/debug/clios" --root "$REPO")
H="$TMP/home"
"${CLIOS[@]}" --home "$H" theme set --mode dark --accent ember >/dev/null
COLORS="$H/.config/foot/colors.ini"

# id:segundos de espera (o bonsai precisa de tempo para crescer)
for spec in bonsai:9 aquarium:4 matrix-rain:4 lava:5 sakura:6 pipes:6 clock:2; do
  id="${spec%%:*}" secs="${spec##*:}"
  if ! "${CLIOS[@]}" --home "$H" play --list | grep -E "^$id " | grep -qv "clios apps install"; then
    echo "  pulado: $id (não instalado)"
    continue
  fi
  HOME="$H" python3 "$REPO/tests/tools/termshot.py" --colors "$COLORS" --cols 72 --rows 24 --seconds "$secs" \
    --out "$TMP/$id.ansi" -- "$REPO/target/debug/clios" --root "$REPO" --home "$H" play "$id"
  python3 "$REPO/tests/tools/ansi2png.py" "$TMP/$id.ansi" "$OUT/$id.png" "${FONT_ARGS[@]}" >/dev/null
  echo "  $id.png"
done
python3 "$REPO/tests/tools/fetch_demo.py" "$H/.config/fastfetch/config.jsonc" "$TMP/fetch-demo.jsonc"
HOME="$H" python3 "$REPO/tests/tools/termshot.py" --colors "$COLORS" --cols 80 --rows 19 --seconds 1.5 --out "$TMP/fetch.ansi" -- \
  fastfetch -c "$TMP/fetch-demo.jsonc"
python3 "$REPO/tests/tools/ansi2png.py" "$TMP/fetch.ansi" "$REPO/site/img/fetch.png" "${FONT_ARGS[@]}" >/dev/null
echo "  fetch.png"
