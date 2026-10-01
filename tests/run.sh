#!/usr/bin/env bash
# Roda toda a bateria. Etapas que dependem de ferramentas ausentes são puladas, com aviso.
#
#   pip install -r tests/requirements.txt     (lupa, PySide6, shellcheck-py, ...)
#   ./tests/run.sh
#
# Variáveis: PYTHON (padrão python3), GEIST_MONO_DIR (pasta com GeistMono-*.ttf, para os PNGs do QML).
set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1
PY="${PYTHON:-python3}"
failed=()
skipped=()

step() {
  local name="$1"
  shift
  printf '\n\033[1m▌ %s\033[0m\n' "$name"
  if "$@"; then :; else failed+=("$name"); fi
}
skip() { printf '\n\033[1m▌ %s\033[0m\n  \033[33mpulado: %s\033[0m\n' "$1" "$2"; skipped+=("$1"); }
have_py() { "$PY" -c "import $1" 2>/dev/null; }

step "rustfmt" cargo fmt --all -- --check
step "clippy" cargo clippy --workspace --all-targets -- -D warnings
step "cargo test" cargo test --workspace
step "cargo build (debug, para os harnesses)" cargo build -q -p clios

if have_py lupa; then
  step "hyprland: o validador reprova configs ruins" "$PY" tests/hypr/test_harness.py
  step "hyprland: config Lua contra o stub oficial" "$PY" tests/hypr/check_config.py
else
  skip "hyprland (Lua)" "pip install lupa"
fi

if have_py PySide6; then
  step "quickshell: componentes renderizam sem avisos" "$PY" tests/qml/render.py
else
  skip "quickshell (QML offscreen)" "pip install PySide6-Essentials (e libEGL no sistema)"
fi

if command -v shellcheck >/dev/null; then
  step "shellcheck" shellcheck scripts/bootstrap.sh iso/build.sh iso/airootfs/usr/local/bin/clios-live-setup tests/run.sh
else
  skip "shellcheck" "pip install shellcheck-py"
fi

if command -v fish >/dev/null; then
  render_dir="$(mktemp -d)"
  # As aspas simples são de propósito: o $1 pertence ao `bash -c`, não a este script.
  # shellcheck disable=SC2016
  step "fish: sintaxe (estática e gerada)" bash -c '
    set -e
    ./target/debug/clios --root . --home "$1" sync --copy >/dev/null
    for f in config/fish/config.fish config/fish/functions/*.fish "$1"/.config/fish/conf.d/clios-colors.fish; do fish -n "$f"; done' _ "$render_dir"
  rm -rf "$render_dir"
else
  skip "fish" "instale o fish"
fi

printf '\n\033[1m────────────────────────────────\033[0m\n'
((${#skipped[@]})) && printf '\033[33mpuladas: %s\033[0m\n' "${skipped[*]}"
if ((${#failed[@]})); then
  printf '\033[31mfalharam: %s\033[0m\n' "${failed[*]}"
  exit 1
fi
printf '\033[32mtudo verde\033[0m\n'
