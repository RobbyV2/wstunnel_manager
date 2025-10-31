use iced::Color;

pub struct WstunnelTheme {
    #[allow(dead_code)]
    pub colors: ThemeColors,
}

impl WstunnelTheme {
    pub fn new() -> Self {
        Self {
            colors: ThemeColors::new(),
        }
    }

    pub fn to_iced_theme(&self) -> iced::Theme {
        iced::Theme::CatppuccinLatte
    }
}

impl Default for WstunnelTheme {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
pub struct ThemeColors {
    pub success: Color,
    pub error: Color,
    pub warning: Color,
    pub info: Color,
    pub primary: Color,
    pub background: Color,
    pub text: Color,
    pub border: Color,
}

impl ThemeColors {
    pub fn new() -> Self {
        Self {
            success: Color::from_rgb(0.25, 0.7, 0.25),
            error: Color::from_rgb(0.85, 0.2, 0.2),
            warning: Color::from_rgb(0.9, 0.7, 0.1),
            info: Color::from_rgb(0.3, 0.6, 0.85),
            primary: Color::from_rgb(0.35, 0.55, 0.75),
            background: Color::from_rgb(0.96, 0.96, 0.96),
            text: Color::from_rgb(0.15, 0.15, 0.15),
            border: Color::from_rgb(0.65, 0.65, 0.65),
        }
    }
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
pub const SPACING_SMALL: u16 = 5;
#[allow(dead_code)]
pub const SPACING_MEDIUM: u16 = 10;
#[allow(dead_code)]
pub const SPACING_LARGE: u16 = 20;
#[allow(dead_code)]
pub const SPACING_XLARGE: u16 = 30;

#[allow(dead_code)]
pub const FONT_SIZE_SMALL: f32 = 12.0;
#[allow(dead_code)]
pub const FONT_SIZE_NORMAL: f32 = 14.0;
#[allow(dead_code)]
pub const FONT_SIZE_LARGE: f32 = 16.0;
#[allow(dead_code)]
pub const FONT_SIZE_XLARGE: f32 = 20.0;

#[allow(dead_code)]
pub const BORDER_RADIUS_SMALL: f32 = 2.0;
#[allow(dead_code)]
pub const BORDER_RADIUS_MEDIUM: f32 = 4.0;
#[allow(dead_code)]
pub const BORDER_RADIUS_LARGE: f32 = 8.0;

#[allow(dead_code)]
pub const BUTTON_PADDING: u16 = 10;
#[allow(dead_code)]
pub const CONTAINER_PADDING: u16 = 15;
