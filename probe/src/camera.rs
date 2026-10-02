//! Current-build camera geometry and control policy. No simulation writes.
use crate::{platform_input::Keys, Logger};
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}
impl Rect {
    pub fn contains(self, p: (f32, f32)) -> bool {
        p.0 >= self.x && p.1 >= self.y && p.0 < self.x + self.w && p.1 < self.y + self.h
    }
    pub fn valid(self) -> bool {
        [self.x, self.y, self.w, self.h]
            .into_iter()
            .all(f32::is_finite)
            && self.w > 0.0
            && self.h > 0.0
            && self.w <= 4096.0
            && self.h <= 4096.0
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraFrame {
    pub viewport: Rect,
    pub center: (f32, f32),
    pub extent: (f32, f32),
    pub minimap: Rect,
}
impl CameraFrame {
    pub fn valid(self) -> bool {
        self.viewport.valid()
            && self.minimap.valid()
            && [self.center.0, self.center.1, self.extent.0, self.extent.1]
                .into_iter()
                .all(f32::is_finite)
            && self.extent.0 > 0.0
            && self.extent.1 > 0.0
            && self.extent.0 <= 8192.0
            && self.extent.1 <= 8192.0
    }
    /// Native Game texture is 2048 square, cropped at its center into UI.
    /// Its camera extent is NOT the extent of the visible cropped viewport.
    pub fn unproject(self, point: (f32, f32)) -> Option<(u64, u64)> {
        if !self.valid() || !self.viewport.contains(point) || self.minimap.contains(point) {
            return None;
        }
        let x = self.center.0
            + (point.0 - self.viewport.x - self.viewport.w / 2.0) * self.extent.0 / 2048.0;
        let y = self.center.1
            + (point.1 - self.viewport.y - self.viewport.h / 2.0) * self.extent.1 / 2048.0;
        if !(0.0..=960.0).contains(&x) || !(0.0..=960.0).contains(&y) {
            return None;
        }
        Some(((x * 1000.0).round() as u64, (y * 1000.0).round() as u64))
    }
    pub fn project(self, point: (u64, u64)) -> Option<(f32, f32)> {
        if !self.valid() {
            return None;
        }
        let result = self.project_unclipped(point.0 as f32 / 1000., point.1 as f32 / 1000.);
        (self.viewport.contains(result) && !self.minimap.contains(result)).then_some(result)
    }
    pub fn project_unclipped(self, x: f32, y: f32) -> (f32, f32) {
        (
            self.viewport.x + self.viewport.w / 2.0 + (x - self.center.0) * 2048.0 / self.extent.0,
            self.viewport.y + self.viewport.h / 2.0 + (y - self.center.1) * 2048.0 / self.extent.1,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Request {
    Free((f32, f32)),
    Follow(usize),
}
#[derive(Default)]
struct State {
    frame: Option<(CameraFrame, Instant)>,
    blocked: Vec<Rect>,
    command_blocked: Vec<Rect>,
    engaged: bool,
    running: bool,
    locked: bool,
    previous_y: bool,
    previous_middle: bool,
    drag_active: bool,
    previous_cursor: Option<(f32, f32)>,
    expected_follow: bool,
    interrupted_space: bool,
}
#[derive(Default)]
pub struct CameraControl {
    state: Mutex<State>,
}
impl CameraControl {
    pub fn set_blocked(&self, rects: Vec<Rect>) {
        if let Ok(mut s) = self.state.lock() {
            s.blocked = rects;
            s.command_blocked = s.blocked.clone();
        }
    }
    pub fn set_command_blocked(&self, rects: Vec<Rect>) {
        if let Ok(mut s) = self.state.lock() {
            s.command_blocked = rects;
        }
    }
    pub fn command_blocked(&self, p: (f32, f32)) -> bool {
        self.state
            .lock()
            .map_or(true, |s| s.command_blocked.iter().any(|r| r.contains(p)))
    }
    pub fn capture(&self, frame: CameraFrame) {
        if let Ok(mut s) = self.state.lock() {
            s.frame = frame.valid().then_some((frame, Instant::now()));
        }
    }
    pub fn frame(&self) -> Option<CameraFrame> {
        let s = self.state.lock().ok()?;
        s.frame
            .filter(|(_, t)| t.elapsed() <= Duration::from_millis(250))
            .map(|(f, _)| f)
    }
    pub fn reset(&self) {
        if let Ok(mut s) = self.state.lock() {
            *s = State::default();
        }
    }
    pub fn reset_session(&self, keys: Keys) {
        if let Ok(mut s) = self.state.lock() {
            *s = State {
                previous_y: keys.camera_toggle,
                previous_middle: keys.middle,
                interrupted_space: keys.space,
                ..State::default()
            };
        }
    }
    pub fn blocked(&self, p: (f32, f32)) -> bool {
        self.state
            .lock()
            .map_or(true, |s| s.blocked.iter().any(|r| r.contains(p)))
    }
    pub fn describe(&self) -> &'static str {
        self.state.lock().map_or("camera unavailable", |s| {
            if s.locked {
                "camera locked"
            } else if s.expected_follow {
                "camera following (Space)"
            } else {
                "camera free"
            }
        })
    }
    pub fn locked(&self) -> bool {
        self.state
            .lock()
            .is_ok_and(|s| s.locked || s.expected_follow)
    }
    /// Called only inside the bound viewer's native borrow, including paused
    /// updates. The caller applies this request through native camera config.
    #[allow(clippy::too_many_arguments)]
    pub fn step(
        &self,
        frame: CameraFrame,
        native_mode: u32,
        keys: Keys,
        player: usize,
        champion: impl Into<Option<(u64, u64)>>,
        running: bool,
        dt: f32,
        log: &Logger,
    ) -> Option<Request> {
        let champion = champion.into();
        let mut s = self.state.lock().ok()?;
        let mut recenter = !s.engaged || running && !s.running;
        if !s.engaged {
            s.engaged = true;
            s.locked = false;
        }
        let minimap_navigation =
            keys.focused && keys.left && keys.cursor.is_some_and(|p| frame.minimap.contains(p));
        if s.expected_follow && native_mode == 1 && minimap_navigation && !recenter {
            s.locked = false;
            s.expected_follow = false;
            s.interrupted_space = keys.space;
            log.write("CAMERA explicit minimap navigation; unlocked");
        }
        if keys.focused && keys.camera_toggle && !s.previous_y {
            s.locked = !s.locked;
            s.interrupted_space = false;
            log.write(&format!("CAMERA Y locked={}", s.locked));
        }
        if !keys.space {
            s.interrupted_space = false;
        }
        let mut request = None;
        let mut drag = false;
        if !keys.focused || !keys.middle {
            s.drag_active = false;
        } else if !s.previous_middle {
            s.drag_active = keys.cursor.is_some_and(|p| {
                frame.viewport.contains(p)
                    && !frame.minimap.contains(p)
                    && !s.blocked.iter().any(|r| r.contains(p))
            });
        }
        if s.drag_active && s.previous_middle {
            if let (Some(current), Some(previous)) = (keys.cursor, s.previous_cursor) {
                let delta = (current.0 - previous.0, current.1 - previous.1);
                if delta != (0.0, 0.0) {
                    s.locked = false;
                    s.interrupted_space = keys.space;
                    request = Some(Request::Free((
                        frame.center.0 - delta.0 * frame.extent.0 / 2048.0,
                        frame.center.1 - delta.1 * frame.extent.1 / 2048.0,
                    )));
                    drag = true;
                    recenter = false;
                }
            }
        }
        let following =
            champion.is_some() && (s.locked || keys.focused && keys.space && !s.interrupted_space);
        if following && !drag {
            request = Some(Request::Follow(player));
        } else if let Some(champion) = champion.filter(|_| recenter) {
            request = Some(Request::Free((
                champion.0 as f32 / 1000.0,
                champion.1 as f32 / 1000.0,
            )));
        } else if !drag {
            if s.expected_follow || native_mode != 1 {
                request = Some(Request::Free(frame.center));
            }
            if keys.focused && !keys.middle {
                if let Some(p) = keys
                    .cursor
                    .filter(|p| (0.0..1920.0).contains(&p.0) && (0.0..1080.0).contains(&p.1))
                {
                    let dx = if p.0 < 12.0 {
                        -1.0
                    } else if p.0 >= 1908.0 {
                        1.0
                    } else {
                        0.0
                    };
                    let dy = if p.1 < 12.0 {
                        -1.0
                    } else if p.1 >= 1068.0 {
                        1.0
                    } else {
                        0.0
                    };
                    if dx != 0.0 || dy != 0.0 {
                        let elapsed = if dt.is_finite() {
                            dt.clamp(0.0, 0.05)
                        } else {
                            0.0
                        };
                        request = Some(Request::Free((
                            frame.center.0 + dx * 800.0 * elapsed * frame.extent.0 / 2048.0,
                            frame.center.1 + dy * 800.0 * elapsed * frame.extent.1 / 2048.0,
                        )));
                    }
                }
            }
        }
        s.expected_follow = following && !drag;
        s.previous_y = keys.camera_toggle;
        s.previous_middle = keys.middle;
        s.previous_cursor = keys.cursor;
        s.running = running;
        request
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame() -> CameraFrame {
        CameraFrame {
            viewport: Rect {
                x: 0.,
                y: 50.,
                w: 1920.,
                h: 974.,
            },
            center: (480., 480.),
            extent: (1024., 1024.),
            minimap: Rect {
                x: 1581.,
                y: 740.,
                w: 320.,
                h: 320.,
            },
        }
    }
    #[test]
    fn next_session_clears_lock_and_drag_without_replaying_held_camera_keys() {
        let camera = CameraControl::default();
        let log = crate::timing_test::tests::logger("camera-session-reset");
        let f = frame();
        let held = Keys {
            focused: true,
            camera_toggle: true,
            middle: true,
            space: true,
            cursor: Some((960., 537.)),
            ..Keys::default()
        };
        camera.capture(f);
        camera.step(f, 1, held, 7, (200_000, 200_000), true, 0.016, &log);
        camera.reset_session(held);
        assert!(camera.frame().is_none());
        camera.step(f, 1, held, 4, (300_000, 400_000), false, 0.016, &log);
        assert_eq!(camera.describe(), "camera free");
        assert!(!camera.state.lock().unwrap().drag_active);
        camera.step(
            f,
            1,
            Keys {
                focused: true,
                ..Keys::default()
            },
            4,
            (300_000, 400_000),
            false,
            0.016,
            &log,
        );
        camera.step(
            f,
            1,
            Keys {
                focused: true,
                camera_toggle: true,
                ..Keys::default()
            },
            4,
            (300_000, 400_000),
            false,
            0.016,
            &log,
        );
        assert_eq!(camera.describe(), "camera locked");
    }
    #[test]
    fn cropped_texture_picking_tracks_pan_zoom_and_roundtrips() {
        let f = frame();
        assert_eq!(f.unproject((960., 537.)), Some((480_000, 480_000)));
        assert_eq!(f.unproject((1060., 637.)), Some((530_000, 530_000)));
        assert_eq!(f.project((530_000, 530_000)), Some((1060., 637.)));
        let panned = CameraFrame {
            center: (580., 280.),
            ..f
        };
        assert_eq!(panned.unproject((1060., 637.)), Some((630_000, 330_000)));
        let zoomed = CameraFrame {
            extent: (512., 512.),
            ..panned
        };
        assert_eq!(zoomed.unproject((1060., 637.)), Some((605_000, 305_000)));
        let narrow = CameraFrame {
            viewport: Rect {
                x: 942.,
                y: 50.,
                w: 978.,
                h: 974.,
            },
            ..f
        };
        assert_eq!(narrow.unproject((1431., 537.)), Some((480_000, 480_000)));
    }
    #[test]
    fn rejects_ui_minimap_nonfinite_and_off_map_points() {
        let f = frame();
        assert_eq!(f.unproject((960., 20.)), None);
        assert_eq!(f.unproject((1600., 800.)), None);
        assert_eq!(f.unproject((f32::NAN, 100.)), None);
        assert!(!CameraFrame {
            extent: (0., 1.),
            ..f
        }
        .valid());
        assert_eq!(
            CameraFrame {
                center: (0., 0.),
                ..f
            }
            .unproject((100., 100.)),
            None
        );
    }
    #[test]
    fn paused_drag_unlocks_and_survives_crossing_hud() {
        let camera = CameraControl::default();
        let log = crate::timing_test::tests::logger("camera-drag");
        let k = Keys {
            focused: true,
            cursor: Some((960., 537.)),
            ..Keys::default()
        };
        let f = frame();
        camera.step(f, 0, k, 7, (200_000, 300_000), false, 0.016, &log);
        camera.step(
            f,
            1,
            Keys {
                camera_toggle: true,
                ..k
            },
            7,
            (200_000, 300_000),
            false,
            0.016,
            &log,
        );
        camera.step(
            f,
            2,
            Keys { middle: true, ..k },
            7,
            (200_000, 300_000),
            false,
            0.016,
            &log,
        );
        assert_eq!(
            camera.step(
                f,
                2,
                Keys {
                    middle: true,
                    cursor: Some((1060., 537.)),
                    ..k
                },
                7,
                (200_000, 300_000),
                false,
                0.,
                &log
            ),
            Some(Request::Free((430., 480.)))
        );
        assert_eq!(camera.describe(), "camera free");
        camera.set_blocked(vec![Rect {
            x: 900.,
            y: 500.,
            w: 100.,
            h: 100.,
        }]);
        camera.step(
            f,
            1,
            Keys { middle: true, ..k },
            7,
            (200_000, 300_000),
            false,
            0.016,
            &log,
        );
        assert_eq!(
            camera.step(
                f,
                1,
                Keys {
                    middle: true,
                    cursor: Some((1100., 537.)),
                    ..k
                },
                7,
                (200_000, 300_000),
                false,
                0.016,
                &log
            ),
            Some(Request::Free((410., 480.)))
        );
    }
    #[test]
    fn death_keeps_drag_and_edge_pan_without_following_an_absent_champion() {
        let camera = CameraControl::default();
        let log = crate::timing_test::tests::logger("camera-dead-drag");
        let f = frame();
        let keys = Keys {
            focused: true,
            cursor: Some((960., 537.)),
            ..Default::default()
        };
        camera.step(f, 1, keys, 7, (200_000, 300_000), true, 0.016, &log);
        camera.step(
            f,
            1,
            Keys {
                camera_toggle: true,
                ..keys
            },
            7,
            (200_000, 300_000),
            true,
            0.016,
            &log,
        );
        assert_eq!(
            camera.step(f, 2, keys, 7, None, true, 0.016, &log),
            Some(Request::Free(f.center))
        );
        assert_eq!(camera.describe(), "camera locked"); // Keep intent until deliberate pan.
        camera.step(
            f,
            1,
            Keys {
                middle: true,
                space: true,
                ..keys
            },
            7,
            None,
            true,
            0.016,
            &log,
        );
        camera.set_blocked(vec![Rect {
            x: 1000.,
            y: 500.,
            w: 100.,
            h: 100.,
        }]);
        assert_eq!(
            camera.step(
                f,
                1,
                Keys {
                    middle: true,
                    space: true,
                    cursor: Some((1060., 537.)),
                    ..keys
                },
                7,
                None,
                false,
                0.,
                &log
            ),
            Some(Request::Free((430., 480.)))
        );
        assert_eq!(camera.describe(), "camera free");
        // A drag interrupted held-Space intent; respawn must not unexpectedly snap.
        assert_eq!(
            camera.step(
                f,
                1,
                Keys {
                    space: true,
                    cursor: Some((1060., 537.)),
                    ..keys
                },
                7,
                (900_000, 30_000),
                false,
                0.016,
                &log
            ),
            None
        );
        camera.step(f, 1, keys, 7, None, true, 0.016, &log);
        assert_eq!(
            camera.step(
                f,
                1,
                Keys {
                    cursor: Some((1., 537.)),
                    ..keys
                },
                7,
                None,
                true,
                0.02,
                &log
            ),
            Some(Request::Free((472., 480.)))
        );
    }
    #[test]
    fn death_retains_lock_intent_for_respawn_but_does_not_capture_hud_drags() {
        let camera = CameraControl::default();
        let log = crate::timing_test::tests::logger("camera-dead-lock");
        let f = frame();
        let keys = Keys {
            focused: true,
            cursor: Some((960., 537.)),
            ..Default::default()
        };
        camera.step(f, 1, keys, 7, None, true, 0.016, &log);
        camera.step(
            f,
            1,
            Keys {
                camera_toggle: true,
                ..keys
            },
            7,
            None,
            true,
            0.016,
            &log,
        );
        assert_eq!(camera.step(f, 1, keys, 7, None, true, 0.016, &log), None);
        assert_eq!(
            camera.step(f, 1, keys, 7, (900_000, 30_000), true, 0.016, &log),
            Some(Request::Follow(7))
        );
        camera.reset();
        camera.set_blocked(vec![Rect {
            x: 900.,
            y: 500.,
            w: 100.,
            h: 100.,
        }]);
        camera.step(f, 1, keys, 7, None, true, 0.016, &log);
        camera.step(
            f,
            1,
            Keys {
                middle: true,
                ..keys
            },
            7,
            None,
            true,
            0.016,
            &log,
        );
        assert_eq!(
            camera.step(
                f,
                1,
                Keys {
                    middle: true,
                    cursor: Some((1060., 537.)),
                    ..keys
                },
                7,
                None,
                true,
                0.016,
                &log
            ),
            None
        );
    }
    #[test]
    fn held_space_survives_native_mode_changes_and_releases_in_place() {
        let camera = CameraControl::default();
        let log = crate::timing_test::tests::logger("space-follow");
        let k = Keys {
            focused: true,
            ..Keys::default()
        };
        let f = frame();
        camera.step(f, 0, k, 7, (200_000, 300_000), true, 0.016, &log);
        for mode in [1, 2, 1, 0, 2] {
            assert_eq!(
                camera.step(
                    f,
                    mode,
                    Keys { space: true, ..k },
                    7,
                    (200_000, 300_000),
                    true,
                    0.016,
                    &log
                ),
                Some(Request::Follow(7))
            );
        }
        assert_eq!(
            camera.step(f, 2, k, 7, (200_000, 300_000), true, 0.016, &log),
            Some(Request::Free(f.center))
        );
    }
    #[test]
    fn edge_scroll_uses_window_edges_even_over_hud_and_minimap() {
        let camera = CameraControl::default();
        let log = crate::timing_test::tests::logger("outer-edges");
        let k = Keys {
            focused: true,
            ..Keys::default()
        };
        let f = frame();
        camera.step(f, 0, k, 7, (200_000, 300_000), true, 0.016, &log);
        camera.set_blocked(vec![
            Rect {
                x: 0.,
                y: 0.,
                w: 1920.,
                h: 100.,
            },
            Rect {
                x: 0.,
                y: 1000.,
                w: 1920.,
                h: 80.,
            },
        ]);
        for (cursor, expected) in [
            ((960., 2.), (480., 473.6)),
            ((960., 1078.), (480., 486.4)),
            ((1918., 800.), (486.4, 480.)),
            ((2., 537.), (473.6, 480.)),
        ] {
            assert_eq!(
                camera.step(
                    f,
                    1,
                    Keys {
                        cursor: Some(cursor),
                        ..k
                    },
                    7,
                    (200_000, 300_000),
                    true,
                    0.016,
                    &log
                ),
                Some(Request::Free(expected))
            );
        }
        assert_eq!(
            camera.step(
                f,
                1,
                Keys {
                    cursor: Some((960., 60.)),
                    ..k
                },
                7,
                (200_000, 300_000),
                true,
                0.016,
                &log
            ),
            None
        );
        camera.step(
            f,
            1,
            Keys {
                camera_toggle: true,
                ..k
            },
            7,
            (200_000, 300_000),
            true,
            0.016,
            &log,
        );
        assert_eq!(
            camera.step(
                f,
                2,
                Keys {
                    cursor: Some((960., 2.)),
                    ..k
                },
                7,
                (200_000, 300_000),
                true,
                0.016,
                &log
            ),
            Some(Request::Follow(7))
        );
    }
    #[test]
    fn drag_started_on_hud_never_captures_and_release_ends_capture() {
        let camera = CameraControl::default();
        let log = crate::timing_test::tests::logger("drag-capture");
        let k = Keys {
            focused: true,
            cursor: Some((960., 537.)),
            ..Keys::default()
        };
        let f = frame();
        camera.step(f, 0, k, 7, (200_000, 300_000), false, 0.016, &log);
        camera.set_blocked(vec![Rect {
            x: 900.,
            y: 500.,
            w: 100.,
            h: 100.,
        }]);
        camera.step(
            f,
            1,
            Keys { middle: true, ..k },
            7,
            (200_000, 300_000),
            false,
            0.016,
            &log,
        );
        assert_eq!(
            camera.step(
                f,
                1,
                Keys {
                    middle: true,
                    cursor: Some((1100., 537.)),
                    ..k
                },
                7,
                (200_000, 300_000),
                false,
                0.016,
                &log
            ),
            None
        );
        camera.step(
            f,
            1,
            Keys {
                cursor: Some((1100., 537.)),
                ..k
            },
            7,
            (200_000, 300_000),
            false,
            0.016,
            &log,
        );
        camera.step(
            f,
            1,
            Keys {
                middle: true,
                cursor: Some((1100., 537.)),
                ..k
            },
            7,
            (200_000, 300_000),
            false,
            0.016,
            &log,
        );
        assert_eq!(
            camera.step(
                f,
                1,
                Keys { middle: true, ..k },
                7,
                (200_000, 300_000),
                false,
                0.016,
                &log
            ),
            Some(Request::Free((550., 480.)))
        );
        assert_eq!(
            camera.step(f, 1, k, 7, (200_000, 300_000), false, 0.016, &log),
            None
        );
        assert_eq!(
            camera.step(
                f,
                1,
                Keys {
                    cursor: Some((1200., 537.)),
                    ..k
                },
                7,
                (200_000, 300_000),
                false,
                0.016,
                &log
            ),
            None
        );
    }
    #[test]
    fn temporary_follow_releases_in_place_and_explicit_minimap_unlocks() {
        let camera = CameraControl::default();
        let log = crate::timing_test::tests::logger("camera");
        let k = Keys {
            focused: true,
            ..Keys::default()
        };
        let f = frame();
        assert_eq!(
            camera.step(f, 0, k, 7, (200_000, 300_000), true, 0.016, &log),
            Some(Request::Free((200., 300.)))
        );
        assert_eq!(
            camera.step(
                f,
                1,
                Keys { space: true, ..k },
                7,
                (200_000, 300_000),
                true,
                0.016,
                &log
            ),
            Some(Request::Follow(7))
        );
        assert_eq!(
            camera.step(f, 2, k, 7, (200_000, 300_000), true, 0.016, &log),
            Some(Request::Free(f.center))
        );
        assert_eq!(
            camera.step(
                f,
                1,
                Keys {
                    camera_toggle: true,
                    ..k
                },
                7,
                (200_000, 300_000),
                true,
                0.016,
                &log
            ),
            Some(Request::Follow(7))
        );
        assert_eq!(
            camera.step(f, 2, k, 7, (200_000, 300_000), true, 0.016, &log),
            Some(Request::Follow(7))
        );
        assert_eq!(
            camera.step(
                f,
                1,
                Keys {
                    left: true,
                    cursor: Some((1600., 800.)),
                    ..k
                },
                7,
                (200_000, 300_000),
                true,
                0.016,
                &log
            ),
            None
        );
        assert_eq!(camera.describe(), "camera free");
    }
}
