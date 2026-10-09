//! Original PNG glyph references; all chrome is grayscale, with state accents.
pub fn image(id: &str, glyph: &str, x: usize, y: usize, size: usize, z: usize) -> String {
    let ink = if glyph.starts_with("ef_") {
        " color: #eeececff;"
    } else {
        ""
    };
    format!("#{id}:image {{ x: {x}px; y: {y}px; width: {size}px; height: {size}px; z: {z}; source: \"asset/lt_direct_control/ui/{glyph}\";{ink} ignore_event: true; }}\n")
}
