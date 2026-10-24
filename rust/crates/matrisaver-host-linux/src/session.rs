//! Which display session, if any, this process can draw into.
//!
//! The milestone asks for *explicit* fallback behaviour on unsupported
//! compositor paths. Explicit means: say which session was detected, say
//! which rung of the fallback ladder was used, and when there is no
//! session at all, say so with an actionable message and a distinct exit
//! code instead of letting winit fail with a bare OS error. A support
//! report should be able to quote one line.

/// Detected session kind, in the order the host prefers them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Session {
    Wayland,
    X11,
    /// No display server reachable — a TTY, an SSH session, a container,
    /// or a CI runner. Playback is impossible; the headless benchmark
    /// path still works.
    Headless,
}

impl Session {
    pub fn key(self) -> &'static str {
        match self {
            Self::Wayland => "wayland",
            Self::X11 => "x11",
            Self::Headless => "headless",
        }
    }

    pub fn can_present(self) -> bool {
        !matches!(self, Self::Headless)
    }
}

/// Detect from the process environment.
pub fn detect() -> Session {
    detect_with(|key| std::env::var(key).ok())
}

/// Detection over an arbitrary environment lookup, so the precedence
/// rules are testable without mutating the real environment (which races
/// across parallel test threads).
///
/// Wayland leads X11 deliberately: under XWayland both `WAYLAND_DISPLAY`
/// and `DISPLAY` are set, and the native Wayland path avoids a round
/// trip through the X server. `MATRISAVER_FORCE_SESSION` overrides
/// everything, which is how the X11 path gets exercised on a Wayland
/// desktop (and vice versa) without logging out.
pub fn detect_with<F>(lookup: F) -> Session
where
    F: Fn(&str) -> Option<String>,
{
    let non_empty = |key: &str| lookup(key).filter(|value| !value.trim().is_empty());

    if let Some(forced) = non_empty("MATRISAVER_FORCE_SESSION") {
        match forced.trim().to_ascii_lowercase().as_str() {
            "wayland" => return Session::Wayland,
            "x11" => return Session::X11,
            "headless" | "none" => return Session::Headless,
            _ => {}
        }
    }

    if non_empty("WAYLAND_DISPLAY").is_some() {
        return Session::Wayland;
    }
    if non_empty("DISPLAY").is_some() {
        return Session::X11;
    }
    // XDG_SESSION_TYPE alone is not enough to draw into — a session can
    // advertise "wayland" while the socket env var is absent, e.g. a
    // systemd user unit started before the compositor exported it — but
    // it is worth honouring when it names something concrete, because
    // winit can still find the socket via XDG_RUNTIME_DIR.
    if let Some(kind) = non_empty("XDG_SESSION_TYPE") {
        match kind.trim().to_ascii_lowercase().as_str() {
            "wayland" => return Session::Wayland,
            "x11" => return Session::X11,
            _ => {}
        }
    }
    Session::Headless
}

/// The message shown when playback was asked for and there is no
/// session. Spelled out because this is the single most likely support
/// question for a `.deb` install driven from a systemd unit.
pub const NO_SESSION_HELP: &str = "\
No display session detected: WAYLAND_DISPLAY and DISPLAY are both unset.

MatriSaver needs a compositor to present into. Common causes:

  * Running from a bare TTY or an SSH session with no X/Wayland forwarding.
  * A systemd *system* unit. Idle integration must be a user unit, so it
    inherits the graphical session environment:
        systemctl --user start matrisaver-idle.service
  * A user unit that started before the compositor published its
    environment. Add to the compositor autostart:
        systemctl --user import-environment WAYLAND_DISPLAY DISPLAY XDG_RUNTIME_DIR

To benchmark the renderer with no session at all, use the headless path:

    matrisaver --benchmark-frames 600

To force a specific backend once a session exists:

    MATRISAVER_FORCE_SESSION=x11 matrisaver --run";

#[cfg(test)]
mod tests {
    use super::{detect_with, Session};

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let owned: Vec<(String, String)> = pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect();
        move |key: &str| {
            owned
                .iter()
                .find(|(candidate, _)| candidate == key)
                .map(|(_, value)| value.clone())
        }
    }

    #[test]
    fn wayland_wins_over_x11_under_xwayland() {
        let session = detect_with(env(&[("WAYLAND_DISPLAY", "wayland-0"), ("DISPLAY", ":0")]));
        assert_eq!(session, Session::Wayland);
    }

    #[test]
    fn x11_detected_when_only_display_is_set() {
        assert_eq!(detect_with(env(&[("DISPLAY", ":0")])), Session::X11);
    }

    #[test]
    fn empty_values_do_not_count_as_a_session() {
        let session = detect_with(env(&[("WAYLAND_DISPLAY", "  "), ("DISPLAY", "")]));
        assert_eq!(session, Session::Headless);
    }

    #[test]
    fn nothing_set_is_headless() {
        assert_eq!(detect_with(env(&[])), Session::Headless);
        assert!(!Session::Headless.can_present());
    }

    #[test]
    fn session_type_is_a_last_resort() {
        assert_eq!(
            detect_with(env(&[("XDG_SESSION_TYPE", "wayland")])),
            Session::Wayland
        );
        assert_eq!(
            detect_with(env(&[("XDG_SESSION_TYPE", "tty")])),
            Session::Headless
        );
    }

    #[test]
    fn force_override_beats_real_environment() {
        let session = detect_with(env(&[
            ("MATRISAVER_FORCE_SESSION", "x11"),
            ("WAYLAND_DISPLAY", "wayland-0"),
        ]));
        assert_eq!(session, Session::X11);

        let session = detect_with(env(&[
            ("MATRISAVER_FORCE_SESSION", "HEADLESS"),
            ("WAYLAND_DISPLAY", "wayland-0"),
        ]));
        assert_eq!(session, Session::Headless);
    }

    #[test]
    fn unrecognized_force_value_falls_through_to_detection() {
        let session = detect_with(env(&[
            ("MATRISAVER_FORCE_SESSION", "mir"),
            ("DISPLAY", ":0"),
        ]));
        assert_eq!(session, Session::X11);
    }

    #[test]
    fn session_keys_are_stable() {
        assert_eq!(Session::Wayland.key(), "wayland");
        assert_eq!(Session::X11.key(), "x11");
        assert_eq!(Session::Headless.key(), "headless");
        assert!(Session::Wayland.can_present());
    }
}
