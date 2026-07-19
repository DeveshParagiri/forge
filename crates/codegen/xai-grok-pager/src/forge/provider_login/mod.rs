//! Pi-style multi-provider `/login` (SpaceXAI / Codex / OpenRouter).

mod dispatch;
mod input;
mod render;

pub(crate) use dispatch::{
    dispatch_choose_provider_login, dispatch_openrouter_key_submitted,
    dispatch_provider_login_selected,
};
pub(crate) use input::{
    esc_cancels_direct_input, esc_cancels_provider_dialog, is_direct_input_kind,
    is_provider_picker_kind, translate_provider_login_submit,
};
pub(crate) use render::{FooterLeftPolicy, direct_input_prefix, enter_label, footer_left_policy};
