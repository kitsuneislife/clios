//! Renderização de templates (minijinja) com filtros de cor.
//!
//! Filtros, todos sobre strings `#RRGGBB`:
//!   hex            #RRGGBB
//!   bare           RRGGBB
//!   rgb            r, g, b
//!   hypr           rgb(RRGGBB)           (Hyprland)
//!   hypra(0.5)     rgba(RRGGBBAA)        (Hyprland, com alfa)
//!   alpha(0.5)     #RRGGBBAA
//!   osc            rgb:RR/RR/RR          (sequências OSC dos terminais)
//!   qml            "#RRGGBB"             (QML/JSON, entre aspas)
//!   bool           true | false          (minijinja imprime True/False, que Lua e JSON não entendem)

use anyhow::{Result, anyhow};
use minijinja::{Environment, Error, ErrorKind, UndefinedBehavior};
use serde::Serialize;

use crate::color::Rgb;

fn rgb(value: &str) -> Result<Rgb, Error> {
    Rgb::parse(value).map_err(|e| Error::new(ErrorKind::InvalidOperation, e.to_string()))
}

fn alpha_byte(a: f64) -> u8 {
    (a.clamp(0.0, 1.0) * 255.0).round() as u8
}

pub fn environment() -> Environment<'static> {
    let mut env = Environment::new();
    // Variável faltando é bug no template; melhor falhar alto do que escrever lixo num dotfile.
    env.set_undefined_behavior(UndefinedBehavior::Strict);
    env.set_trim_blocks(true);
    env.set_lstrip_blocks(true);
    env.set_keep_trailing_newline(true);

    env.add_filter("hex", |v: &str| rgb(v).map(Rgb::hex));
    env.add_filter("bare", |v: &str| rgb(v).map(Rgb::bare));
    env.add_filter("rgb", |v: &str| rgb(v).map(|c| format!("{}, {}, {}", c.r, c.g, c.b)));
    env.add_filter("hypr", |v: &str| rgb(v).map(|c| format!("rgb({})", c.bare())));
    env.add_filter("hypra", |v: &str, a: f64| rgb(v).map(|c| format!("rgba({}{:02X})", c.bare(), alpha_byte(a))));
    env.add_filter("alpha", |v: &str, a: f64| rgb(v).map(|c| format!("{}{:02X}", c.hex(), alpha_byte(a))));
    env.add_filter("osc", |v: &str| rgb(v).map(|c| format!("rgb:{:02x}/{:02x}/{:02x}", c.r, c.g, c.b)));
    env.add_filter("bool", |v: bool| if v { "true" } else { "false" });
    env.add_filter("qml", |v: &str| rgb(v).map(|c| format!("\"{}\"", c.hex())));
    env
}

pub fn render<S: Serialize>(name: &str, source: &str, ctx: &S) -> Result<String> {
    let env = environment();
    // `{:?}` do minijinja mostra a linha do template e a variável problemática.
    env.render_str(source, ctx).map_err(|e| anyhow!("renderizando {name}:\n{e:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use crate::tokens::{Mode, MotionLevel, Tokens};

    fn ctx() -> Theme {
        Theme::resolve(&Tokens::builtin(), Mode::Dark, "ember", MotionLevel::Full).unwrap()
    }

    fn r(src: &str) -> String {
        render("teste", src, &ctx()).unwrap()
    }

    #[test]
    fn color_filters() {
        assert_eq!(r("{{ c.accent | hex }}"), "#FF5A1F");
        assert_eq!(r("{{ c.accent | bare }}"), "FF5A1F");
        assert_eq!(r("{{ c.accent | rgb }}"), "255, 90, 31");
        assert_eq!(r("{{ c.accent | hypr }}"), "rgb(FF5A1F)");
        assert_eq!(r("{{ c.accent | hypra(0.5) }}"), "rgba(FF5A1F80)");
        assert_eq!(r("{{ c.accent | alpha(1.0) }}"), "#FF5A1FFF");
        assert_eq!(r("{{ c.accent | osc }}"), "rgb:ff/5a/1f");
        assert_eq!(r("{{ c.accent | qml }}"), "\"#FF5A1F\"");
    }

    #[test]
    fn booleans_are_lowercase_for_lua_and_json() {
        assert_eq!(r("{{ motion.enabled | bool }}"), "true");
        assert_eq!(r("{{ false | bool }}"), "false");
        // O comportamento padrão do minijinja, que é exatamente a armadilha:
        assert_eq!(r("{{ motion.enabled }}"), "True");
    }

    #[test]
    fn numbers_and_loops() {
        assert_eq!(r("{{ motion.duration.base }}"), "240");
        let out = r("{% for name, curve in motion.curve | items %}{{ name }}={{ curve.x1 }};{% endfor %}");
        assert!(out.contains("out=0.16;"), "{out}");
        assert!(out.contains("linear=0.0;"), "{out}");
    }

    #[test]
    fn missing_variable_is_an_error_not_an_empty_string() {
        let err = render("t", "{{ c.nao_existe }}", &ctx()).unwrap_err();
        assert!(format!("{err:#}").contains("nao_existe"), "{err:#}");
    }

    #[test]
    fn bad_color_input_reports_the_value() {
        let err = render("t", "{{ 'banana' | hex }}", &ctx()).unwrap_err();
        assert!(format!("{err:#}").contains("banana"), "{err:#}");
    }

    #[test]
    fn keeps_trailing_newline() {
        assert!(r("a\n").ends_with('\n'));
    }
}
