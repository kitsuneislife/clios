-- O que sobe com a sessão. Quase tudo é daemon; os apps o usuário abre quando quer.
return function(_)
  hl.on("hyprland.start", function()
    -- Os portais (arquivos, captura de tela) são serviços do systemd e precisam saber qual é a tela.
    hl.exec_cmd("dbus-update-activation-environment --systemd WAYLAND_DISPLAY XDG_CURRENT_DESKTOP HYPRLAND_INSTANCE_SIGNATURE")
    -- O servidor do foot é o que deixa cada janela de terminal abrir instantaneamente.
    hl.exec_cmd("foot --server")
    hl.exec_cmd("quickshell")
    hl.exec_cmd("hypridle")
    hl.exec_cmd("/usr/lib/hyprpolkitagent/hyprpolkitagent")

    -- Pendrives e HDs externos montam sozinhos (e avisam); `udiskie-umount --all` ejeta tudo.
    hl.exec_cmd("udiskie --no-tray")

    -- Histórico da área de transferência (lido pelo hub com `"`).
    hl.exec_cmd("wl-paste --type text --watch cliphist store")
    hl.exec_cmd("wl-paste --type image --watch cliphist store")

    -- O modo noturno agendado (`clios night auto 20:30-06:45`), se houver.
    hl.exec_cmd("clios night --login")

    -- Garante tema e terminais coerentes mesmo se o estado salvo mudou fora da sessão.
    hl.exec_cmd("clios theme apply --no-hooks")

    -- O terminal do scratchpad nasce escondido, depois que o servidor do foot estiver de pé.
    hl.timer(function()
      hl.exec_cmd("footclient -a clios.scratch")
    end, { timeout = 800, type = "oneshot" })

    -- As janelas da última sessão voltam (`clios session off` desliga), e a de agora é salva a cada minuto.
    hl.timer(function()
      hl.exec_cmd("clios session restore --login")
    end, { timeout = 1200, type = "oneshot" })
    hl.timer(function()
      hl.exec_cmd("clios session save --quiet")
    end, { timeout = 60000, type = "repeat" })

    -- No primeiro login, o guia de boas-vindas abre sozinho (depois disso, só com SUPER + F10).
    hl.timer(function()
      hl.exec_cmd("clios welcome --first-run")
    end, { timeout = 1800, type = "oneshot" })
  end)
end
