// Compiled separately against the built public library; no production modules
// or private helpers are imported into the audit.
use markie::{fonts::{CosmicTextMeasure, TextMeasure}, renderer::Renderer, theme::Theme};

fn main() {
    if std::env::args().nth(1).as_deref() == Some("--measure") {
        let args: Vec<String> = std::env::args().collect();
        let (width, _) = CosmicTextMeasure::new().unwrap().measure_text(
            &args[2], args[3].parse().unwrap(), args[4] == "true",
            args[5] == "true", args[6] == "true", None,
        );
        println!("{width}");
        return;
    }
    if std::env::args().nth(1).as_deref() == Some("--raster") {
        for path in std::env::args().skip(2) {
            let svg = std::fs::read_to_string(&path).unwrap();
            match markie::export::svg_to_png(&svg, 1.0) {
                Ok(png) => std::fs::write(std::path::Path::new(&path).with_extension("png"), png).unwrap(),
                Err(error) => eprintln!("{}: {}", path, error),
            }
        }
        return;
    }
    let mut renderer = Renderer::new(Theme::default(), CosmicTextMeasure::new().unwrap(), 1200.0).unwrap();
    renderer.render("Alpha").unwrap();
    let second = renderer.render("Beta").unwrap();
    std::fs::write(std::env::args().nth(1).unwrap(), second).unwrap();
}
