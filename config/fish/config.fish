# CLIOS · fish
# Só roda em sessão interativa. Cores: conf.d/clios-colors.fish (gerado).
status is-interactive; or return

set -g fish_greeting
fish_add_path -g ~/.local/bin ~/.cargo/bin

# ferramentas: cada uma só é ligada se estiver instalada, para o shell nunca quebrar ao abrir
type -q starship; and starship init fish | source
type -q zoxide; and zoxide init fish | source
type -q fzf; and fzf --fish | source
# Ctrl+R vira o histórico do atuin (fuzzy, com contexto); a seta para cima continua a do fish.
type -q atuin; and atuin init fish --disable-up-arrow | source

# ls, git e afins
type -q eza; and begin
    alias ls 'eza --group-directories-first'
    alias ll 'eza -l --group-directories-first --git'
    alias la 'eza -la --group-directories-first --git'
end
abbr -a g lazygit
abbr -a c clios
abbr -a ap 'clios apps'
type -q paru; and abbr -a yay paru
type -q tldr; and abbr -a '?' tldr
abbr -a e hx
abbr -a .. 'cd ..'
abbr -a ... 'cd ../..'
abbr -a gs 'git status -sb'
abbr -a gd 'git diff'
abbr -a gl 'git log --oneline --graph -20'

set -gx BAT_THEME ansi
set -gx MANPAGER "sh -c 'col -bx | bat -l man -p'"
set -gx RIPGREP_CONFIG_PATH ~/.config/ripgrep/config

# Abre um app gráfico e entrega o terminal de volta (o Hyprland troca as janelas por swallow).
function open --description 'abrir com o app padrão, sem prender o terminal'
    for f in $argv
        setsid -f xdg-open $f >/dev/null 2>&1
    end
end
