//! Where a ping points: the world point under a field or minimap cursor.
//! Pure functions. Superseded 2026-11-04 (1.7.1-dev21): the nearest
//! candidate on screen, for the research lever keys, went with them.
//!
//! Superseded 2026-11-04 (1.7.1-dev15): the snap rule (the sprite under
//! the cursor, else the nearest within 50 px, else a lone one within 120
//! px). The wheel never snaps to a unit now, and the armed click takes the
//! unit under the cursor (`combat::picked`).
use crate::camera::CameraFrame;

/// A minimap icon counts as "under the cursor" within this many pixels.
pub const MINIMAP_ICON: f32 = 10.;

/// The world point under a field cursor, clamped onto the map (a point off
/// the map's edge becomes the nearest edge point). None off the field.
pub fn field_point(frame: CameraFrame, p: (f32, f32)) -> Option<(u64, u64)> {
    if !frame.valid() || !frame.viewport.contains(p) || frame.minimap.contains(p) {
        return None;
    }
    let x =
        frame.center.0 + (p.0 - frame.viewport.x - frame.viewport.w / 2.) * frame.extent.0 / 2048.;
    let y =
        frame.center.1 + (p.1 - frame.viewport.y - frame.viewport.h / 2.) * frame.extent.1 / 2048.;
    clamp((x * 1000.).round(), (y * 1000.).round())
}
/// The world point under a minimap cursor (the inverse of the mapping the
/// route drawing uses, `CameraFrame::project_minimap`).
pub fn minimap_point(frame: CameraFrame, p: (f32, f32)) -> Option<(u64, u64)> {
    let (x, y) = frame.unproject_minimap(p)?;
    clamp(x as f32, y as f32)
}
fn clamp(x: f32, y: f32) -> Option<(u64, u64)> {
    (x.is_finite() && y.is_finite())
        .then(|| (x.clamp(0., 959_999.) as u64, y.clamp(0., 959_999.) as u64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::Rect;
    fn frame() -> CameraFrame {
        CameraFrame {
            viewport: Rect {
                x: 0.,
                y: 50.,
                w: 1920.,
                h: 1030.,
            },
            center: (480., 480.),
            extent: (2048., 2048.),
            zoom: 1.,
            minimap: Rect {
                x: 1564.,
                y: 724.,
                w: 352.,
                h: 352.,
            },
        }
    }
    #[test]
    fn the_minimap_point_inverts_the_route_drawing_mapping() {
        let f = frame();
        for point in [
            (0, 0),
            (240_000, 720_000),
            (480_000, 480_000),
            (900_000, 100_000),
        ] {
            let screen = f.project_minimap(point);
            let back = minimap_point(f, screen).unwrap();
            // One minimap pixel is about 2 727 world units.
            assert!(back.0.abs_diff(point.0) <= 2_000, "{point:?} {back:?}");
            assert!(back.1.abs_diff(point.1) <= 2_000, "{point:?} {back:?}");
        }
        assert!(minimap_point(f, (100., 100.)).is_none());
    }
    #[test]
    fn a_field_point_off_the_map_is_clamped_onto_it() {
        let mut f = frame();
        assert_eq!(field_point(f, (960., 565.)), Some((480_000, 480_000)));
        f.center = (0., 0.);
        let p = field_point(f, (10., 100.)).unwrap();
        assert_eq!(p, (0, 0));
        assert!(field_point(f, (1600., 800.)).is_none()); // on the minimap
    }
}
