//! Whether the window is really on screen.
//!
//! winit follows visibility on X11 from `VisibilityNotify` events alone, and
//! a window manager that unmaps a window sends none: a scratchpad, another
//! workspace and a minimized window all go off screen with the cached state
//! still saying visible. eframe then lays out and paints every frame the app
//! asks for, and the GPU draws pictures nobody sees.
//!
//! The window manager owns `_NET_WM_STATE`, and winit reads
//! `_NET_WM_STATE_HIDDEN` from it as the window's minimized state. Adding that
//! atom while the window is unmapped is what makes the frame loop idle: eframe
//! keeps running [`crate::app::App::run_logic`] and stops drawing, which is
//! all a player in the background needs.

#[cfg(target_os = "linux")]
use std::time::Duration;

/// How long a map-state answer is trusted while the window is on screen.
///
/// A hidden window is asked every pass: its passes are few, and the first
/// event that maps it has to be noticed at once even when nothing is playing.
#[cfg(target_os = "linux")]
const POLL: Duration = Duration::from_millis(200);

/// Watches whether the app's window is on screen.
///
/// Nothing on a host that reports visibility itself, which is every platform
/// but X11: [`Screen::new`] answers `None` there and the frame loop is left
/// alone.
pub struct Screen {
    /// The X11 side, present only on Linux.
    #[cfg(target_os = "linux")]
    x11: x11::Server,
}

impl Screen {
    /// Starts watching `window`, or answers `None` when this host reports
    /// visibility on its own.
    pub fn new(window: &winit::window::Window) -> Option<Self> {
        #[cfg(target_os = "linux")]
        let screen = x11::Server::new(window).map(|x11| Self { x11 });
        #[cfg(not(target_os = "linux"))]
        let screen = {
            let _ = window;
            None
        };
        screen
    }

    /// Whether the window is off screen, keeping its EWMH state in step.
    ///
    /// On X11 the map state is the authority, and the hidden atom is added or
    /// removed to match it.
    pub fn hidden(&mut self) -> bool {
        #[cfg(target_os = "linux")]
        let hidden = self.x11.hidden();
        #[cfg(not(target_os = "linux"))]
        let hidden = false;
        hidden
    }
}

/// The X11 side of whether the window is on screen.
#[cfg(target_os = "linux")]
mod x11 {
    use std::time::Instant;

    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::ConnectionExt as _;
    use x11rb::protocol::xproto::{AtomEnum, MapState, PropMode, Window};
    use x11rb::rust_connection::RustConnection;
    use x11rb::wrapper::ConnectionExt as _;

    use super::POLL;

    /// The X11 window whose map state and EWMH state are watched.
    pub(super) struct Server {
        /// The connection this window is asked over.
        connection: RustConnection,
        /// The window winit opened.
        window: Window,
        /// `_NET_WM_STATE`, which carries the window manager's state list.
        state: u32,
        /// `_NET_WM_STATE_HIDDEN`, the atom winit reads as minimized.
        hidden: u32,
        /// Whether the hidden atom was added to the state list.
        marked: bool,
        /// When the map state was last asked for.
        asked: Option<Instant>,
    }

    impl Server {
        /// Connects to the display `window` is on, or answers `None` when the
        /// window is not an X11 one.
        pub(super) fn new(window: &winit::window::Window) -> Option<Self> {
            let handle = window.window_handle().ok()?.as_raw();
            let RawWindowHandle::Xlib(handle) = handle else {
                return None;
            };
            let (connection, _screen) = x11rb::connect(None).ok()?;
            let state = connection
                .intern_atom(false, b"_NET_WM_STATE")
                .ok()?
                .reply()
                .ok()?
                .atom;
            let hidden = connection
                .intern_atom(false, b"_NET_WM_STATE_HIDDEN")
                .ok()?
                .reply()
                .ok()?
                .atom;
            Some(Self {
                connection,
                window: handle.window as Window,
                state,
                hidden,
                marked: false,
                asked: None,
            })
        }

        /// Whether the window is off screen, asking the server as often as
        /// [`POLL`] allows while it is on screen.
        pub(super) fn hidden(&mut self) -> bool {
            let now = Instant::now();
            if !self.marked
                && self
                    .asked
                    .is_some_and(|asked| now.duration_since(asked) < POLL)
            {
                return false;
            }
            self.asked = Some(now);
            let off_screen = self
                .connection
                .get_window_attributes(self.window)
                .ok()
                .and_then(|cookie| cookie.reply().ok())
                .is_some_and(|reply| reply.map_state != MapState::VIEWABLE);
            if !off_screen {
                if self.marked {
                    self.mark(false);
                }
                return false;
            }
            self.mark(true);
            true
        }

        /// Adds or removes the hidden atom, leaving every other state alone.
        fn mark(&mut self, hidden: bool) {
            let current: Vec<u32> = self
                .connection
                .get_property(false, self.window, self.state, AtomEnum::ATOM, 0, 64)
                .ok()
                .and_then(|cookie| cookie.reply().ok())
                .and_then(|reply| reply.value32().map(|atoms| atoms.collect()))
                .unwrap_or_default();
            let Some(next) = with_state(&current, self.hidden, hidden) else {
                self.marked = hidden;
                return;
            };
            let _ = self.connection.change_property32(
                PropMode::REPLACE,
                self.window,
                self.state,
                AtomEnum::ATOM,
                &next,
            );
            let _ = self.connection.flush();
            self.marked = hidden;
        }
    }

    /// The state list with `atom` added or removed, or `None` when it already
    /// says so.
    fn with_state(atoms: &[u32], atom: u32, present: bool) -> Option<Vec<u32>> {
        if atoms.contains(&atom) == present {
            return None;
        }
        let mut next = atoms.to_vec();
        if present {
            next.push(atom);
        } else {
            next.retain(|other| *other != atom);
        }
        Some(next)
    }

    #[cfg(test)]
    mod tests {
        use super::with_state;

        #[test]
        fn the_hidden_atom_joins_the_state_list_without_replacing_it() {
            let focused = 1;
            let hidden = 2;
            assert_eq!(
                with_state(&[focused], hidden, true),
                Some(vec![focused, hidden])
            );
            assert_eq!(
                with_state(&[focused, hidden], hidden, true),
                None,
                "a state list that already says hidden is left alone"
            );
            assert_eq!(
                with_state(&[focused, hidden], hidden, false),
                Some(vec![focused])
            );
            assert_eq!(
                with_state(&[focused], hidden, false),
                None,
                "a state list without the atom is left alone"
            );
        }
    }
}
