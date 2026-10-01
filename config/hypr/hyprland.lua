-- CLIOS · Hyprland (config em Lua, Hyprland >= 0.55)
--
-- Este arquivo só costura os módulos de conf/. Cada `require` roda isolado: se um módulo
-- quebrar, os outros continuam e o Hyprland mostra o erro, em vez de cair na config de emergência.
--
-- Tokens (cores, durações, curvas) vêm de theme.lua, gerado por `clios theme apply`.
-- Para mudar algo seu (teclado, monitores), edite user.lua, que carrega por último.

local ok, T = pcall(require, "theme")
if not ok then
  -- Primeira execução, antes do `clios sync`: valores seguros, sem animação.
  T = require("conf.fallback")
end

require("conf.env")(T)
require("conf.look")(T)
require("conf.motion")(T)
require("conf.input")(T)
require("conf.rules")(T)
require("conf.binds")(T)
require("conf.autostart")(T)

-- Seu: monitores, layout de teclado, qualquer ajuste. Não existe até você criar.
pcall(require, "user")
