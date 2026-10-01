#!/usr/bin/env bash
# CLIOS · bootstrap
#
# Transforma uma instalação mínima do Arch Linux (archinstall, perfil "minimal") em CLIOS.
# Idempotente: pode rodar de novo depois de atualizar o repositório.
#
#   ./scripts/bootstrap.sh              instala tudo
#   ./scripts/bootstrap.sh --dry-run    mostra o que faria, sem mudar nada
#
# Opções:
#   --no-aur           não instala os pacotes do AUR
#   --no-system        não mexe em /etc nem em serviços (só pacotes e dotfiles)
#   --kernel-cmdline   acrescenta quiet + paleta do console às entradas do systemd-boot (com backup)
#   --dry-run          não executa nada, só imprime
#   -h, --help
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DRY=0 AUR=1 SYSTEM=1 CMDLINE=0

usage() { sed -n '2,/^set -euo/p' "${BASH_SOURCE[0]}" | sed '$d; s/^# \{0,1\}//'; }

for arg in "$@"; do
  case "$arg" in
    --dry-run) DRY=1 ;;
    --no-aur) AUR=0 ;;
    --no-system) SYSTEM=0 ;;
    --kernel-cmdline) CMDLINE=1 ;;
    -h | --help) usage; exit 0 ;;
    *) echo "opção desconhecida: $arg" >&2; usage >&2; exit 2 ;;
  esac
done

say() { printf '\033[1m▌ %s\033[0m\n' "$*"; }
note() { printf '  %s\n' "$*"; }
warn() { printf '\033[33m  aviso: %s\033[0m\n' "$*" >&2; }
die() { printf '\033[31merro: %s\033[0m\n' "$*" >&2; exit 1; }

# Executa, ou só imprime no --dry-run.
run() {
  if ((DRY)); then printf '  \033[2m$ %s\033[0m\n' "$*"; else "$@"; fi
}

SUDO=""
if [[ $EUID -ne 0 ]]; then SUDO="sudo"; fi

# ── checagens ─────────────────────────────────────────────────────────────
if [[ ! -f /etc/arch-release ]]; then
  if ((DRY)); then
    warn "isto não é o Arch Linux; seguindo só porque é --dry-run."
  else
    die "o bootstrap é para o Arch Linux (não achei /etc/arch-release)."
  fi
fi
if [[ $EUID -eq 0 && -z "${SUDO_USER:-}" ]] && ((! DRY)); then
  die "rode como o seu usuário (o script chama sudo quando precisa), não como root puro."
fi
TARGET_USER="${SUDO_USER:-$(id -un)}"

# ── pacotes ───────────────────────────────────────────────────────────────
read_list() { grep -vE '^\s*(#|$)' "$1" | sed 's/\s*#.*//' ; }

install_official() {
  say "pacotes oficiais"
  local all missing=() ok=()
  mapfile -t all < <(read_list "$REPO/packages/official.txt")
  if ((DRY)); then
    note "${#all[@]} pacotes: ${all[*]}"
    return
  fi
  $SUDO pacman -Sy --noconfirm >/dev/null
  for p in "${all[@]}"; do
    if pacman -Si "$p" >/dev/null 2>&1; then ok+=("$p"); else missing+=("$p"); fi
  done
  ((${#missing[@]})) && warn "não estão nos repositórios oficiais (pulados): ${missing[*]}"
  $SUDO pacman -S --needed --noconfirm "${ok[@]}"
  # `|| true`: sob `set -e` a lista vazia faria o script sair aqui.
  printf '%s\n' "${missing[@]}" > "${XDG_CACHE_HOME:-$HOME/.cache}/clios-missing.txt" || true
}

install_aur() {
  ((AUR)) || { note "AUR pulado (--no-aur)"; return; }
  say "pacotes do AUR"
  local helper="" all
  for h in paru yay; do command -v "$h" >/dev/null && { helper="$h"; break; }; done
  mapfile -t all < <(read_list "$REPO/packages/aur.txt")
  if [[ -z "$helper" ]]; then
    warn "nenhum helper do AUR (paru ou yay). Para instalar depois: ${all[*]}"
    return
  fi
  run "$helper" -S --needed --noconfirm "${all[@]}"
}

# ── o clios ───────────────────────────────────────────────────────────────
build_clios() {
  say "compilando o clios"
  run cargo build --release --locked --manifest-path "$REPO/Cargo.toml"
  run $SUDO install -Dm755 "$REPO/target/release/clios" /usr/local/bin/clios

  # O clios acha o checkout em ~/.local/share/clios (ou /usr/share/clios).
  local link="$HOME/.local/share/clios"
  if [[ -e "$link" && "$(readlink -f "$link")" != "$REPO" ]]; then
    warn "$link já existe e aponta para outro lugar; mantive. Use CLIOS_ROOT=$REPO se for o caso."
  else
    run mkdir -p "$(dirname "$link")"
    run ln -sfn "$REPO" "$link"
  fi
}

sync_dotfiles() {
  say "dotfiles e tema"
  if ((DRY)); then
    run "$REPO/target/release/clios" --root "$REPO" sync --dry-run || note "(compile primeiro para ver o plano real)"
  else
    "$REPO/target/release/clios" --root "$REPO" sync
  fi
}

# ── sistema ───────────────────────────────────────────────────────────────
install_system_file() {
  local src="$REPO/system/$1" dst="/$1"
  if [[ -f "$dst" ]] && ! cmp -s "$src" "$dst"; then
    run $SUDO cp -n "$dst" "$dst.clios-bak"
    note "backup de $dst em $dst.clios-bak"
  fi
  run $SUDO install -Dm644 "$src" "$dst"
}

configure_system() {
  ((SYSTEM)) || { note "sistema pulado (--no-system)"; return; }
  say "arquivos de sistema"
  while IFS= read -r f; do install_system_file "${f#"$REPO/system/"}"; done \
    < <(find "$REPO/system" -type f | sort)

  say "serviços"
  for svc in greetd iwd bluetooth power-profiles-daemon upower systemd-resolved; do
    run $SUDO systemctl enable "$svc.service"
  done
  run $SUDO ln -sf /run/systemd/resolve/stub-resolv.conf /etc/resolv.conf

  say "usuário $TARGET_USER"
  # `video` permite ao brightnessctl mexer no brilho sem root.
  run $SUDO usermod -aG video "$TARGET_USER"
  if [[ "$(getent passwd "$TARGET_USER" | cut -d: -f7)" != "/usr/bin/fish" ]] && command -v fish >/dev/null; then
    run $SUDO chsh -s /usr/bin/fish "$TARGET_USER"
  fi
}

configure_cmdline() {
  ((CMDLINE)) || return 0
  say "linha de comando do kernel (systemd-boot)"
  local extra
  if ((DRY)); then extra="quiet loglevel=3 vt.default_red=… (da paleta)"; else
    extra="quiet loglevel=3 $("$REPO/target/release/clios" --root "$REPO" theme cmdline)"
  fi
  local entries=(/boot/loader/entries/*.conf)
  [[ -e "${entries[0]}" ]] || { warn "não achei entradas do systemd-boot em /boot/loader/entries"; return; }
  for e in "${entries[@]}"; do
    if grep -q "vt.default_red" "$e" 2>/dev/null; then note "$e já tem a paleta"; continue; fi
    run $SUDO cp -n "$e" "$e.clios-bak"
    run $SUDO sed -i "/^options / s|\$| $extra|" "$e"
    note "$e atualizada (backup: $e.clios-bak)"
  done
}

if ((DRY)); then say "CLIOS · bootstrap (simulação)"; else say "CLIOS · bootstrap"; fi
install_official
install_aur
build_clios
sync_dotfiles
configure_system
configure_cmdline

say "pronto"
note "reinicie. O greetd abre o login; escolha o seu usuário e o Hyprland sobe sozinho."
note "depois: SUPER + espaço abre o hub, SUPER + / mostra todos os atalhos, e 'clios doctor' diz o que falta."
