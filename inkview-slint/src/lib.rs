use inkview::screen::PixelFormat;
use inkview::{event::Key, screen::Screen, Event};
use slint::platform::{
    software_renderer::{self as renderer, PhysicalRegion},
    WindowEvent,
};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::mpsc::Receiver,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
struct Pixel<P>(P);

impl<P: PixelFormat + Copy> TargetPixel for Pixel<P> {
    fn blend(&mut self, color: renderer::PremultipliedRgbaColor) {
        let other = P::from_rgb24(color.red, color.green, color.blue);
        let by = u8::MAX - color.alpha;
        *self = Pixel(self.0.mix(other, by));
    }

    fn from_rgb(red: u8, green: u8, blue: u8) -> Self {
        Self(P::from_rgb24(red, green, blue))
    }
}

pub struct Backend<P: 'static> {
    screen: RefCell<Screen<'static, P>>,
    evts: Receiver<Event>,
    width: usize,
    height: usize,
    window: RefCell<Option<Rc<renderer::MinimalSoftwareWindow>>>,
    buffer: RefCell<Vec<Pixel<P>>>,
}

impl Backend {
    pub fn new(screen: Screen<'static>, evts: Receiver<Event>) -> Self {
        let width = screen.width();
        let height = screen.height();

        let buffer = vec![Default::default(); width * height];

        Self {
            screen: screen.into(),
            evts,
            width,
            height,
            window: Default::default(),
            buffer: buffer.into(),
        }
    }
}

fn rect_from_phys(r: PhysicalRegion) -> euclid::Rect<i32, euclid::UnknownUnit> {
    euclid::Rect::new(
        euclid::Point2D::new(r.bounding_box_origin().x, r.bounding_box_origin().y),
        euclid::Size2D::new(
            r.bounding_box_size().width as i32,
            r.bounding_box_size().height as i32,
        ),
    )
}

fn scale_from_screen<P: 'static>(screen: &Screen<P>) -> f32 {
    let dpi = screen.dpi() as f32 / 100.0;

    return dpi * screen.scale();
}

impl<P: PixelFormat + Copy> slint::platform::Platform for Backend<P> {
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
        let window =
            renderer::MinimalSoftwareWindow::new(renderer::RepaintBufferType::ReusedBuffer);
        self.window.replace(Some(window.clone()));
        Ok(window)
    }

    fn run_event_loop(&self) -> Result<(), slint::PlatformError> {
        let scale_factor = scale_from_screen(&*self.screen.borrow());

        let convert_evt = |evt| ink_evt_to_slint(scale_factor, evt);

        slint::Window::set_size(
            self.window.borrow().as_ref().unwrap().as_ref(),
            slint::PhysicalSize::new(self.width as u32, self.height as u32)
                .to_logical(scale_factor),
        );

        self.window
            .borrow()
            .as_ref()
            .unwrap()
            .dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor });

        let mut perform_full_redraw_over_dynamic_regions_after: Option<Instant> = None;
        let mut regions_updated_dynamically_in_need_of_redraw: Option<
            euclid::Rect<i32, euclid::UnknownUnit>,
        > = None;

        loop {
            slint::platform::update_timers_and_animations();

            if let Some(window) = self.window.borrow().clone() {
                let delay = if window.has_active_animations() {
                    None
                } else {
                    match (
                        slint::platform::duration_until_next_timer_update(),
                        perform_full_redraw_over_dynamic_regions_after.map(|i| i.elapsed()),
                    ) {
                        (Some(a), Some(b)) => Some(a.min(b)),
                        (a, b) => Some(a.or(b).unwrap_or(Duration::from_millis(1000))),
                    }
                };

                let evt = if let Some(delay) = delay {
                    self.evts.recv_timeout(delay).ok().and_then(convert_evt)
                } else if window.has_active_animations() {
                    self.evts.try_recv().ok().and_then(convert_evt)
                } else {
                    self.evts.recv().ok().and_then(convert_evt)
                };

                if let (Some(redraw_region), Some(perform_full_redraw_after)) = (
                    regions_updated_dynamically_in_need_of_redraw,
                    perform_full_redraw_over_dynamic_regions_after,
                ) {
                    if perform_full_redraw_after < Instant::now() {
                        perform_full_redraw_over_dynamic_regions_after = None;
                        regions_updated_dynamically_in_need_of_redraw = None;

                        let mut screen = self.screen.borrow_mut();
                        screen.partial_update(
                            redraw_region.origin.x,
                            redraw_region.origin.y,
                            redraw_region.width() as u32,
                            redraw_region.height() as u32,
                        );
                    }
                }

                if let Some(evt) = evt {
                    window.dispatch_event(evt);
                }

                while let Some(evt) = self.evts.try_recv().ok().and_then(convert_evt) {
                    window.dispatch_event(evt);
                }

                window.draw_if_needed(|renderer| {
                    let mut buffer = self.buffer.borrow_mut();
                    let damage = renderer.render(buffer.as_mut_slice(), self.width);
                    let mut screen = self.screen.borrow_mut();

                    for dy in 0..damage.bounding_box_size().height {
                        for dx in 0..damage.bounding_box_size().width {
                            let x = damage.bounding_box_origin().x + dx as i32;
                            let y = damage.bounding_box_origin().y + dy as i32;
                            let idx = y as usize * self.width + x as usize;
                            let c = buffer[idx];
                            screen.draw(x as usize, y as usize, c.0);
                        }
                    }

                    // println!("Drawing to: {:?}", damage);

                    if screen.is_updating() {
                        println!(
                            "  Slint partial update full redraw, now={:?}",
                            Instant::now()
                        );
                        screen.partial_update(
                            damage.bounding_box_origin().x,
                            damage.bounding_box_origin().y,
                            damage.bounding_box_size().width,
                            damage.bounding_box_size().height,
                        );

                        if let Some(r) = regions_updated_dynamically_in_need_of_redraw.as_mut() {
                            *r = r.union(&rect_from_phys(damage));
                        } else {
                            regions_updated_dynamically_in_need_of_redraw =
                                Some(rect_from_phys(damage));
                        }

                        if perform_full_redraw_over_dynamic_regions_after.is_none() {
                            perform_full_redraw_over_dynamic_regions_after =
                                Some(Instant::now() + Duration::from_millis(200));
                        }
                    } else {
                        screen.partial_update(
                            damage.bounding_box_origin().x,
                            damage.bounding_box_origin().y,
                            damage.bounding_box_size().width,
                            damage.bounding_box_size().height,
                        );
                    }
                });
            }
        }
    }
}

fn ink_key_to_slint(key: Key) -> Option<slint::platform::Key> {
    match key {
        Key::Up => Some(slint::platform::Key::UpArrow),
        Key::Down => Some(slint::platform::Key::DownArrow),
        Key::Left => Some(slint::platform::Key::LeftArrow),
        Key::Prev => Some(slint::platform::Key::LeftArrow),
        Key::Prev2 => Some(slint::platform::Key::LeftArrow),
        Key::Right => Some(slint::platform::Key::RightArrow),
        Key::Next => Some(slint::platform::Key::RightArrow),
        Key::Next2 => Some(slint::platform::Key::RightArrow),
        Key::Ok => Some(slint::platform::Key::Return),
        Key::Back => Some(slint::platform::Key::Backspace),
        Key::Menu => Some(slint::platform::Key::Menu),
        Key::Home => Some(slint::platform::Key::Home),
        Key::Plus => Some(slint::platform::Key::PageUp),
        Key::Minus => Some(slint::platform::Key::PageDown),
        _ => None,
    }
}

fn ink_evt_to_slint(scale_factor: f32, evt: Event) -> Option<WindowEvent> {
    println!("evt: {:?}", evt);
    let evt = match evt {
        Event::PointerDown { x, y } => WindowEvent::PointerPressed {
            position: slint::PhysicalPosition { x, y }.to_logical(scale_factor),
            button: slint::platform::PointerEventButton::Left,
        },
        Event::PointerMove { x, y } => WindowEvent::PointerMoved {
            position: slint::PhysicalPosition { x, y }.to_logical(scale_factor),
        },
        Event::PointerUp { x, y } => WindowEvent::PointerReleased {
            position: slint::PhysicalPosition { x, y }.to_logical(scale_factor),
            button: slint::platform::PointerEventButton::Left,
        },
        Event::Foreground { .. } => WindowEvent::WindowActiveChanged(true),
        Event::Background { .. } => WindowEvent::WindowActiveChanged(false),
        Event::KeyDown { key } => {
            if let Some(slint_key) = ink_key_to_slint(key) {
                WindowEvent::KeyPressed {
                    text: slint_key.into(),
                }
            } else {
                return None;
            }
        }
        Event::KeyRepeat { key } => {
            if let Some(slint_key) = ink_key_to_slint(key) {
                WindowEvent::KeyPressRepeated {
                    text: slint_key.into(),
                }
            } else {
                return None;
            }
        }
        Event::KeyUp { key } => {
            if let Some(slint_key) = ink_key_to_slint(key) {
                WindowEvent::KeyReleased {
                    text: slint_key.into(),
                }
            } else {
                return None;
            }
        }
        _ => return None,
    };

    Some(evt)
}
