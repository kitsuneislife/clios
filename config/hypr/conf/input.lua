return function(_)
  hl.config({
    input = {
      kb_layout = "br",
      -- Caps vira Esc: no helix e no shell, é a tecla mais usada do teclado.
      kb_options = "caps:escape",
      repeat_rate = 40,
      repeat_delay = 220,
      follow_mouse = 1,
      sensitivity = 0,
      touchpad = { natural_scroll = true },
    },
  })

  hl.gesture({ fingers = 3, direction = "horizontal", action = "workspace" })

  -- Monitor: o que o sistema achar melhor. Ajuste em user.lua.
  hl.monitor({ output = "", mode = "preferred", position = "auto", scale = "auto" })
end
