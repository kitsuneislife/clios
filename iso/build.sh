#!/usr/bin/env bash
# Monta a ISO do CLIOS a partir do perfil releng do archiso.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"
WORK="$HERE/work"
OUT="$HERE/out"
RELENG=/usr/share/archiso/configs/releng

[[ -d "$RELENG" ]] || { echo "erro: instale o archiso (pacman -S archiso)" >&2; exit 1; }
command -v cargo >/dev/null || { echo "erro: instale o rust (pacman -S rust)" >&2; exit 1; }
if [[ $EUID -ne 0 ]]; then
  echo "mkarchiso precisa de root; use: sudo $0" >&2
  exit 1
fi

rm -rf "$WORK"
mkdir -p "$WORK" "$OUT"

echo "▌ perfil releng"
cp -r "$RELENG" "$WORK/profile"
PROFILE="$WORK/profile"

echo "▌ pacotes"
{
  cat "$RELENG/packages.x86_64"
  grep -vE '^\s*(#|$)' "$REPO/packages/official.txt" | sed 's/\s*#.*//'
} | sort -u > "$PROFILE/packages.x86_64"

echo "▌ clios"
cargo build --release --locked --manifest-path "$REPO/Cargo.toml"
install -Dm755 "$REPO/target/release/clios" "$PROFILE/airootfs/usr/local/bin/clios"

mkdir -p "$PROFILE/airootfs/usr/share/fish/vendor_completions.d" "$PROFILE/airootfs/usr/share/bash-completion/completions"
"$REPO/target/release/clios" completions fish > "$PROFILE/airootfs/usr/share/fish/vendor_completions.d/clios.fish"
"$REPO/target/release/clios" completions bash > "$PROFILE/airootfs/usr/share/bash-completion/completions/clios"

# O checkout dentro da ISO: o clios procura em /usr/share/clios.
mkdir -p "$PROFILE/airootfs/usr/share/clios"
cp -r "$REPO"/{tokens,templates,config,seed,brand} "$PROFILE/airootfs/usr/share/clios/"

echo "▌ dotfiles renderizados em /etc/skel"
mkdir -p "$PROFILE/airootfs/etc/skel"
"$REPO/target/release/clios" --root "$REPO" --home "$PROFILE/airootfs/etc/skel" sync --copy

echo "▌ sobreposição do CLIOS"
cp -r "$HERE/airootfs/." "$PROFILE/airootfs/"
install -Dm644 "$REPO/system/etc/sysctl.d/99-clios.conf" "$PROFILE/airootfs/etc/sysctl.d/99-clios.conf"
install -Dm644 "$REPO/system/etc/systemd/zram-generator.conf" "$PROFILE/airootfs/etc/systemd/zram-generator.conf"
install -Dm644 "$REPO/system/etc/iwd/main.conf" "$PROFILE/airootfs/etc/iwd/main.conf"
install -Dm644 "$REPO/system/etc/nftables.conf" "$PROFILE/airootfs/etc/nftables.conf"

# Identidade da ISO e usuário `live`.
# O $(date) fica entre aspas simples de propósito: quem o avalia é o profiledef.sh, ao ser lido.
# shellcheck disable=SC2016
sed -i \
  -e 's/^iso_name=.*/iso_name="clios"/' \
  -e 's/^iso_label=.*/iso_label="CLIOS_$(date +%Y%m)"/' \
  -e 's/^iso_publisher=.*/iso_publisher="CLIOS"/' \
  -e 's/^iso_application=.*/iso_application="CLIOS live"/' \
  "$PROFILE/profiledef.sh"
{
  echo 'root:x:0:0:root:/root:/usr/bin/bash'
  echo 'live:x:1000:1000:live:/home/live:/usr/bin/fish'
  echo 'greeter:x:970:970:greetd:/var/lib/greetd:/usr/bin/nologin'
} > "$PROFILE/airootfs/etc/passwd"
sed -i '/^live:/d' "$PROFILE/airootfs/etc/group" 2>/dev/null || true
echo 'live:x:1000:' >> "$PROFILE/airootfs/etc/group"
echo 'root::14871::::::' > "$PROFILE/airootfs/etc/shadow"
echo 'live::14871::::::' >> "$PROFILE/airootfs/etc/shadow"
echo 'greeter:!:14871::::::' >> "$PROFILE/airootfs/etc/shadow"
cat >> "$PROFILE/profiledef.sh" <<'PERMS'
file_permissions+=(
  ["/etc/shadow"]="0:0:400"
  ["/usr/local/bin/clios"]="0:0:755"
  ["/usr/local/bin/clios-live-setup"]="0:0:755"
)
PERMS

# /etc/skel só vale para usuários novos: copia para o home do live.
mkdir -p "$PROFILE/airootfs/home"
cp -r "$PROFILE/airootfs/etc/skel" "$PROFILE/airootfs/home/live"

echo "▌ mkarchiso"
mkarchiso -v -w "$WORK/tmp" -o "$OUT" "$PROFILE"
isos=("$OUT"/*.iso)
echo "pronto: ${isos[-1]}"
