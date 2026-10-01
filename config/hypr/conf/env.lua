-- Ambiente herdado por tudo que o Hyprland abre.
return function(_)
  hl.env("XCURSOR_SIZE", "24")
  hl.env("HYPRCURSOR_SIZE", "24")

  -- Wayland nativo sempre que o app deixar.
  hl.env("QT_QPA_PLATFORM", "wayland;xcb")
  hl.env("GDK_BACKEND", "wayland,x11")
  hl.env("SDL_VIDEODRIVER", "wayland,x11")
  hl.env("MOZ_ENABLE_WAYLAND", "1")
  hl.env("ELECTRON_OZONE_PLATFORM_HINT", "auto")

  hl.env("XDG_CURRENT_DESKTOP", "Hyprland")
  hl.env("XDG_SESSION_DESKTOP", "Hyprland")

  -- O terminal é o centro: tudo que pede editor ou pager abre aqui.
  hl.env("EDITOR", "hx")
  hl.env("VISUAL", "hx")
  hl.env("PAGER", "less")
  hl.env("BROWSER", "firefox")
end
