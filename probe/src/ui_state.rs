//! Window state that input, camera and wheel code need to know about. A leaf
//! module: the windows write these flags, and nothing here depends on them.
use std::sync::atomic::AtomicBool;

/// The settings window is open: gameplay keys and battlefield clicks pause.
pub static SETTINGS_OPEN: AtomicBool = AtomicBool::new(false);
/// The shop window is open: Esc belongs to the shop (it must not also cancel
/// a recall).
pub static SHOP_OPEN: AtomicBool = AtomicBool::new(false);
/// Includes the closing edge and held cancel buttons, so they cannot leak.
pub static EMOTE_CAPTURE: AtomicBool = AtomicBool::new(false);
/// The "HUD tooltip (hold)" key is down; a mouse button is down. Written by
/// input polling, read by the HUD tooltip.
pub static HUD_TOOLTIP_KEY: AtomicBool = AtomicBool::new(false);
pub static POINTER_BUTTON: AtomicBool = AtomicBool::new(false);
