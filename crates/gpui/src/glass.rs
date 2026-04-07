use crate::{Hsla, Rgba, Window, WindowAppearance, hsla};

/// A small set of design tokens inspired by `Glass/docs/theme/css/variables.css`.
///
/// These are intentionally low-level (colors + a few opacities) so they can be
/// reused across GPUI demos and apps without imposing a component system.
#[derive(Clone, Copy, Debug)]
pub struct GlassTokens {
    /// Page/background color.
    pub bg: Hsla,
    /// Default foreground/text color.
    pub fg: Hsla,
    /// Higher-contrast foreground (titles / strong text).
    pub fg_strong: Hsla,
    /// Subdued foreground (secondary text).
    pub fg_muted: Hsla,
    /// Accent (links / active).
    pub accent: Hsla,
    /// Panel/surface background.
    pub panel: Hsla,
    /// Border/divider color.
    pub border: Hsla,
    /// A very subtle divider/background separation color.
    pub divider: Hsla,
    /// Overlay scrim color for modals.
    pub scrim: Hsla,
}

impl GlassTokens {
    /// Convert a token color into an RGBA value (convenient for `.bg(rgb(..))`-style APIs).
    #[inline]
    pub fn rgba(&self, c: Hsla) -> Rgba {
        c.into()
    }
}

/// Returns the "Glass" design tokens for the current window appearance.
pub fn glass_tokens(window: &Window) -> GlassTokens {
    // We treat both Dark and VibrantDark as the dark palette.
    let dark = matches!(
        window.appearance(),
        WindowAppearance::Dark | WindowAppearance::VibrantDark
    );

    if dark {
        // From `Glass/docs/theme/css/variables.css` (.dark)
        // --bg: hsl(220, 13%, 7.5%);
        // --fg: hsl(220, 14%, 70%);
        // --title-color / --links: ~hsl(220, 92-93%, 75-80%);
        // --border: hsl(220, 13%, 20%);
        // --sidebar-bg: hsl(220, 13%, 6.5%);
        // --divider: hsl(220, 13%, 12%);
        GlassTokens {
            bg: hsla(220. / 360., 0.13, 0.075, 1.0),
            fg: hsla(220. / 360., 0.14, 0.70, 1.0),
            fg_strong: hsla(220. / 360., 0.13, 0.95, 1.0),
            fg_muted: hsla(220. / 360., 0.14, 0.60, 1.0),
            accent: hsla(220. / 360., 0.93, 0.75, 1.0),
            panel: hsla(220. / 360., 0.13, 0.065, 1.0),
            border: hsla(220. / 360., 0.13, 0.20, 1.0),
            divider: hsla(220. / 360., 0.13, 0.12, 1.0),
            scrim: hsla(0. / 360., 0.0, 0.0, 0.55),
        }
    } else {
        // From `Glass/docs/theme/css/variables.css` (:root)
        // --bg: hsla(50, 25%, 96%);
        // --fg: hsl(220, 13%, 34%);
        // --title-color / --links: hsl(220, 92%, 42%);
        // --border: hsl(220, 13%, 80%);
        // --sidebar-bg: hsla(50, 25%, 94%);
        // --divider: hsl(220, 50%, 45%, 0.1);
        GlassTokens {
            bg: hsla(50. / 360., 0.25, 0.96, 1.0),
            fg: hsla(220. / 360., 0.13, 0.34, 1.0),
            fg_strong: hsla(220. / 360., 0.13, 0.10, 1.0),
            fg_muted: hsla(220. / 360., 0.13, 0.45, 1.0),
            accent: hsla(220. / 360., 0.92, 0.42, 1.0),
            panel: hsla(50. / 360., 0.25, 0.94, 1.0),
            border: hsla(220. / 360., 0.13, 0.80, 1.0),
            divider: hsla(220. / 360., 0.50, 0.45, 0.10),
            scrim: hsla(0. / 360., 0.0, 0.0, 0.35),
        }
    }
}

