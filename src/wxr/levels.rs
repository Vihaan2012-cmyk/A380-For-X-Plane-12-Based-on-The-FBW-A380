//! The four levels a real Airbus weather radar paints: green, amber and red
//! for rising return strength, magenta for the most hazardous cells.
//! `physics.rs` decides which level a sampled cell gets.

/// One radar return level, most severe last.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Level {
    #[default]
    None,
    Green,
    Amber,
    Red,
    Magenta,
}

impl Level {
    /// Straight RGBA (not premultiplied), X-Plane's `nd.js` amber
    /// (`#ffff00`, `VerticalDisplay.tsx:673`) and the industry-standard WXR
    /// palette otherwise (pure green/red/magenta returns).
    pub fn rgba(self) -> [u8; 4] {
        match self {
            Level::None => [0, 0, 0, 0],
            Level::Green => [0, 255, 0, 255],
            Level::Amber => [255, 255, 0, 255],
            Level::Red => [255, 0, 0, 255],
            Level::Magenta => [255, 0, 255, 255],
        }
    }

    pub fn is_none(self) -> bool {
        self == Level::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_order_by_severity() {
        assert!(Level::None < Level::Green);
        assert!(Level::Green < Level::Amber);
        assert!(Level::Amber < Level::Red);
        assert!(Level::Red < Level::Magenta);
    }

    #[test]
    fn none_is_transparent() {
        assert_eq!(Level::None.rgba()[3], 0);
        for l in [Level::Green, Level::Amber, Level::Red, Level::Magenta] {
            assert_eq!(l.rgba()[3], 255);
        }
    }
}
