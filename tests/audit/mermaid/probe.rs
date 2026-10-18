use markie::fonts::CosmicTextMeasure;
use markie::mermaid::{DiagramStyle, parse_mermaid, render_diagram};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let source = std::fs::read_to_string(&args[1]).unwrap();
    let ast = parse_mermaid(&source);
    std::fs::write(format!("{}.ast.txt", args[2]), format!("{ast:#?}")).unwrap();
    let mut measure = CosmicTextMeasure::new().unwrap();
    match render_diagram(&source, &DiagramStyle::default(), &mut measure) {
        Ok((fragment, width, height)) => {
            std::fs::write(format!("{}.fragment.svg", args[2]), format!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\">{fragment}</svg>"
            )).unwrap();
            println!("OK {width} {height}");
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
