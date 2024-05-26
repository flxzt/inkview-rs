use std::fmt::Display;

use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::{Dimensions, Point};
use embedded_graphics::mono_font::{self, MonoTextStyle};
use embedded_graphics::pixelcolor::Gray8;
use embedded_graphics::text::{Alignment, Text};
use embedded_graphics::Drawable;
use inkview::bindings::Inkview;
use inkview_eg::InkviewDisplay;

use crate::daemon::DAEMON_NAME;
use crate::error::DaemonError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Page {
    Greet,
    Status,
}

impl Display for Page {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Page::Greet => write!(f, "Greet"),
            Page::Status => write!(f, "Status"),
        }
    }
}

impl Default for Page {
    fn default() -> Self {
        Self::Greet
    }
}

#[derive(Debug, Default)]
pub(crate) struct DisplayState {
    page: Page,
}

impl DisplayState {
    pub(crate) fn page_prev(&mut self) {
        self.page = match self.page {
            Page::Greet => Page::Greet,
            Page::Status => Page::Greet,
        };
        tracing::info!("Switching to prev page to: {}", self.page);
    }

    pub(crate) fn page_next(&mut self) {
        self.page = match self.page {
            Page::Greet => Page::Status,
            Page::Status => Page::Status,
        };
        tracing::info!("Switching to next page to: {}", self.page);
    }

    pub(crate) fn paint(
        &self,
        _iv: &'static Inkview,
        display: &mut InkviewDisplay,
    ) -> Result<(), DaemonError> {
        let bounding_box = display.bounding_box();

        display.clear(Gray8::new(0xff)).unwrap();

        Text::with_alignment(
            &format!("Page: {}", self.page),
            bounding_box.top_left + Point::new(0, 20),
            MonoTextStyle::new(&mono_font::ascii::FONT_10X20, Gray8::new(0x00)),
            Alignment::Left,
        )
        .draw(display)
        .unwrap();

        match self.page {
            Page::Greet => {
                Text::with_alignment(
                    &format!("{DAEMON_NAME} running.."),
                    bounding_box.center(),
                    MonoTextStyle::new(&mono_font::ascii::FONT_10X20, Gray8::new(0x00)),
                    Alignment::Center,
                )
                .draw(display)
                .unwrap();
            }
            Page::Status => {
                Text::with_alignment(
                    &format!(
                        "display (width, height): ({},{})",
                        bounding_box.size.width, bounding_box.size.height
                    ),
                    bounding_box.center(),
                    MonoTextStyle::new(&mono_font::ascii::FONT_10X20, Gray8::new(0x00)),
                    Alignment::Center,
                )
                .draw(display)
                .unwrap();
            }
        }

        display.flush();
        Ok(())
    }
}
