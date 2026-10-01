-- O que sobe com a sessão. Quase tudo é daemon; os apps o usuário abre quando quer.
return function(_)
  hl.on("hyprland.start", function()
    -- O servidor do foot é o que deixa cada janela de terminal abrir instantaneamente.
    hl.exec_cmd("foot --server")
    hl.exec_cmd("quickshell")
    hl.exec_cmd("hypridle")
    hl.exec_cmd("/usr/lib/hyprpolkitagent/hyprpolkitagent")

    -- Histórico da área de transferência (lido pelo hub com `"`).
    hl.exec_cmd("wl-paste --type text --watch cliphist store")
    hl.exec_cmd("wl-paste --type image --watch cliphist store")

    -- Garante tema e terminais coerentes mesmo se o estado salvo mudou fora da sessão.
    hl.exec_cmd("clios theme apply --no-hooks")

    -- O terminal do scratchpad nasce escondido, depois que o servidor do foot estiver de pé.
    hl.timer(function()
      hl.exec_cmd("footclient -a clios.scratch")
    end, { timeout = 800, type = "oneshot" })

    -- No primeiro login, o guia de boas-vindas abre sozinho (depois disso, só com SUPER + F10).
    hl.timer(function()
      hl.exec_cmd("clios welcome --first-run")
    end, { timeout = 1800, type = "oneshot" })
  end)
end
