#!/usr/bin/env bash
# Regenera hl.meta.lua a partir do código do Hyprland.
# uso: tests/hypr/update-stub.sh [caminho-do-checkout-do-Hyprland]
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
src="${1:-}"
if [[ -z "$src" ]]; then
  src="$(mktemp -d)"
  git clone --depth 1 https://github.com/hyprwm/Hyprland.git "$src"
fi
tmp="$(mktemp)"
python3 "$src/meta/generateLuaStubs.py" --root "$src" --output "$tmp"
{
  printf -- '-- Stub oficial da API Lua do Hyprland (gerado de meta/generateLuaStubs.py, Hyprland %s).\n' "$(cat "$src/VERSION")"
  printf -- '-- Hyprland é BSD-3-Clause (c) vaxry e colaboradores. Regenerar: tests/hypr/update-stub.sh\n'
  cat "$tmp"
} > "$here/hl.meta.lua"
echo "atualizado: $here/hl.meta.lua"
