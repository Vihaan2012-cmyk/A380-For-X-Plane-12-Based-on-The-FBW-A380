pub mod compute;
pub mod css;
pub mod dom;
pub mod selector;
pub mod shorthand;
pub mod svgattr;
pub mod ua;
pub mod values;

pub use compute::{Cascade, Style, PROPS};
pub use dom::{parse_html, Doc};

impl Cascade {
    pub fn with_defaults(vw: f64, vh: f64) -> Cascade {
        let mut c = Cascade::new(vw, vh);
        c.add_sheet(ua::UA_SHEET, 0);
        c.add_sheet(ua::CUSTOM_ELEMENTS, 0);
        c
    }

    pub fn add_host_sheets(&mut self) {
        self.add_sheet(ua::HOST_SHEET, 1);
    }
}
