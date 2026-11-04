use crate::Fault;

use super::{
    AppContext, Pinch, ad_is_showing, adjust_track_event, batch_task_enqueue, comeback_notification_schedule,
    event_calendar_notification_schedule, expedition_notification_schedule, get_battle_status, get_inquiry_code,
    notification_schedule, onegai_notification_schedule, option_menu_open, set_bgm_duck, set_keep_awake, sound_is_paused,
    sound_manager, sound_resume_all, sound_resume_task_make, tapjoy_is_showing, time_manager_request,
};

pub fn app_on_focus_gained(ctx: &mut AppContext) -> Result<(), Fault> {
    let scene = ctx.i32_at(AppContext::SCENE_ID)?;
    let menu = scene.wrapping_sub(0x61) as u32;
    let mut awake = 1u8;

    if !(menu <= 4 && (0x13u32 >> menu) & 1 != 0) && !(scene == 0x63 && ctx.i32_at(AppContext::SCENE_0X63_STATE)? != 4) {
        let mut running = false;

        if scene == 0x12c && (get_battle_status(ctx)? == 0 || get_battle_status(ctx)? == 3) {
            running = ctx.u8_at(AppContext::OPTION_MENU_IS_OPEN)? == 0 && ctx.u8_at(AppContext::UNIT_INFO_OVERLAY_OPEN)? == 0;
        }

        if !running {
            awake = 0;
        }
    }

    set_keep_awake(ctx, awake)?;

    let mut showing = true;

    if !ad_is_showing(ctx)? {
        showing = tapjoy_is_showing(ctx)?;
    }

    if ctx.u8_at(AppContext::SOUNDS_LOADED)? != 0 {
        if ctx.i32_at(AppContext::SCENE_ID)? == 0x61 && sound_is_paused(sound_manager(ctx)?) {
            sound_resume_all(ctx)?;
            showing = true;
        }

        let task = sound_resume_task_make(ctx, !showing)?;

        batch_task_enqueue(ctx, task)?;
    }

    ctx.set_block_at::<1>(AppContext::PINCH + Pinch::ACTIVE, [0])?;
    ctx.set_block_at::<2>(AppContext::PINCH + Pinch::FIRST_DOWN, [0, 0])?;
    ctx.set_block_at::<1>(AppContext::FOCUS_CLEARED_BYTE, [0])?;
    ctx.set_i32_at(AppContext::TOUCH_BEGAN, 0)?;
    ctx.set_block_at::<2>(AppContext::TOUCH_PENDING_RELEASED, [0, 0])?;

    if ctx.i32_at(AppContext::SCENE_ID)? == 0x12c
        && ctx.u8_at(AppContext::OPTION_MENU_IS_OPEN)? == 0
        && ctx.u8_at(AppContext::TUTORIAL_POPUP_OPEN)? == 0
        && ctx.u8_at(AppContext::CAT_GOD_MENU_IS_OPEN)? == 0
        && get_battle_status(ctx)? == 0
        && ctx.u8_at(AppContext::UNIT_INFO_OVERLAY_OPEN)? == 0
    {
        ctx.set_block_at::<1>(AppContext::OPTION_MENU_IS_OPEN, [1])?;
        option_menu_open(ctx)?;

        if ctx.i32_at(AppContext::BGM_SWITCH_STATE)? == 0 {
            set_bgm_duck(sound_manager(ctx)?, 0x32);
        }
    }

    expedition_notification_schedule(ctx, 0, 0, 1)?;
    expedition_notification_schedule(ctx, 1, 0, 1)?;
    onegai_notification_schedule(ctx, 0, 1)?;
    event_calendar_notification_schedule(ctx, 0, 1)?;
    notification_schedule(ctx, 0, 1)?;
    comeback_notification_schedule(ctx, 0, 1)?;

    if ctx.u8_at(AppContext::EX_REDIRECT_ENABLED)? != 0 {
        time_manager_request(ctx)?;
        ctx.set_block_at::<1>(AppContext::NOTIFICATIONS_SCHEDULED, [0])?;
    }

    let code = get_inquiry_code(ctx)?;

    adjust_track_event(ctx, b"nzwxtv", &code)
}
