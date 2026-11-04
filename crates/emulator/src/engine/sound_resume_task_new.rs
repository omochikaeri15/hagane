use std::rc::Rc;

use crate::Fault;

use super::{AppContext, Imgcut, Texture, get_text_texture, localize, text_texture_cache, texture_cache_load};

pub struct SoundResumeBatchTask {
    pub resume: bool,
    pub text: Option<Texture>,
    pub sheet: Option<Rc<Imgcut>>,
    pub progress: f32,
    pub done: bool,
}

pub fn sound_resume_task_new(ctx: &mut AppContext, resume: bool) -> Result<SoundResumeBatchTask, Fault> {
    let sheet = texture_cache_load(ctx, b"download.png", b"download.imgcut", 0x2601)?;
    let text = localize(ctx, b"loading")?;
    let font = ctx.default_font.clone();
    let text = get_text_texture(text_texture_cache(ctx)?, &text, &font, 0x18, 1, 0);

    Ok(SoundResumeBatchTask { resume, text: Some(text), sheet, progress: 0.0, done: false })
}
