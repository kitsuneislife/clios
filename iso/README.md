# ISO ao vivo

Uma sessão CLIOS completa para experimentar numa VM ou num pendrive, sem instalar nada.

```sh
sudo pacman -S archiso rust
./iso/build.sh          # gera iso/out/clios-AAAA.MM.DD-x86_64.iso
```

O `build.sh` parte do perfil `releng` oficial do archiso (então acompanha as atualizações dele) e sobrepõe:

- os pacotes de `packages/official.txt`;
- o binário `clios` compilado;
- os dotfiles já renderizados em `/etc/skel` (`clios sync --copy --home`), com o tema padrão;
- um usuário `live` (sem senha) e o login automático do greetd direto no Hyprland.

**Estado**: o perfil e o script não foram executados ainda (o ambiente onde foram escritos não é Arch).
Rode numa máquina Arch ou use o workflow `iso` do GitHub Actions (`.github/workflows/iso.yml`),
e conte com ajustes na primeira execução.

Para instalar de verdade, use `archinstall` (perfil *minimal*) e depois `scripts/bootstrap.sh`.
