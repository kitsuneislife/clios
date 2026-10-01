-- Regras de janela e de camada.
--
-- Convenção de classes (app-id do foot), definida em `clios open`:
--   clios.hub        o lançador                 flutuante, centralizado
--   clios.float.*    TUIs flutuantes             flutuante, centralizadas
--   clios.tui.*      TUIs em mosaico             nenhuma regra
--   clios.scratch    terminal do scratchpad      mora na workspace especial
return function(T)
  -- Apps que pedem para maximizar: ignorados. Quem manda no tamanho é o layout.
  hl.window_rule({
    name = "suppress-maximize",
    match = { class = ".*" },
    suppress_event = "maximize",
  })

  hl.window_rule({
    name = "hub",
    match = { class = "^clios\\.hub$" },
    float = true,
    center = true,
    stay_focused = true,
    animation = T.motion.spatial and "popin 94%" or "popin 100%",
  })

  hl.window_rule({
    name = "float-tuis",
    match = { class = "^clios\\.float\\..*" },
    float = true,
    center = true,
  })

  hl.window_rule({
    name = "scratchpad",
    match = { class = "^clios\\.scratch$" },
    workspace = "special:scratch silent",
    float = true,
    center = true,
    size = { "monitor_w*0.62", "monitor_h*0.56" },
  })

  -- Diálogos que precisam de foco e de cara de diálogo.
  hl.window_rule({
    name = "pinentry",
    match = { class = "^(pinentry-.*|hyprpolkitagent|org\\.hyprland\\..*)$" },
    float = true,
    center = true,
    stay_focused = true,
  })

  -- Vídeo: a tela não apaga enquanto o mpv está em foco.
  hl.window_rule({
    name = "mpv",
    match = { class = "^mpv$" },
    idle_inhibit = "focus",
  })

  -- Picture-in-picture do navegador.
  hl.window_rule({
    name = "pip",
    match = { title = "^(Picture-in-Picture|Picture in picture)$" },
    float = true,
    pin = true,
    size = { 480, 270 },
    move = { "monitor_w-window_w-24", "monitor_h-window_h-24" },
  })

  -- Os painéis da shell se animam por dentro (Quickshell). Sem animação do compositor por cima.
  hl.layer_rule({ name = "shell-no-anim", match = { namespace = "^clios-" }, no_anim = true })
end
