//! Island geometry Rust needs. Pure, so the property tests can hammer it.
//!
//! The window never resizes (tech.md 6.7), so the only rectangle that moves is
//! the shape drawn inside it. The webview measures what it drew and reports it
//! through `island_bounds`; this module turns that measurement into the screen
//! rectangle the resting mark occupies, which is the one place the collapsed
//! island takes a click.

/// A rectangle in physical pixels, y growing downwards, the coordinate space
/// Tauri reports both the window frame and the pointer in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Half open on the far edges, so two adjacent rectangles never both claim
    /// the same pointer.
    pub fn contains(&self, point: (f64, f64)) -> bool {
        point.0 >= self.x
            && point.0 < self.x + self.width
            && point.1 >= self.y
            && point.1 < self.y + self.height
    }

    fn is_sane(&self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.width.is_finite()
            && self.height.is_finite()
            && self.width > 0.0
            && self.height > 0.0
    }
}

/// Where the resting mark sits on screen, given the window frame and the
/// bounds the webview reported for the collapsed shape.
///
/// The shape is centred horizontally in the window and flush with its top
/// edge, which is what the route lays out and what section 6.7 fixes. Bounds
/// that never arrived, or that arrived unusable, yield nothing rather than a
/// guessed rectangle: a wrong guess eats clicks next to the mark, and that is
/// worse than a mark that is not clickable yet.
pub fn rest_rect(window: Rect, bounds: (f64, f64)) -> Option<Rect> {
    if !window.is_sane() {
        return None;
    }
    let mark = Rect::new(0.0, 0.0, bounds.0, bounds.1);
    if !mark.is_sane() {
        return None;
    }

    // Clamped rather than rejected: a shape wider than the window is clipped by
    // the webview with no error, so the clickable part is the visible part.
    let width = mark.width.min(window.width);
    let height = mark.height.min(window.height);

    Some(Rect::new(
        window.x + (window.width - width) / 2.0,
        window.y,
        width,
        height,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const WINDOW: Rect = Rect {
        x: 100.0,
        y: 0.0,
        width: 720.0,
        height: 560.0,
    };

    #[test]
    fn centres_the_mark_on_the_top_edge_of_the_window() {
        let rect = rest_rect(WINDOW, (200.0, 50.0)).expect("sane bounds give a rect");
        assert_eq!(rect, Rect::new(100.0 + 260.0, 0.0, 200.0, 50.0));
    }

    /// The error path of S12: the webview has not reported yet, so there is no
    /// hotspot at all.
    #[test]
    fn bounds_that_never_arrived_give_no_hotspot() {
        for bounds in [
            (0.0, 0.0),
            (200.0, 0.0),
            (-1.0, 50.0),
            (f64::NAN, 50.0),
            (200.0, f64::INFINITY),
        ] {
            assert_eq!(rest_rect(WINDOW, bounds), None, "{bounds:?}");
        }
    }

    #[test]
    fn a_shape_wider_than_the_window_is_clipped_to_what_is_visible() {
        let rect = rest_rect(WINDOW, (9000.0, 9000.0)).expect("clamped, not rejected");
        assert_eq!(rect, Rect::new(100.0, 0.0, 720.0, 560.0));
    }

    #[test]
    fn the_far_edges_belong_to_the_neighbour() {
        let rect = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert!(rect.contains((0.0, 0.0)));
        assert!(rect.contains((9.9, 9.9)));
        assert!(!rect.contains((10.0, 5.0)));
        assert!(!rect.contains((5.0, 10.0)));
        assert!(!rect.contains((-0.1, 5.0)));
    }
}
