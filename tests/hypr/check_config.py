#!/usr/bin/env python3
"""Executa a config Lua do Hyprland contra um `hl` simulado e valida cada chamada
contra o stub oficial (hl.meta.lua): funções, chaves de config, tipos, folhas de
animação, efeitos de regra, sintaxe de atalhos e conflitos.

Roda a config em todas as combinações de modo x movimento, com o tema renderizado
pelo próprio `clios`. Não precisa do Hyprland instalado.

uso: check_config.py [--clios target/debug/clios] [--repo .]
requer: pip install lupa
"""
import argparse
import itertools
import pathlib
import re
import subprocess
import sys
import tempfile
import tomllib

from lupa import LuaRuntime

HERE = pathlib.Path(__file__).resolve().parent

# Do wiki do Hyprland (Window Rules): props de match e efeitos aceitos.
MATCH_PROPS = {
    "class", "content", "focus", "fullscreen", "fullscreen_state_client", "fullscreen_state_internal",
    "float", "group", "initial_class", "initial_title", "modal", "pin", "tag", "title", "workspace",
    "xdg_tag", "xwayland",
}
WINDOW_EFFECTS = {
    # estáticos
    "center", "content", "float", "fullscreen", "fullscreen_state", "group", "maximize", "monitor", "move",
    "no_close_for", "no_initial_focus", "pin", "pseudo", "scrolling_width", "size", "suppress_event", "tile",
    "workspace",
    # dinâmicos
    "allows_input", "animation", "border_color", "border_size", "confine_pointer", "dim_around", "decorate",
    "focus_on_activate", "idle_inhibit", "immediate", "keep_aspect_ratio", "max_size", "min_size",
    "nearest_neighbor", "no_anim", "no_auto_hdr", "no_blur", "no_dim", "no_focus", "no_follow_mouse",
    "no_glow", "no_max_size", "no_screen_share", "no_shadow", "no_shortcuts_inhibit", "no_vrr", "no_wobble",
    "no_xdg_drags", "opacity", "opaque", "force_rgbx", "persistent_size", "render_unfocused", "rounding",
    "rounding_power", "scroll_mouse", "scroll_touchpad", "stay_focused", "sync_fullscreen", "tag", "tonemap",
    "xray",
}
# Do wiki (Animations): a árvore de folhas e os estilos de cada família.
LEAF_STYLES = {
    "global": set(),
    "windows": {"slide", "popin", "gnomed"}, "windowsIn": {"slide", "popin", "gnomed"},
    "windowsOut": {"slide", "popin", "gnomed"}, "windowsMove": set(),
    "layers": {"slide", "popin", "fade"}, "layersIn": {"slide", "popin", "fade"},
    "layersOut": {"slide", "popin", "fade"},
    "fade": set(), "fadeIn": set(), "fadeOut": set(), "fadeSwitch": set(), "fadeShadow": set(),
    "fadeGlow": set(), "fadeDim": set(), "fadeLayers": set(), "fadeLayersIn": set(), "fadeLayersOut": set(),
    "fadePopups": set(), "fadePopupsIn": set(), "fadePopupsOut": set(), "fadeDpms": set(),
    "border": set(), "borderangle": {"once", "loop"}, "shadowangle": {"once", "loop"},
    "glowangle": {"once", "loop"},
    "workspaces": {"slide", "slidevert", "fade", "slidefade", "slidefadevert"},
    "workspacesIn": {"slide", "slidevert", "fade", "slidefade", "slidefadevert"},
    "workspacesOut": {"slide", "slidevert", "fade", "slidefade", "slidefadevert"},
    "specialWorkspace": {"slide", "slidevert", "fade", "slidefade", "slidefadevert"},
    "specialWorkspaceIn": {"slide", "slidevert", "fade", "slidefade", "slidefadevert"},
    "specialWorkspaceOut": {"slide", "slidevert", "fade", "slidefade", "slidefadevert"},
    "zoomFactor": set(), "monitorAdded": set(),
}
EVENTS = {
    "config.props_refreshed", "config.reloaded", "config.unload", "hyprland.shutdown", "hyprland.start",
    "input.keyboard.key", "keybinds.submap", "layer.closed", "layer.opened", "monitor.added",
    "monitor.focused", "monitor.layout_changed", "monitor.removed", "screenshare.state", "window.active",
    "window.bell", "window.class", "window.close", "window.destroy", "window.fullscreen", "window.kill",
    "window.minimize", "window.move_to_workspace", "window.open", "window.open_early", "window.pin",
    "window.title", "window.update_rules", "window.urgent", "workspace.active", "workspace.created",
    "workspace.move_to_monitor", "workspace.removed", "workspace.special_active",
}
MODS = {"SUPER", "SHIFT", "CTRL", "ALT", "MOD2", "MOD3", "MOD5", "CAPS"}


def parse_class(src, name):
    m = re.search(r"---@class " + re.escape(name) + r"\n((?:---@field .*\n)+)", src)
    if not m:
        return None
    out = {}
    for line in m.group(1).splitlines():
        f = re.match(r"---@field (\S+?)(\?)? (.*)", line)
        if f:
            out[f.group(1)] = f.group(3)
    return out


def load_stub():
    src = (HERE / "hl.meta.lua").read_text()
    api = parse_class(src, "HL.API")
    dsp = {
        "": parse_class(src, "HL.DspNamespace"),
        "cursor": parse_class(src, "HL.DspCursorNamespace"),
        "group": parse_class(src, "HL.DspGroupNamespace"),
        "window": parse_class(src, "HL.DspWindowNamespace"),
        "workspace": parse_class(src, "HL.DspWorkspaceNamespace"),
    }
    cfg = {}
    m = re.search(r"---@class HL.ConfigValueTypes\n((?:---@field .*\n)+)", src)
    for line in m.group(1).splitlines():
        f = re.match(r"---@field \['([^']+)'\] (.*)", line)
        cfg[f.group(1)] = f.group(2)
    return {
        "api": set(api),
        "dsp": {k: set(v) for k, v in dsp.items()},
        "cfg": cfg,
        "bind_opts": set(parse_class(src, "HL.BindOptions")) | {"mouse"},  # `mouse` está no wiki, não no stub
        "monitor": set(parse_class(src, "HL.MonitorSpec")),
        "gesture": set(parse_class(src, "HL.GestureSpec")),
        "layer": set(parse_class(src, "HL.LayerRuleSpec")),
        "ws_rule": set(parse_class(src, "HL.WorkspaceRuleSpec")),
    }


LUA_MOCK = r"""
local V, config_dir = ...
local errors, report = {}, { binds = {}, rules = 0, anims = {}, regexes = {}, config = {}, envs = {} }
local function err(fmt, ...) errors[#errors + 1] = string.format(fmt, ...) end
local function keys_of(t) local o = {} for k in pairs(t) do o[#o + 1] = tostring(k) end table.sort(o) return table.concat(o, ", ") end

package.path = config_dir .. "/?.lua;" .. package.path
-- `require("a.b")` do Hyprland: relativo ao diretório do hyprland.lua, com "." como separador.

local function type_ok(typestr, v)
  local t = type(v)
  local has = function(s) return typestr:find(s, 1, true) ~= nil end
  if t == "boolean" then return has("boolean") end
  if t == "number" then
    if math.type(v) == "integer" then return has("integer") or has("number") end
    return has("number")
  end
  if t == "string" then return has("string") or has("Gradient") or has("Color") end
  if t == "table" then return has("HL.") or has("table") end
  return false
end

local prefixes = {}
for k in pairs(V.cfg) do
  local p = ""
  for part in k:gmatch("[^.]+") do
    p = (p == "") and part or (p .. "." .. part)
    prefixes[p] = true
  end
end

local function flatten(prefix, t)
  for k, v in pairs(t) do
    local key = (prefix == "") and tostring(k) or (prefix .. "." .. tostring(k))
    if V.cfg[key] ~= nil then
      if not type_ok(V.cfg[key], v) then
        err("config %s = %s (%s): o tipo esperado é %s", key, tostring(v), type(v), V.cfg[key])
      end
      report.config[key] = v
    elseif type(v) == "table" and prefixes[key] then
      flatten(key, v)
    else
      err("config: a opção %s não existe nesta versão do Hyprland", key)
    end
  end
end

local curves = { default = true }
local current_submap = ""
local seen_binds = {}
local callbacks = {}

local function check_spec_keys(what, spec, allowed_sets, extra)
  for k in pairs(spec) do
    local ok = false
    for _, set in ipairs(allowed_sets) do if set[k] then ok = true end end
    if not ok then err("%s: campo desconhecido %q (campos: %s)", what, k, extra or "?") end
  end
end

local function note_regex(where, s) report.regexes[#report.regexes + 1] = { where, s } end

local function handle(kind)
  return setmetatable({}, { __index = function(_, k)
    if k == "set_enabled" or k == "is_enabled" or k == "unbind" or k == "remove" then return function() end end
  end })
end

local dsp_cache = {}
local function dsp_ns(path)
  if dsp_cache[path] then return dsp_cache[path] end
  local allowed = V.dsp[path]
  local ns = setmetatable({}, { __index = function(_, name)
    if V.dsp[name] and path == "" then return dsp_ns(name) end
    if not allowed[name] then
      err("hl.dsp%s.%s não existe", path == "" and "" or ("." .. path), name)
      return function() return {} end
    end
    return function(...) return { __dsp = (path == "" and "" or (path .. ".")) .. name, args = { ... } } end
  end })
  dsp_cache[path] = ns
  return ns
end

local hl = {}
local impl = {}

function impl.env(k, v)
  if type(k) ~= "string" or type(v) ~= "string" then err("hl.env espera (string, string), recebeu (%s, %s)", type(k), type(v)) end
  report.envs[k] = v
end

function impl.config(c)
  if type(c) ~= "table" then err("hl.config espera uma tabela") return end
  flatten("", c)
end

function impl.curve(name, spec)
  if type(name) ~= "string" then err("hl.curve: nome precisa ser string") return end
  curves[name] = true
  if spec.type == "bezier" then
    local p = spec.points
    if type(p) ~= "table" or #p ~= 2 or #p[1] ~= 2 or #p[2] ~= 2 then err("curva %s: points precisa ser {{x1,y1},{x2,y2}}", name) return end
    for i = 1, 2 do
      if type(p[i][1]) ~= "number" or type(p[i][2]) ~= "number" then err("curva %s: pontos precisam ser números", name) end
      if p[i][1] < 0 or p[i][1] > 1 then err("curva %s: x fora de [0,1] (%s) deixa de ser função do tempo", name, p[i][1]) end
    end
  elseif spec.type == "spring" then
    for _, f in ipairs({ "mass", "stiffness", "damping" }) do
      if type(spec[f]) ~= "number" or spec[f] <= 0 then err("curva %s: %s precisa ser número > 0", name, f) end
    end
  else
    err("curva %s: type precisa ser bezier ou spring, veio %s", name, tostring(spec.type))
  end
end

function impl.animation(spec)
  local leaf = spec.leaf
  local styles = V.leaf_styles[leaf]
  if styles == nil then err("animação: a folha %q não existe", tostring(leaf)) return end
  if type(spec.enabled) ~= "boolean" then err("animação %s: enabled precisa ser boolean", leaf) end
  for k in pairs(spec) do
    if not ({ leaf = 1, enabled = 1, speed = 1, bezier = 1, spring = 1, style = 1 })[k] then err("animação %s: campo %q desconhecido", leaf, k) end
  end
  if spec.enabled then
    if type(spec.speed) ~= "number" or spec.speed <= 0 then err("animação %s: speed precisa ser número > 0 (decisegundos)", leaf) end
    if (spec.bezier == nil) == (spec.spring == nil) then err("animação %s: precisa de exatamente um entre bezier e spring", leaf) end
    local c = spec.bezier or spec.spring
    if c and not curves[c] then err("animação %s: a curva %q não foi registrada antes", leaf, tostring(c)) end
    if spec.style then
      local word = spec.style:match("^(%S+)")
      if not styles[word] then err("animação %s: estilo %q inválido (válidos: %s)", leaf, spec.style, keys_of(styles)) end
    end
  end
  report.anims[leaf] = spec
end

function impl.monitor(spec)
  if type(spec.output) ~= "string" then err("hl.monitor: output é obrigatório") end
  check_spec_keys("hl.monitor", spec, { V.monitor }, keys_of(V.monitor))
end

function impl.gesture(spec) check_spec_keys("hl.gesture", spec, { V.gesture }, keys_of(V.gesture)) end

local function rule(kind, spec, extra_ok)
  report.rules = report.rules + 1
  if type(spec.match) ~= "table" then err("%s: falta `match`", kind) return handle() end
  for k, v in pairs(spec.match) do
    local allowed = (kind == "layer_rule") and ({ namespace = true }) or V.match_props
    if not allowed[k] then err("%s: match.%s não existe", kind, k) end
    if (k == "class" or k == "title" or k == "namespace" or k == "initial_class" or k == "initial_title") then note_regex(kind .. " " .. (spec.name or "?") .. "." .. k, v) end
  end
  if kind == "window_rule" then
    local has_effect = false
    for k in pairs(spec) do
      if k ~= "name" and k ~= "match" and k ~= "enabled" then
        has_effect = true
        if not V.window_effects[k] then err("window_rule %q: o efeito %q não existe", spec.name or "?", k) end
      end
    end
    if not has_effect then err("window_rule %q: sem nenhum efeito", spec.name or "?") end
    if spec.animation then
      local word = spec.animation:match("^(%S+)")
      if not V.leaf_styles.windows[word] then err("window_rule %q: animation %q inválida", spec.name or "?", spec.animation) end
    end
  else
    check_spec_keys("layer_rule", spec, { V.layer }, keys_of(V.layer))
  end
  return handle()
end
function impl.window_rule(spec) return rule("window_rule", spec) end
function impl.layer_rule(spec) return rule("layer_rule", spec) end
function impl.workspace_rule(spec)
  check_spec_keys("workspace_rule", spec, { V.ws_rule }, keys_of(V.ws_rule))
  return handle()
end

local function check_keys(keys)
  if type(keys) ~= "string" or keys == "" then err("atalho: precisa ser string não vazia") return end
  local parts = {}
  for p in (keys .. " + "):gmatch("(.-) %+ ") do parts[#parts + 1] = p end
  if #parts == 0 then err("atalho %q mal formado", keys) return end
  for i = 1, #parts - 1 do
    if not V.mods[parts[i]] then err("atalho %q: %q não é um modificador (use %s)", keys, parts[i], keys_of(V.mods)) end
  end
  local key = parts[#parts]
  if key:find("%s") or key == "" then err("atalho %q: tecla inválida %q", keys, key) end
end

function impl.bind(keys, d, opts)
  check_keys(keys)
  opts = opts or {}
  if type(d) ~= "table" and type(d) ~= "function" then err("atalho %s: o segundo argumento precisa ser um dispatcher ou função", tostring(keys)) end
  if type(d) == "table" and d.__dsp == nil then err("atalho %s: tabela que não é dispatcher", tostring(keys)) end
  for k in pairs(opts) do
    if not V.bind_opts[k] then err("atalho %s: opção %q inválida (válidas: %s)", keys, k, keys_of(V.bind_opts)) end
  end
  local id = current_submap .. "|" .. tostring(keys)
  if seen_binds[id] and not opts.long_press and not opts.release then
    err("atalho duplicado: %s (submap %q)", keys, current_submap)
  end
  seen_binds[id] = true
  report.binds[#report.binds + 1] = { keys = keys, description = opts.description, submap = current_submap }
  return handle()
end

function impl.define_submap(name, a, b)
  local fn = type(a) == "function" and a or b
  if type(name) ~= "string" or type(fn) ~= "function" then err("define_submap(nome, [reset], função)") return end
  local prev = current_submap
  current_submap = name
  fn()
  current_submap = prev
end

function impl.on(event, cb)
  if not V.events[event] then err("hl.on: evento %q não existe", tostring(event)) end
  if type(cb) ~= "function" then err("hl.on(%s): precisa de função", tostring(event)) return end
  callbacks[#callbacks + 1] = { event = event, cb = cb }
  return handle()
end

function impl.timer(cb, opts)
  if type(cb) ~= "function" then err("hl.timer: precisa de função") end
  if type(opts) ~= "table" or math.type(opts.timeout) ~= "integer" or (opts.type ~= "repeat" and opts.type ~= "oneshot") then
    err("hl.timer: opts precisa de timeout (inteiro, ms) e type (repeat|oneshot)")
  end
  callbacks[#callbacks + 1] = { event = "timer", cb = cb }
  return handle()
end

function impl.exec_cmd(cmd, rules)
  if type(cmd) ~= "string" or cmd == "" then err("hl.exec_cmd: comando precisa ser string") end
  report.execs = (report.execs or 0) + 1
end
function impl.dispatch(d) end
function impl.unbind() end

setmetatable(hl, {
  __index = function(_, name)
    if name == "dsp" then return dsp_ns("") end
    if impl[name] then return impl[name] end
    if not V.api[name] then
      err("hl.%s não existe na API", tostring(name))
    end
    return function() return handle() end
  end,
})
_G.hl = hl

-- Dispatchers de exec_cmd dentro de binds precisam aceitar string.
local dsp = dsp_ns("")
local orig = dsp.exec_cmd

-- O hyprland.lua cai no fallback em silêncio se theme.lua quebrar; aqui isso é erro.
do
  local f, lerr = loadfile(config_dir .. "/theme.lua")
  if not f then err("theme.lua não carrega (o config usaria o fallback sem animação): %s", tostring(lerr))
  else
    local ok3, t = pcall(f)
    if not ok3 then err("theme.lua falhou ao executar: %s", tostring(t)) end
  end
end

local ok, e = pcall(function()
  local f, lerr = loadfile(config_dir .. "/hyprland.lua")
  if not f then error(lerr) end
  f()
end)
if not ok then err("hyprland.lua falhou: %s", tostring(e)) end

-- Roda os callbacks (hyprland.start, timers) para validar o corpo.
for _, c in ipairs(callbacks) do
  local ok2, e2 = pcall(c.cb)
  if not ok2 then err("callback de %s falhou: %s", c.event, tostring(e2)) end
end

return { errors = errors, report = report }
"""


def to_lua_set(lua, items):
    t = lua.table()
    for i in items:
        t[i] = True
    return t


def run_once(stub, config_dir):
    lua = LuaRuntime(unpack_returned_tuples=True)
    V = lua.table()
    V["api"] = to_lua_set(lua, stub["api"])
    dsp = lua.table()
    for k, v in stub["dsp"].items():
        dsp[k] = to_lua_set(lua, v)
    V["dsp"] = dsp
    cfg = lua.table()
    for k, v in stub["cfg"].items():
        cfg[k] = v
    V["cfg"] = cfg
    for name in ("bind_opts", "monitor", "gesture", "layer", "ws_rule"):
        V[name] = to_lua_set(lua, stub[name])
    V["match_props"] = to_lua_set(lua, MATCH_PROPS)
    V["window_effects"] = to_lua_set(lua, WINDOW_EFFECTS)
    V["events"] = to_lua_set(lua, EVENTS)
    V["mods"] = to_lua_set(lua, MODS)
    ls = lua.table()
    for leaf, styles in LEAF_STYLES.items():
        ls[leaf] = to_lua_set(lua, styles)
    V["leaf_styles"] = ls
    result = lua.execute(LUA_MOCK, V, str(config_dir))
    errors = [str(e) for e in result["errors"].values()]
    rep = result["report"]
    binds = [(b["keys"], b["description"], b["submap"]) for b in rep["binds"].values()]
    regexes = [(w, s) for w, s in ((r[1], r[2]) for r in rep["regexes"].values())]
    anims = sorted(str(k) for k in rep["anims"].keys())
    cfg_set = {str(k): v for k, v in rep["config"].items()}
    return errors, binds, regexes, anims, cfg_set


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--repo", default=str(HERE.parent.parent))
    ap.add_argument("--clios", default=None)
    a = ap.parse_args()
    repo = pathlib.Path(a.repo)
    clios = a.clios or str(repo / "target/debug/clios")
    stub = load_stub()

    failures = 0
    for mode, motion in itertools.product(("dark", "light"), ("full", "reduced", "off")):
        with tempfile.TemporaryDirectory() as home:
            def c(*args):
                return subprocess.run([clios, "--root", str(repo), "--home", home, *args],
                                      capture_output=True, text=True, check=True)
            c("sync", "--copy", "--no-theme")
            c("theme", "set", "--mode", mode, "--motion", motion)
            cfg_dir = pathlib.Path(home) / ".config/hypr"
            errors, binds, regexes, anims, cfg = run_once(stub, cfg_dir)

            # Regras do projeto (não da API):
            for keys, desc, submap in binds:
                if not desc:
                    errors.append(f"atalho {keys!r} sem description (o hub usa isso como documentação)")
            # O guia de boas-vindas mostra config/clios/keys.toml: cada atalho citado tem que existir de verdade.
            real = {keys for keys, _, submap in binds if not submap}
            for g in tomllib.loads((repo / "config/clios/keys.toml").read_text())["group"]:
                for k in g["key"]:
                    for b in k["bind"]:
                        if b not in real:
                            errors.append(f"keys.toml ({g['id']}): {b!r} não existe em binds.lua")
            for where, rx in regexes:
                try:
                    re.compile(rx)
                except re.error as e:
                    errors.append(f"regex inválida em {where}: {rx!r} ({e})")
            if motion == "off" and anims:
                errors.append(f"movimento off não deveria registrar animações, mas registrou {anims}")
            if motion != "off" and "windows" not in anims:
                errors.append("movimento ligado mas sem animação de janelas")
            if motion != "off" and cfg.get("animations.enabled") is not True:
                errors.append("movimento ligado mas animations.enabled != true")
            if motion == "off" and cfg.get("animations.enabled") is not False:
                errors.append("movimento off mas animations.enabled != false")

            label = f"{mode:5s} · movimento {motion:7s}"
            if errors:
                failures += 1
                print(f"✗ {label}")
                for e in errors:
                    print(f"    {e}")
            else:
                print(f"✓ {label}  {len(binds)} atalhos, {len(anims)} animações, {len(cfg)} opções")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
