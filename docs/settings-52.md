# Native settings structure (0.52.0)

`probe/src/settings.rs` contains persisted values, validation, choice/slider definitions, keybind definitions, chord labels and conflict resolution. It has no native UI handles. Add an option to OPTIONS with its page, section, label, hint, control type and default. Existing pages automatically populate rows and scroll when needed. Add a binding to BINDINGS, then connect its semantic action in platform_input and the relevant consumer.

`probe/src/settings_ui.rs` owns the draft, page, scrolling, binding capture, dropdown and conflict dialog. It renders native nodes from the shared definitions. Opening acquires a pause immediately only for a running owned match. Resume on close requires the same match and generation, a pause caused by this window, and a still-paused session. Previously paused sessions stay paused; returning to AI or changing sessions cannot be resumed by an old window. Apply validates essential commands; Cancel/close/Escape discard the draft.

`probe/src/platform_input.rs` maps physical states to semantic inputs. More specific chords suppress base commands (Shift + Mouse2 suppresses plain Mouse2). Primary and secondary bindings are supported. Normal polling queries only configured keys; the full virtual-key set is read while capturing/editing settings. Gameplay and native spectator key handling are gated while settings is open; camera movement and wheel zoom are blocked, and wheel events scroll the settings list.

`controls.json` remains in the existing per-user mod folder. Its versioned object retains cursor_size, low_health_effect and unknown fields. Missing new options use defaults; Apply materializes all declared values and bindings, preserving future choices. Existing malformed or oversized files are not overwritten. In-memory settings work even when saving fails, with a logged retry.

Champion-only defaults to Hold. Attack-move defaults to ignoring champion-only and choosing the nearest enemy to the champion, within its acquisition area. Near cursor and Honor mode are optional. Shift + right-click issues a persistent AttackMove destination. A-click preserves explicit enemy selection when clicked directly; ground A-click uses the same acquisition policy. Native combat validation, cooldowns and post-hit attack release remain authoritative.

The more menu retains immediate vision and Return to AI actions; its Settings entry replaces cursor/low-health preferences. All four settings pages use the approved warm panel palette, packaged fonts and keyboard/panel-layout icons. Keybinds scroll six visible rows at a time, using the wheel or side arrows.

Automated checks verify state/input logic and assets. Actual native rendering, interaction and gameplay require user testing; HTML and template render captures are not in-game proof.
