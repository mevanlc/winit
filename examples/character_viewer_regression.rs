//! Exercise AppKit text callbacks without opening Character Viewer or changing system input
//! settings. Run on macOS with `cargo run --example character_viewer_regression`.

#[cfg(target_os = "macos")]
mod macos {
    use std::time::{Duration, Instant};

    use objc2::msg_send;
    use objc2_app_kit::{NSEvent, NSEventModifierFlags, NSEventType, NSView};
    use objc2_foundation::{NSNotFound, NSPoint, NSRange, NSString};
    use winit::application::ApplicationHandler;
    use winit::event::{ElementState, Ime, WindowEvent};
    use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use winit::window::{Window, WindowId};

    #[derive(Debug, PartialEq, Eq)]
    enum Observed {
        Ime(Ime),
        Key(String),
    }

    #[derive(Default)]
    struct Probe {
        window: Option<Window>,
        case: usize,
        expected: Vec<Observed>,
        observed: Vec<Observed>,
        deadline: Option<Instant>,
    }

    impl Probe {
        fn view(&self) -> &NSView {
            let handle = self.window.as_ref().unwrap().window_handle().unwrap();
            let RawWindowHandle::AppKit(handle) = handle.as_raw() else { unreachable!() };
            // The window owns its AppKit view for the lifetime of this borrow.
            unsafe { handle.ns_view.cast::<NSView>().as_ref() }
        }

        fn insert(&self, text: &str) {
            let text = NSString::from_str(text);
            unsafe {
                let _: () = msg_send![self.view(), insertText: &*text,
                    replacementRange: NSRange::new(NSNotFound as usize, 0)];
            }
        }

        fn mark(&self) {
            let text = NSString::from_str("仮");
            unsafe {
                let _: () = msg_send![self.view(), setMarkedText: &*text,
                    selectedRange: NSRange::new(1, 0),
                    replacementRange: NSRange::new(NSNotFound as usize, 0)];
            }
        }

        fn press(&self, key_code: u16, text: &str, base: &str, modifiers: NSEventModifierFlags) {
            let text = NSString::from_str(text);
            let base = NSString::from_str(base);
            unsafe {
                let window = self.view().window().unwrap();
                for event_type in [NSEventType::KeyDown, NSEventType::KeyUp] {
                    let event = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
                        event_type, NSPoint::new(0.0, 0.0), modifiers,
                        0.0, window.windowNumber(), None, &text, &base, false, key_code,
                    ).unwrap();
                    if event_type == NSEventType::KeyDown {
                        let _: () = msg_send![self.view(), keyDown: &*event];
                    } else {
                        let _: () = msg_send![self.view(), keyUp: &*event];
                    }
                }
            }
        }

        fn start_case(&mut self, event_loop: &ActiveEventLoop) {
            use Observed::{Ime as I, Key};
            self.observed.clear();
            self.expected = match self.case {
                0 => {
                    self.insert("👩🏽‍💻");
                    vec![I(Ime::Enabled), I(Ime::Commit("👩🏽‍💻".into()))]
                },
                1 | 7 => {
                    self.press(7, "x", "x", NSEventModifierFlags::empty());
                    vec![Key("x".into())]
                },
                2 => {
                    self.insert("<");
                    self.insert("🇺🇸");
                    vec![I(Ime::Commit("<".into())), I(Ime::Commit("🇺🇸".into()))]
                },
                3 => {
                    self.window.as_ref().unwrap().set_ime_allowed(false);
                    self.insert("😀");
                    vec![I(Ime::Disabled)]
                },
                4 => {
                    self.window.as_ref().unwrap().set_ime_allowed(true);
                    self.insert("");
                    self.insert("\n");
                    vec![]
                },
                5 => {
                    self.mark();
                    vec![I(Ime::Enabled), I(Ime::Preedit("仮".into(), Some((3, 3))))]
                },
                6 => {
                    self.insert("確定");
                    let has_marked: bool = unsafe { msg_send![self.view(), hasMarkedText] };
                    assert!(!has_marked, "standalone commit must clear marked text");
                    vec![I(Ime::Preedit(String::new(), None)), I(Ime::Commit("確定".into()))]
                },
                _ => {
                    println!("All 8 Character Viewer callback cases passed");
                    event_loop.exit();
                    return;
                },
            };
            let deadline = Instant::now() + Duration::from_millis(100);
            self.deadline = Some(deadline);
            event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
        }
    }

    impl ApplicationHandler for Probe {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            if self.window.is_some() {
                return;
            }
            self.window = Some(
                event_loop
                    .create_window(
                        Window::default_attributes()
                            .with_title("Character Viewer regression")
                            .with_visible(false),
                    )
                    .unwrap(),
            );
            self.window.as_ref().unwrap().set_ime_allowed(true);
            self.start_case(event_loop);
        }

        fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
            match event {
                WindowEvent::Ime(ime) => self.observed.push(Observed::Ime(ime)),
                WindowEvent::KeyboardInput { event, .. }
                    if event.state == ElementState::Pressed =>
                {
                    self.observed.push(Observed::Key(event.text.unwrap_or_default().to_string()));
                },
                _ => {},
            }
        }

        fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
            if self.deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                assert_eq!(self.observed, self.expected, "callback case {}", self.case);
                println!("Callback case {} passed: {:?}", self.case, self.observed);
                self.case += 1;
                self.deadline = None;
                self.start_case(event_loop);
            }
        }
    }

    pub fn run() {
        EventLoop::new().unwrap().run_app(&mut Probe::default()).unwrap();
    }
}

fn main() {
    #[cfg(target_os = "macos")]
    macos::run();
    #[cfg(not(target_os = "macos"))]
    println!("Character Viewer regression is macOS-only");
}
