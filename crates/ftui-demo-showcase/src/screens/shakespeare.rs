#![forbid(unsafe_code)]

use ftui_core::event::Event;
use ftui_core::geometry::Rect;
use ftui_render::cell::PackedRgba;
use ftui_render::frame::Frame;
use ftui_runtime::Cmd;
use ftui_style::Style;
use ftui_widgets::Widget;
use ftui_widgets::paragraph::Paragraph;

use super::Screen;

pub struct Shakespeare;

impl Shakespeare {
    pub fn new() -> Self {
        Self
    }
}

impl Screen for Shakespeare {
    type Message = Event;

    fn update(&mut self, _event: &Event) -> Cmd<Self::Message> {
        Cmd::None
    }

    fn view(&self, frame: &mut Frame, area: Rect) {
        let placeholder =
            Paragraph::new("Shakespeare Library\n\nThis screen is under construction.")
                .style(Style::new().fg(PackedRgba::rgb(120, 120, 150)));
        placeholder.render(area, frame);
    }

    fn title(&self) -> &'static str {
        "Shakespeare Library"
    }

    fn tab_label(&self) -> &'static str {
        "Shakespeare"
    }
}
