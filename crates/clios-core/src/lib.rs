//! Núcleo do CLIOS: tokens de design, tema resolvido e o motor que espalha o tema
//! por todos os apps. Sem terminal, sem rede, sem processos: só lógica testável.

pub mod color;
pub mod fsutil;
pub mod paths;
pub mod state;
pub mod sync;
pub mod template;
pub mod terminal;
pub mod theme;
pub mod tokens;

pub use color::Rgb;
pub use paths::Paths;
pub use state::State;
pub use theme::Theme;
pub use tokens::{Mode, MotionLevel, Tokens};
