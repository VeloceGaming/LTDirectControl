//! Window state that input, camera and wheel code need to know about. A leaf
//! module: the windows write these flags, and nothing here depends on them.
use std::sync::atomic::AtomicBool;

/// The settings window is open: gameplay keys and battlefield clicks pause.
pub static SETTINGS_OPEN: AtomicBool = AtomicBool::new(false);
/// The shop window is open: Esc belongs to the shop (it must not also cancel
/// a recall).
pub static SHOP_OPEN: AtomicBool = AtomicBool::new(false);
/// A left-click that meant nothing else (empty ground, outside every window
/// and the minimap, no attack-move or skill waiting for it) asks the open
/// shop to close. Written by movement input, taken by the shop.
pub static SHOP_CLOSE: AtomicBool = AtomicBool::new(false);
/// Includes the closing edge and held cancel buttons, so they cannot leak.
pub static EMOTE_CAPTURE: AtomicBool = AtomicBool::new(false);
/// The "HUD tooltip (hold)" key is down; a mouse button is down. Written by
/// input polling, read by the HUD tooltip.
pub static HUD_TOOLTIP_KEY: AtomicBool = AtomicBool::new(false);
/// Ctrl is down: skill tooltips show the game's own "base + percentage"
/// wording instead of values computed from your stats.
pub static DETAIL_KEY: AtomicBool = AtomicBool::new(false);
pub static POINTER_BUTTON: AtomicBool = AtomicBool::new(false);
