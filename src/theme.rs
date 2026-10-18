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
    (
        "catppuccin_latte",
        include_str!("../themes/catppuccin_latte.toml"),
    ),
    (
        "catppuccin_mocha",
        include_str!("../themes/catppuccin_mocha.toml"),
    ),
    ("dracula", include_str!("../themes/dracula.toml")),
    ("everforest", include_str!("../themes/everforest.toml")),
    ("github_dark", include_str!("../themes/github_dark.toml")),
    ("github_light", include_str!("../themes/github_light.toml")),
    ("gruvbox_dark", include_str!("../themes/gruvbox_dark.toml")),
    (
        "gruvbox_light",
        include_str!("../themes/gruvbox_light.toml"),
    ),
    ("monokai_pro", include_str!("../themes/monokai_pro.toml")),
    ("nord", include_str!("../themes/nord.toml")),
    (
        "solarized_dark",
        include_str!("../themes/solarized_dark.toml"),
    ),
    (
        "solarized_light",
        include_str!("../themes/solarized_light.toml"),
    ),
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

    /// Background fill opacity for table header rows (0.0–1.0).
    #[serde(default = "default_table_header_opacity")]
    pub table_header_opacity: f32,
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
fn default_table_header_opacity() -> f32 {
    0.08
}

impl Default for Theme {
    fn default() -> Self {
        Self::from_builtin_or_fallback("solarized_light")
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
    fn from_builtin_or_fallback(name: &str) -> Self {
        Self::from_builtin(name).unwrap_or_else(|_| Self::github_light())
    }

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
            table_header_opacity: 0.08,
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

    /// Detect Alacritty theme syntax by content, retaining diagnostics from both parsers.
    pub fn from_alacritty(content: &str) -> Result<Self, String> {
        Self::from_alacritty_toml(content).or_else(|toml_error| {
            Self::from_alacritty_yaml(content).map_err(|yaml_error| {
                format!("Failed to parse theme file as TOML or YAML:\n{toml_error}\n{yaml_error}")
            })
        })
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
        let mut colors = alacritty.colors;
        for (field, color) in [
            ("primary.background", &mut colors.primary.background),
            ("primary.foreground", &mut colors.primary.foreground),
            ("normal.black", &mut colors.normal.black),
            ("normal.blue", &mut colors.normal.blue),
            ("normal.cyan", &mut colors.normal.cyan),
            ("normal.white", &mut colors.normal.white),
        ] {
            let hex = color.strip_prefix('#').or_else(|| color.strip_prefix("0x"));
            match hex {
                Some(hex) if hex.len() == 6 && hex.bytes().all(|c| c.is_ascii_hexdigit()) => {
                    *color = format!("#{hex}");
                }
                _ => {
                    return Err(format!(
                        "Invalid Alacritty color colors.{field}: {color:?} (expected #RRGGBB or 0xRRGGBB)"
                    ));
                }
            }
        }

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
            table_header_opacity: 0.08,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Theme;

    #[test]
    fn alacritty_colors_are_validated_and_hex_prefixes_normalized() {
        let content = include_str!("../themes/dracula.toml");
        for (value, field) in [
            ("#282a36", "primary.background"),
            ("#f8f8f2", "primary.foreground"),
            ("#000000", "normal.black"),
            ("#bd93f9", "normal.blue"),
            ("#8be9fd", "normal.cyan"),
            ("#bbbbbb", "normal.white"),
        ] {
            let error = Theme::from_alacritty(&content.replace(value, "not-a-color")).unwrap_err();
            assert!(
                error.contains(&format!("Invalid Alacritty color colors.{field}")),
                "{error}"
            );
        }
        let theme = Theme::from_alacritty(&content.replace("'#", "'0x")).unwrap();
        assert_eq!(theme.background_color, "#282a36");
        assert_eq!(theme.text_color, "#f8f8f2");
    }

    #[test]
    fn from_builtin_accepts_hyphenated_and_case_insensitive_names() {
        let underscore = Theme::from_builtin("solarized_light").expect("underscore variant");
        let hyphen = Theme::from_builtin("Solarized-Light").expect("hyphen variant");

        assert_eq!(underscore.background_color, hyphen.background_color);
        assert_eq!(underscore.text_color, hyphen.text_color);
    }

    #[test]
    fn fallback_uses_github_light_for_unknown_builtin_theme() {
        let fallback = Theme::from_builtin_or_fallback("does_not_exist");
        let github_light = Theme::github_light();

        assert_eq!(fallback.background_color, github_light.background_color);
        assert_eq!(fallback.text_color, github_light.text_color);
        assert_eq!(fallback.code_bg_color, github_light.code_bg_color);
    }
}
