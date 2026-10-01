-- Aparência: plana, reta, sem blur e sem sombra. O que não existe não custa GPU.
return function(T)
  hl.config({
    general = {
      gaps_in = T.ui.gap_in,
      gaps_out = T.ui.gap_out,
      border_size = T.ui.border,
      col = {
        active_border = T.color.accent,
        inactive_border = T.color.line,
      },
      resize_on_border = true,
      allow_tearing = false,
      layout = "dwindle",
    },

    decoration = {
      rounding = T.ui.radius,
      active_opacity = 1.0,
      inactive_opacity = 1.0,
      shadow = { enabled = false },
      blur = { enabled = false },
    },

    dwindle = {
      preserve_split = true,
      force_split = 2, -- a janela nova sempre entra à direita/abaixo
    },

    misc = {
      disable_hyprland_logo = true,
      disable_splash_rendering = true,
      force_default_wallpaper = 0,
      background_color = T.color.bg,
      -- O terminal que lança um programa gráfico cede o lugar a ele, e volta quando ele fecha.
      enable_swallow = true,
      swallow_regex = "^(foot|footclient|clios\\.tui\\..*)$",
      swallow_exception_regex = "^(wev|Wayland-event-viewer)$",
      focus_on_activate = false,
      mouse_move_enables_dpms = true,
      key_press_enables_dpms = true,
      enable_anr_dialog = false,
      close_special_on_empty = true,
    },

    cursor = {
      hide_on_key_press = true,
      inactive_timeout = 4,
    },

    binds = {
      workspace_back_and_forth = true,
      allow_workspace_cycles = true,
    },

    xwayland = { force_zero_scaling = true },

    ecosystem = {
      no_update_news = true,
      no_donation_nag = true,
    },
  })
end
