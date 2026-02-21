use serde::{Deserialize, Serialize};

const GITHUB_LIGHT_BACKGROUND: &str = "#ffffff";
const GITHUB_LIGHT_TEXT: &str = "#24292f";
const GITHUB_LIGHT_HEADING: &str = "#1b1f23";
const GITHUB_LIGHT_LINK: &str = "#0969da";
const GITHUB_LIGHT_CODE_BG: &str = "#f6f8fa";
const GITHUB_LIGHT_CODE_TEXT: &str = "#24292f";
const GITHUB_LIGHT_QUOTE_BORDER: &str = "#d0d7de";
const GITHUB_LIGHT_QUOTE_TEXT: &str = "#57606a";

const BUILTIN_THEMES: &[(&str, &str)] = &[
    ("catppuccin_latte", include_str!("../themes/catppuccin_latte.toml")),
    ("catppuccin_mocha", include_str!("../themes/catppuccin_mocha.toml")),
    ("dracula", include_str!("../themes/dracula.toml")),
    ("github_dark", include_str!("../themes/github_dark.toml")),
    ("github_light", include_str!("../themes/github_light.toml")),
    ("gruvbox_dark", include_str!("../themes/gruvbox_dark.toml")),
    ("gruvbox_light", include_str!("../themes/gruvbox_light.toml")),
    ("monokai_pro", include_str!("../themes/monokai_pro.toml")),
    ("nord", include_str!("../themes/nord.toml")),
    ("solarized_dark", include_str!("../themes/solarized_dark.toml")),
    ("solarized_light", include_str!("../themes/solarized_light.toml")),
    ("tokyo_night", include_str!("../themes/tokyo_night.toml")),
];

const FONT_SIZE_BASE: f32 = 16.0;
const FONT_SIZE_CODE: f32 = 13.0;
const LINE_HEIGHT: f32 = 1.6;
const MARGIN: f32 = 16.0;
const PADDING: f32 = 32.0;
const CODE_PADDING_X: f32 = 12.0;
const CODE_PADDING_Y: f32 = 8.0;
const CODE_RADIUS: f32 = 4.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    #[serde(default = "default_background")]
    pub background_color: String,
    #[serde(default = "default_text")]
    pub text_color: String,
    #[serde(default = "default_heading")]
    pub heading_color: String,
    #[serde(default = "default_link")]
    pub link_color: String,
    #[serde(default = "default_code_bg")]
    pub code_bg_color: String,
    #[serde(default = "default_code_text")]
    pub code_text_color: String,
    #[serde(default = "default_quote_border")]
    pub quote_border_color: String,
    #[serde(default = "default_quote_text")]
    pub quote_text_color: String,

    #[serde(default = "default_font_size_base")]
    pub font_size_base: f32,
    #[serde(default = "default_font_size_code")]
    pub font_size_code: f32,
    #[serde(default = "default_line_height")]
    pub line_height: f32,

    #[serde(default = "default_margin")]
    pub margin_top: f32,
    #[serde(default = "default_margin")]
    pub margin_bottom: f32,
    #[serde(default = "default_padding")]
    pub padding_x: f32,
    #[serde(default = "default_padding")]
    pub padding_y: f32,

    #[serde(default = "default_code_padding_x")]
    pub code_padding_x: f32,
    #[serde(default = "default_code_padding_y")]
    pub code_padding_y: f32,
    #[serde(default = "default_code_radius")]
    pub code_radius: f32,
}

fn default_background() -> String {
    GITHUB_LIGHT_BACKGROUND.to_string()
}
fn default_text() -> String {
    GITHUB_LIGHT_TEXT.to_string()
}
fn default_heading() -> String {
    GITHUB_LIGHT_HEADING.to_string()
}
fn default_link() -> String {
    GITHUB_LIGHT_LINK.to_string()
}
fn default_code_bg() -> String {
    GITHUB_LIGHT_CODE_BG.to_string()
}
fn default_code_text() -> String {
    GITHUB_LIGHT_CODE_TEXT.to_string()
}
fn default_quote_border() -> String {
    GITHUB_LIGHT_QUOTE_BORDER.to_string()
}
fn default_quote_text() -> String {
    GITHUB_LIGHT_QUOTE_TEXT.to_string()
}
fn default_font_size_base() -> f32 {
    FONT_SIZE_BASE
}
fn default_font_size_code() -> f32 {
    FONT_SIZE_CODE
}
fn default_line_height() -> f32 {
    LINE_HEIGHT
}
fn default_margin() -> f32 {
    MARGIN
}
fn default_padding() -> f32 {
    PADDING
}
fn default_code_padding_x() -> f32 {
    CODE_PADDING_X
}
fn default_code_padding_y() -> f32 {
    CODE_PADDING_Y
}
fn default_code_radius() -> f32 {
    CODE_RADIUS
}

impl Default for Theme {
    fn default() -> Self {
        Self::from_builtin("solarized_light").expect("built-in solarized_light theme must parse")
    }
}

#[derive(Debug, Deserialize)]
struct AlacrittyColors {
    primary: AlacrittyPrimary,
    normal: AlacrittyNormal,
}

#[derive(Debug, Deserialize)]
struct AlacrittyPrimary {
    background: String,
    foreground: String,
}

#[derive(Debug, Deserialize)]
struct AlacrittyNormal {
    black: String,
    blue: String,
    cyan: String,
    white: String,
}

#[derive(Debug, Deserialize)]
struct AlacrittyTheme {
    colors: AlacrittyColors,
}

impl Theme {
    pub fn github_light() -> Self {
        Theme {
            background_color: GITHUB_LIGHT_BACKGROUND.to_string(),
            text_color: GITHUB_LIGHT_TEXT.to_string(),
            heading_color: GITHUB_LIGHT_HEADING.to_string(),
            link_color: GITHUB_LIGHT_LINK.to_string(),
            code_bg_color: GITHUB_LIGHT_CODE_BG.to_string(),
            code_text_color: GITHUB_LIGHT_CODE_TEXT.to_string(),
            quote_border_color: GITHUB_LIGHT_QUOTE_BORDER.to_string(),
            quote_text_color: GITHUB_LIGHT_QUOTE_TEXT.to_string(),

            font_size_base: FONT_SIZE_BASE,
            font_size_code: FONT_SIZE_CODE,
            line_height: LINE_HEIGHT,

            margin_top: MARGIN,
            margin_bottom: MARGIN,
            padding_x: PADDING,
            padding_y: PADDING,

            code_padding_x: CODE_PADDING_X,
            code_padding_y: CODE_PADDING_Y,
            code_radius: CODE_RADIUS,
        }
    }

    pub fn from_builtin(name: &str) -> Result<Self, String> {
        let normalized = name.trim().to_ascii_lowercase().replace('-', "_");
        let content = BUILTIN_THEMES
            .iter()
            .find(|(n, _)| *n == normalized)
            .map(|(_, c)| *c)
            .ok_or_else(|| {
                format!(
                    "Unknown built-in theme '{}'. Available: {}",
                    name,
                    Self::list_builtins().join(", ")
                )
            })?;
        Self::from_alacritty_toml(content)
    }

    pub fn list_builtins() -> Vec<&'static str> {
        BUILTIN_THEMES.iter().map(|(n, _)| *n).collect()
    }

    pub fn from_alacritty_yaml(content: &str) -> Result<Self, String> {
        let alacritty: AlacrittyTheme = serde_yaml::from_str(content)
            .map_err(|e| format!("Failed to parse Alacritty YAML: {}", e))?;

        Self::from_alacritty_theme(alacritty)
    }

    pub fn from_alacritty_toml(content: &str) -> Result<Self, String> {
        let alacritty: AlacrittyTheme = toml::from_str(content)
            .map_err(|e| format!("Failed to parse Alacritty TOML: {}", e))?;

        Self::from_alacritty_theme(alacritty)
    }

    fn from_alacritty_theme(alacritty: AlacrittyTheme) -> Result<Self, String> {
        let colors = alacritty.colors;

        Ok(Theme {
            background_color: colors.primary.background,
            text_color: colors.primary.foreground.clone(),
            heading_color: colors.normal.blue.clone(),
            link_color: colors.normal.cyan,
            code_bg_color: colors.normal.black,
            code_text_color: colors.primary.foreground.clone(),
            quote_border_color: colors.normal.white,
            quote_text_color: colors.primary.foreground,

            font_size_base: FONT_SIZE_BASE,
            font_size_code: FONT_SIZE_CODE,
            line_height: LINE_HEIGHT,
            margin_top: MARGIN,
            margin_bottom: MARGIN,
            padding_x: PADDING,
            padding_y: PADDING,
            code_padding_x: CODE_PADDING_X,
            code_padding_y: CODE_PADDING_Y,
            code_radius: CODE_RADIUS,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Theme;

    #[test]
    fn from_builtin_accepts_hyphenated_and_case_insensitive_names() {
        let underscore = Theme::from_builtin("solarized_light").expect("underscore variant");
        let hyphen = Theme::from_builtin("Solarized-Light").expect("hyphen variant");

        assert_eq!(underscore.background_color, hyphen.background_color);
        assert_eq!(underscore.text_color, hyphen.text_color);
    }
}
