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
#   --extras           instala também os apps recomendados (clios apps install --extras)
#   --no-aur           não instala os pacotes do AUR
#   --no-system        não mexe em /etc nem em serviços (só pacotes e dotfiles)
#   --kernel-cmdline   acrescenta quiet + paleta do console às entradas do systemd-boot (com backup)
#   --dry-run          não executa nada, só imprime
#   -h, --help
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DRY=0 AUR=1 SYSTEM=1 CMDLINE=0 EXTRAS=0 MISSING=()

usage() { sed -n '2,/^set -euo/p' "${BASH_SOURCE[0]}" | sed '$d; s/^# \{0,1\}//'; }

for arg in "$@"; do
  case "$arg" in
    --dry-run) DRY=1 ;;
    --extras) EXTRAS=1 ;;
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
  # O que o pacman não acha pode estar no AUR (pacotes novos, por exemplo): o install_aur tenta.
  MISSING=("${missing[@]}")
  ((${#missing[@]})) && note "fora dos repositórios oficiais, tentando no AUR: ${missing[*]}"
  $SUDO pacman -S --needed --noconfirm "${ok[@]}"
}

# O paru é o helper do AUR (e o `clios apps` e o hub dependem dele). Vem do AUR, então compila.
bootstrap_paru() {
  command -v paru >/dev/null && return 0
  command -v yay >/dev/null && return 0
  say "paru (helper do AUR)"
  local tmp
  if ((DRY)); then
    run git clone --depth 1 https://aur.archlinux.org/paru-bin.git /tmp/paru-bin
    run makepkg -si --noconfirm
    return
  fi
  tmp="$(mktemp -d)"
  if git clone --depth 1 https://aur.archlinux.org/paru-bin.git "$tmp/paru-bin" &&
    (cd "$tmp/paru-bin" && makepkg -si --noconfirm); then
    note "paru instalado"
  else
    warn "não consegui instalar o paru. Tente depois: git clone https://aur.archlinux.org/paru-bin.git && cd paru-bin && makepkg -si"
  fi
  rm -rf "$tmp"
}

install_aur() {
  ((AUR)) || { note "AUR pulado (--no-aur)"; return; }
  say "pacotes do AUR"
  bootstrap_paru
  local helper="" all
  for h in paru yay; do command -v "$h" >/dev/null && { helper="$h"; break; }; done
  mapfile -t all < <(read_list "$REPO/packages/aur.txt")
  all+=("${MISSING[@]}")
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
  # O portal de arquivos chama `clios-filechooser`: o mesmo binário, por outro nome.
  run $SUDO ln -sf clios /usr/local/bin/clios-filechooser

  # Autocompletar (tab) no fish e no bash, com os ids do catálogo.
  if ((DRY)); then
    note "autocompletar: clios completions fish e bash em /usr/share"
  else
    "$REPO/target/release/clios" completions fish | $SUDO install -Dm644 /dev/stdin /usr/share/fish/vendor_completions.d/clios.fish
    "$REPO/target/release/clios" completions bash | $SUDO install -Dm644 /dev/stdin /usr/share/bash-completion/completions/clios
  fi

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

install_extras() {
  ((EXTRAS)) || return 0
  say "apps recomendados"
  if ((DRY)); then run "$REPO/target/release/clios" --root "$REPO" apps install --extras --dry-run; return; fi
  "$REPO/target/release/clios" --root "$REPO" apps install --extras || warn "alguns extras falharam; rode 'clios apps install --extras' de novo depois."
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
  # nftables: firewall de entrada fechada (system/etc/nftables.conf); timesyncd: a hora certa, sem pedir nada.
  for svc in greetd iwd bluetooth power-profiles-daemon upower systemd-resolved systemd-timesyncd nftables; do
    run $SUDO systemctl enable "$svc.service"
  done
  # Manutenção que ninguém lembra de fazer: cache do pacman, espelhos, índice do `pkgfile`.
  for timer in paccache.timer reflector.timer pkgfile-update.timer; do
    run $SUDO systemctl enable "$timer"
  done
  run $SUDO ln -sf /run/systemd/resolve/stub-resolv.conf /etc/resolv.conf

  say "usuário $TARGET_USER"
  # `video` permite ao brightnessctl mexer no brilho sem root.
  run $SUDO usermod -aG video "$TARGET_USER"
  if [[ "$(getent passwd "$TARGET_USER" | cut -d: -f7)" != "/usr/bin/fish" ]] && command -v fish >/dev/null; then
    run $SUDO chsh -s /usr/bin/fish "$TARGET_USER"
  fi
}

# ── fotografias do sistema (btrfs + snapper) ──────────────────────────────
# Cada transação do pacman ganha uma fotografia antes e outra depois (snap-pac); `clios snap` lista, compara e desfaz.
configure_snapshots() {
  ((SYSTEM)) || return 0
  local fs
  fs="$(findmnt -no FSTYPE / 2>/dev/null || true)"
  if [[ "$fs" != btrfs ]]; then
    note "fotografias do sistema puladas: a raiz é ${fs:-desconhecida}, e elas precisam de btrfs"
    return 0
  fi
  say "fotografias do sistema (snapper)"
  run $SUDO pacman -S --needed --noconfirm snapper snap-pac
  if [[ ! -f /etc/snapper/configs/root ]]; then
    if findmnt -n /.snapshots >/dev/null 2>&1; then
      # O archinstall já monta o subvolume @.snapshots em /.snapshots, e o snapper quer criar o dele.
      # O caminho da ArchWiki: deixa o snapper criar a config, apaga o subvolume dele e monta o do archinstall de novo.
      run $SUDO umount /.snapshots
      run $SUDO rmdir /.snapshots
      run $SUDO snapper -c root create-config /
      run $SUDO btrfs subvolume delete /.snapshots
      run $SUDO mkdir /.snapshots
      run $SUDO mount -a
      run $SUDO chmod 750 /.snapshots
    else
      run $SUDO snapper -c root create-config /
    fi
  fi
  # Sem fotografias de hora em hora (as do pacman bastam), poucas guardadas, e o seu usuário lista e cria sem sudo.
  run $SUDO snapper -c root set-config TIMELINE_CREATE=no NUMBER_LIMIT=12 NUMBER_LIMIT_IMPORTANT=6 \
    "ALLOW_USERS=$TARGET_USER" SYNC_ACL=yes
  run $SUDO systemctl enable snapper-cleanup.timer

  # Com o limine, as fotografias aparecem no menu de boot: dá para voltar mesmo se o sistema não subir.
  if [[ -d /boot/limine || -f /boot/EFI/limine/limine.conf || -f /boot/limine.conf ]]; then
    local helper=""
    for h in paru yay; do command -v "$h" >/dev/null && { helper="$h"; break; }; done
    if ((AUR)) && [[ -n "$helper" ]]; then
      run "$helper" -S --needed --noconfirm limine-snapper-sync
      run $SUDO systemctl enable limine-snapper-sync.service
    else
      note "para as fotografias aparecerem no menu do limine, instale limine-snapper-sync (AUR)"
    fi
  else
    note "o menu do systemd-boot não lista fotografias; \`clios snap undo\` desfaz com o sistema rodando"
  fi
  if ! ((DRY)) && ! $SUDO snapper -c root list 2>/dev/null | grep -q "clios bootstrap"; then
    $SUDO snapper -c root create -c number -d "clios bootstrap" || warn "não consegui tirar a primeira fotografia"
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
install_extras
configure_system
configure_snapshots
configure_cmdline

say "pronto"
note "reinicie. O greetd abre o login; escolha o seu usuário e o Hyprland sobe sozinho."
note "depois: SUPER + espaço abre o hub, SUPER + / mostra todos os atalhos, e 'clios doctor' diz o que falta."
