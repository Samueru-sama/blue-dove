//! The renderer a launch asked for.
//!
//! `BD_RENDERER` names it the way GTK's `GSK_RENDERER` names its own: `gl`
//! (the default) draws through glow/OpenGL, `software` through the CPU
//! fallback, and an unknown value is reported and leaves the default.

use std::ffi::OsStr;

/// The renderer a launch asked for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Renderer {
    /// The GPU path through glow/OpenGL.
    #[default]
    Gl,
    /// The CPU path in [`crate::software`].
    Software,
}

impl Renderer {
    /// The name `BD_RENDERER` gives this renderer.
    pub fn name(self) -> &'static str {
        match self {
            Self::Gl => "gl",
            Self::Software => "software",
        }
    }
}

/// The renderer `BD_RENDERER` names.
///
/// Unset means [`Renderer::Gl`]. A value that names nothing is reported on
/// the terminal and also leaves the default, which is what GTK does with an
/// unknown `GSK_RENDERER`.
pub fn renderer() -> Renderer {
    let Some(value) = std::env::var_os("BD_RENDERER") else {
        return Renderer::default();
    };
    parse(value.to_str().unwrap_or_default()).unwrap_or_else(|| {
        report(&value);
        Renderer::default()
    })
}

/// The renderer a name stands for, ignoring case.
fn parse(name: &str) -> Option<Renderer> {
    if name.eq_ignore_ascii_case("gl") {
        Some(Renderer::Gl)
    } else if name.eq_ignore_ascii_case("software") {
        Some(Renderer::Software)
    } else {
        None
    }
}

/// Says that the value names no renderer.
fn report(value: &OsStr) {
    eprintln!(
        "blue-dove: unknown BD_RENDERER {:?}; using {}",
        value,
        Renderer::default().name()
    );
}

#[cfg(test)]
mod tests {
    use super::{Renderer, parse};

    #[test]
    fn the_renderer_names_gl_and_software_without_caring_about_case() {
        assert_eq!(parse("gl"), Some(Renderer::Gl));
        assert_eq!(parse("GL"), Some(Renderer::Gl));
        assert_eq!(parse("software"), Some(Renderer::Software));
        assert_eq!(parse("SoftWare"), Some(Renderer::Software));
        assert_eq!(parse("vulkan"), None);
        assert_eq!(parse(""), None);
    }

    #[test]
    fn gl_is_the_default() {
        assert_eq!(Renderer::default(), Renderer::Gl);
        assert_eq!(Renderer::default().name(), "gl");
    }
}
