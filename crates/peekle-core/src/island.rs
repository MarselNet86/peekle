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

/// Where the shape sits on screen, given the window frame and the rectangle
/// the webview reported for it, in physical pixels relative to the window.
///
/// The webview measures the whole rectangle -- offset included -- and this
/// only lays it over the frame. Rust builds nothing of its own: before v82 it
/// centred the shape and pinned it to the top, which held exactly until the
/// first shape with an inset from the edge (tech.md 6.7), whose hotspot would
/// have stood ten pixels above the shape itself. Collapsed this is the resting
/// mark, the one place a resting island takes a click; open it is the area the
/// pointer has to leave before the island puts itself away. Bounds that never
/// arrived, or that arrived unusable, yield nothing rather than a guessed
/// rectangle: a wrong guess eats clicks next to the mark, and that is worse
/// than a mark that is not clickable yet.
pub fn shape_rect(window: Rect, mark: Rect) -> Option<Rect> {
    if !window.is_sane() || !mark.is_sane() {
        return None;
    }
    if !mark.x.is_finite() || !mark.y.is_finite() || mark.x < 0.0 || mark.y < 0.0 {
        return None;
    }

    // Clipped rather than rejected: a shape past the window's edge is cut by
    // the webview with no error, so the clickable part is the visible part.
    let x = mark.x.min(window.width);
    let y = mark.y.min(window.height);
    let width = mark.width.min(window.width - x);
    let height = mark.height.min(window.height - y);
    if width <= 0.0 || height <= 0.0 {
        return None;
    }

    Some(Rect::new(window.x + x, window.y + y, width, height))
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
    fn lays_the_reported_rectangle_over_the_window_frame() {
        let rect = shape_rect(WINDOW, Rect::new(260.0, 0.0, 200.0, 50.0)).expect("sane");
        assert_eq!(rect, Rect::new(360.0, 0.0, 200.0, 50.0));
    }

    /// A shape floating off the edge (tech.md 6.7) is where it was drawn, not
    /// where a rectangle pinned to the top would put it.
    #[test]
    fn keeps_the_inset_the_webview_measured() {
        let rect = shape_rect(WINDOW, Rect::new(294.0, 20.0, 132.0, 36.0)).expect("sane");
        assert_eq!(rect, Rect::new(394.0, 20.0, 132.0, 36.0));
        assert!(rect.contains((400.0, 30.0)));
        assert!(!rect.contains((400.0, 10.0)));
    }

    /// The error path of S12: the webview has not reported yet, so there is no
    /// hotspot at all.
    #[test]
    fn bounds_that_never_arrived_give_no_hotspot() {
        for mark in [
            Rect::new(0.0, 0.0, 0.0, 0.0),
            Rect::new(0.0, 0.0, 200.0, 0.0),
            Rect::new(0.0, 0.0, -1.0, 50.0),
            Rect::new(0.0, 0.0, f64::NAN, 50.0),
            Rect::new(0.0, 0.0, 200.0, f64::INFINITY),
            Rect::new(-5.0, 0.0, 200.0, 50.0),
            Rect::new(f64::NAN, 0.0, 200.0, 50.0),
            Rect::new(900.0, 0.0, 200.0, 50.0),
        ] {
            assert_eq!(shape_rect(WINDOW, mark), None, "{mark:?}");
        }
    }

    #[test]
    fn a_shape_past_the_window_is_clipped_to_what_is_visible() {
        let rect = shape_rect(WINDOW, Rect::new(0.0, 0.0, 9000.0, 9000.0)).expect("clamped");
        assert_eq!(rect, Rect::new(100.0, 0.0, 720.0, 560.0));
        let rect = shape_rect(WINDOW, Rect::new(700.0, 550.0, 100.0, 100.0)).expect("clamped");
        assert_eq!(rect, Rect::new(800.0, 550.0, 20.0, 10.0));
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
