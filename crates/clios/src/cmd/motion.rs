//! `clios motion`: quanto movimento o desktop faz.

use anyhow::Result;
use clios_core::MotionLevel;

use super::theme;
use crate::ctx::Ctx;

pub fn run(ctx: &mut Ctx, level: Option<MotionLevel>) -> Result<()> {
    match level {
        Some(l) => theme::set(ctx, None, None, Some(l)),
        None => {
            let next = ctx.state.motion.cycled();
            theme::set(ctx, None, None, Some(next))
        }
    }
}
