-- Rede de segurança: o suficiente para o Hyprland subir antes do primeiro `clios sync`.
-- Mesmo formato de theme.lua; movimento desligado.
return {
  mode = "dark",
  accent = "ember",
  color = {
    bg = "rgb(000000)", surface = "rgb(0A0A0A)", raised = "rgb(151515)", line = "rgb(2A2A2A)",
    mute = "rgb(6B6B6B)", dim = "rgb(A8A8A8)", fg = "rgb(F5F5F5)",
    accent = "rgb(FF5A1F)", accent_dim = "rgb(8F3412)", on_accent = "rgb(000000)", red = "rgb(FF6166)",
  },
  font = "GeistMono Nerd Font",
  ui = { gap_in = 6, gap_out = 12, border = 2, radius = 0, bar_height = 28 },
  motion = {
    enabled = false, spatial = false,
    ms = { instant = 0, fast = 0, base = 0, slow = 0 },
    curve = {}, spring = {},
  },
}
