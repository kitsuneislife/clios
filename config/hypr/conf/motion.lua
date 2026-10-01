-- Movimento. As curvas e durações são os tokens; aqui só se decide o que usa o quê.
--
-- Regras do manifesto (tokens/tokens.toml):
--   * entrada rápida, saída ~60% da entrada
--   * mola amortecida criticamente só para o que ocupa espaço (janelas, workspaces)
--   * o resto usa a curva `out`
--
-- Hyprland mede a velocidade em decisegundos: 100ms = 1.
return function(T)
  local m = T.motion

  hl.config({ animations = { enabled = m.enabled } })
  if not m.enabled then
    return
  end

  -- Registra todas as curvas dos tokens com prefixo, sem colidir com as embutidas.
  for name, p in pairs(m.curve) do
    hl.curve("clios_" .. name, { type = "bezier", points = { { p[1], p[2] }, { p[3], p[4] } } })
  end
  for name, s in pairs(m.spring) do
    hl.curve("clios_" .. name, { type = "spring", mass = s.mass, stiffness = s.stiffness, damping = s.damping })
  end

  local function ds(ms) return math.max(ms / 100, 0.1) end
  local function exit_ms(ms) return ms * 0.6 end

  local base, fast = ds(m.ms.base), ds(m.ms.fast)
  local out_base, out_fast = ds(exit_ms(m.ms.base)), ds(exit_ms(m.ms.fast))

  -- Sem movimento espacial (`clios motion reduced`), nada desliza nem escala: só fade.
  local pop = m.spatial and "popin 92%" or "popin 100%"
  local slide = m.spatial and "slidefade 8%" or "fade"

  hl.animation({ leaf = "global", enabled = true, speed = base, bezier = "clios_out" })

  hl.animation({ leaf = "windows", enabled = true, speed = base, spring = "clios_settle", style = pop })
  hl.animation({ leaf = "windowsIn", enabled = true, speed = base, spring = "clios_settle", style = pop })
  hl.animation({ leaf = "windowsOut", enabled = true, speed = out_base, bezier = "clios_in", style = pop })
  hl.animation({ leaf = "windowsMove", enabled = true, speed = base, spring = "clios_settle" })

  hl.animation({ leaf = "fade", enabled = true, speed = fast, bezier = "clios_out" })
  hl.animation({ leaf = "fadeIn", enabled = true, speed = fast, bezier = "clios_out" })
  hl.animation({ leaf = "fadeOut", enabled = true, speed = out_fast, bezier = "clios_in" })

  hl.animation({ leaf = "border", enabled = true, speed = fast, bezier = "clios_out" })

  hl.animation({ leaf = "workspaces", enabled = true, speed = base, bezier = "clios_out", style = slide })
  hl.animation({ leaf = "specialWorkspace", enabled = true, speed = base, bezier = "clios_out",
                 style = m.spatial and "slidefadevert 12%" or "fade" })

  -- A shell (Quickshell) anima os próprios painéis por dentro; o compositor não duplica.
  hl.animation({ leaf = "layers", enabled = false })
  hl.animation({ leaf = "fadeLayers", enabled = false })
end
