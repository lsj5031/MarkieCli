use markie::fonts::{CosmicTextMeasure, TextMeasure};

fn main() {
    let mode = std::env::args().nth(1).expect("probe mode");
    match mode.as_str() {
        "font-export-families" => {
            let sans = std::env::var("AUDIT_SANS").unwrap();
            let mono = std::env::var("AUDIT_MONO").unwrap();
            let svg = |family: &str| format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="500" height="80"><text x="10" y="45" font-size="28" font-family="{}">Wide WWW iii spaced text</text></svg>"#, markie::xml::escape_xml(family));
            for (generic, explicit) in [("sans-serif", &sans), ("serif", &sans), ("monospace", &mono)] {
                let actual = markie::export::svg_to_png(&svg(generic), 1.0).unwrap();
                let expected = markie::export::svg_to_png(&svg(explicit), 1.0).unwrap();
                assert!(actual == expected, "{generic} export differs from explicit {explicit}");
                println!("{generic} raster matches explicit {explicit}");
            }
        }
        "math-error" => {
            let mut measure = CosmicTextMeasure::new().unwrap();
            let result = markie::math::render_math(r"\frac{a}{", 16.0, "#123456", &mut measure, false);
            println!("{result:?}");
            assert!(result.unwrap_err().contains("LaTeX parse error"));
        }
        "math-unknown" => {
            let mut measure = CosmicTextMeasure::new().unwrap();
            let result = markie::math::render_math(r"\definitelyunknown{x}", 16.0, "#123456", &mut measure, false);
            println!("{result:?}");
            assert!(result.is_err(), "unknown command returned Ok with visible parser-error text");
        }
        "math-position" => {
            let mut measure = CosmicTextMeasure::new().unwrap();
            let a = markie::math::render_math_at("x^7", 16.0, "#123456", &mut measure, false, 37.0, 89.0).unwrap();
            println!("{a:?}");
            assert!(a.svg_fragment.contains("x=\"37.00\" y=\"89.00\""));
            assert!(a.svg_fragment.contains("#123456"));
            assert!(a.width.is_finite() && a.width > 0.0 && a.ascent > 0.0 && a.descent > 0.0);
        }
        "math-escaped-script" => {
            let mut measure = CosmicTextMeasure::new().unwrap();
            for expression in [r"a_b^c", r"\_a^b"] {
                let rendered = markie::math::render_math(expression, 16.0, "black", &mut measure, false).unwrap();
                println!("{expression}: {}", rendered.svg_fragment);
                assert!(!rendered.svg_fragment.contains("MarkieScriptSlot"), "internal marker leaked for {expression}");
            }
        }
        "math-color" => {
            let color = "#123456\" injected=\"yes";
            let mut measure = CosmicTextMeasure::new().unwrap();
            let direct = markie::math::render_math(r"\frac{x}{y}", 16.0, color, &mut measure, false).unwrap();
            println!("direct: {}", direct.svg_fragment);
            let mut theme = markie::theme::Theme::default();
            theme.text_color = color.into();
            let mut renderer = markie::renderer::Renderer::new(theme, measure, 720.0).unwrap();
            let markdown = renderer.render("Inline $x$.\n\n$$\n\\frac{x}{y}\n$$\n").unwrap();
            println!("markdown: {markdown}");
            assert!(!direct.svg_fragment.contains("\" injected=\""), "direct math injected an attribute");
            assert!(!markdown.contains("\" injected=\""), "Markdown math injected an attribute");
            assert!(direct.svg_fragment.contains("#123456&quot; injected=&quot;yes"));
            for color in ["rebeccapurple", "rgb(12, 34, 56)"] {
                let mut measure = CosmicTextMeasure::new().unwrap();
                let rendered = markie::math::render_math(r"\frac{x}{y}", 16.0, color, &mut measure, false).unwrap();
                assert!(rendered.svg_fragment.contains(&format!("fill=\"{color}\"")));
                assert!(rendered.svg_fragment.contains(&format!("stroke=\"{color}\"")));
            }
        }
        "png-invalid-svg" => {
            let err = markie::export::svg_to_png("not svg", 1.0).unwrap_err();
            println!("{err}");
            assert!(err.contains("Failed to parse SVG"));
        }
        "pdf-invalid-svg" => {
            let err = markie::export::svg_to_pdf("not svg").unwrap_err();
            println!("{err}");
            assert!(err.contains("Failed to parse SVG"));
        }
        "png-invalid-scale" => {
            for scale in [0.0, -1.0, f32::NAN, f32::INFINITY] {
                let err = markie::export::svg_to_png("not svg", scale).unwrap_err();
                println!("{err}");
                assert!(err.contains("Invalid --png-scale"));
            }
        }
        "font-size" | "font-wrap" | "font-cache" | "font-styles" => {
            let mut measure = CosmicTextMeasure::new().unwrap();
            let sample = "Wide words with varied lengths";
            let normal = measure.measure_text(sample, 16.0, false, false, false, None);
            assert!(normal.0.is_finite() && normal.0 > 0.0 && normal.1 > 0.0);
            let value = match mode.as_str() {
                "font-size" => {
                    let larger = measure.measure_text(sample, 32.0, false, false, false, None);
                    assert!(larger.0 > normal.0 * 1.8 && larger.1 > normal.1 * 1.8);
                    larger
                }
                "font-wrap" => {
                    let wrapped = measure.measure_text(sample, 16.0, false, false, false, Some(85.0));
                    assert!(wrapped.0 <= 85.0 && wrapped.1 > normal.1);
                    wrapped
                }
                "font-cache" => {
                    let mut other = CosmicTextMeasure::new().unwrap();
                    let repeat = other.measure_text(sample, 16.0, false, false, false, None);
                    assert_eq!(normal, repeat);
                    repeat
                }
                _ => {
                    for (code, bold, italic) in [(true,false,false), (false,true,false), (false,false,true)] {
                        let styled = measure.measure_text(sample, 16.0, code, bold, italic, None);
                        assert!(styled.0.is_finite() && styled.0 > 0.0 && styled.1 > 0.0);
                    }
                    for (code, family) in [(false, "sans-serif"), (true, "monospace")] {
                        for bold in [false, true] {
                            let text = "MMMMMMMMMM";
                            let advance = measure.measure_text(text, 35.2, code, bold, false, None).0;
                            let svg = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="600" height="90"><text x="10" y="60" font-size="35.2" font-family="{family}" font-weight="{}">{text}</text></svg>"#, if bold {700} else {400});
                            let png = markie::export::svg_to_png(&svg, 1.0).unwrap();
                            let name = format!("host-{family}-{bold}");
                            std::fs::write(format!("{name}.png"), png).unwrap();
                            println!("family-advance:{name}:{advance}");
                        }
                    }
                    normal
                }
            };
            println!("normal={normal:?}, measured={value:?}");
        }
        "xml" => {
            assert_eq!(markie::xml::escape_xml("<&>\"'\u{7}\t\n\r漢😀"), "&lt;&amp;&gt;&quot;&apos;\t\n\r漢😀");
            assert_eq!(markie::xml::sanitize_xml_text("a\u{1}\u{b}\u{fffe}\u{ffff}b"), "ab");
            println!("All XML entities, controls, valid whitespace and Unicode checked");
        }
        "theme-errors" => {
            assert!(markie::theme::Theme::from_builtin("missing").unwrap_err().contains("Unknown built-in theme"));
            assert!(markie::theme::Theme::from_alacritty_toml("{").unwrap_err().contains("Failed to parse Alacritty TOML"));
            assert!(markie::theme::Theme::from_alacritty_yaml("{").unwrap_err().contains("Failed to parse Alacritty YAML"));
            println!("Builtin/TOML/YAML library errors retain format context");
        }
        _ => panic!("unknown probe: {mode}"),
    }
}
