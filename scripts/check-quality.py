#!/usr/bin/env python3
"""Check exported content, not pixel similarity or general layout correctness."""

import math
from pathlib import Path
import re
import struct
import sys
import xml.etree.ElementTree as ET


def check(directory, png_scale):
    ns = {"s": "http://www.w3.org/2000/svg"}
    labels = {
        "smoke_mermaid": ["Start", "Working?", "Done", "Retry", "Alice", "Bob",
                          "Hello", "Hi", "Animal", "Dog", "makeSound()", "bark()",
                          "Idle", "Processing", "CUSTOMER", "ORDER"],
        "smoke_math": ["Math Smoke Test"],
        "quality_flowchart": ["Receive request", "Check policy", "Second line",
                              "Third line", "Fourth line", "Publish result",
                              "Review", "承認待ち 日本語 中文", "Release", "approved",
                              "Queued", "Running", "Finished", "retry", "heartbeat",
                              "After the diagram:"],
        "quality_document": ["Component", "Responsibility", "Recovery policy",
                             "Evidence", "Collector", "Reconciler", "Publisher",
                             "Delivery receipt", "fn", "publish", "attempt",
                             "Collect customer events",
                             "Reconcile expected and observed state",
                             "Publish delivery receipt", "After content:"],
        "quality_compatibility": ["Note", "Warning", "Review item", "Document",
                                  "diagrams", "Draft", "Reviewed", "markie --help",
                                  "Review packet", "Signed", "Ready", "Red notice",
                                  "then plain text", "Legal", "Security", "Publish",
                                  "Author", "Renderer", "Render source", "Deliver preview",
                                  "Renderable", "Export", "produces", "0..*", "render()",
                                  "fromSource()", "validate()", "bool",
                                  "After compatibility content:"],
    }
    stems = [("smoke_math", 1200), ("smoke_mermaid", 1200)]
    # Match smoke-test.sh: retain 640px cases that fit, give the long LR
    # document readable headroom at 800px, and test its 640px rejection below.
    stems += [(f"{stem}_{width}", width) for stem in labels if stem.startswith("quality_")
              for width in ((800, 1200) if stem == "quality_document" else (640, 1200))]
    for stem, width in stems:
        root = ET.parse(directory / f"{stem}.svg").getroot()
        svg_width, svg_height = map(float, (root.attrib["width"], root.attrib["height"]))
        assert svg_width == width and math.isfinite(svg_height) and svg_height > 0, stem
        text_nodes = root.findall(".//s:text", ns)
        text = " ".join(" ".join("".join(node.itertext()).split()) for node in text_nodes)
        key = stem.rsplit("_", 1)[0] if stem.startswith("quality_") else stem
        for label in labels[key]:
            # Table cells and highlighted code can split even a single word
            # into several SVG text elements. Preserve content across wrapping.
            assert "".join(label.split()) in "".join(text.split()), f"{stem}: lost text {label!r}"
        for group in root.findall(".//s:g", ns):
            scale = re.search(r"\bscale\(([0-9.]+)\)", group.get("transform", ""))
            if scale:
                sizes = [float(node.attrib["font-size"]) * float(scale[1])
                         for node in group.findall(".//s:text", ns)]
                # The renderer writes group scales to four decimal places.
                assert all(size >= 9.999 for size in sizes), f"{stem}: unreadable diagram text {sizes}"
        if key == "quality_flowchart":
            assert "flowchart LR" not in text and "<br" not in text, stem
            # A leading comment must not cause a vertical graph. These labels
            # share a diagram transform, so their local coordinates suffice.
            start = next(n for n in text_nodes if "".join(n.itertext()) == "Receive request")
            end = next(n for n in text_nodes if "".join(n.itertext()) == "Publish result")
            assert float(start.attrib["x"]) < float(end.attrib["x"]), f"{stem}: lost LR order"
            assert abs(float(start.attrib["y"]) - float(end.attrib["y"])) < 1, f"{stem}: lost LR direction"
        with (directory / f"{stem}.png").open("rb") as image:
            header = image.read(24)
        assert header[:8] == b"\x89PNG\r\n\x1a\n", f"{stem}: invalid PNG signature"
        png_width, png_height = struct.unpack(">II", header[16:24])
        assert abs(png_width - svg_width * png_scale) <= 1, f"{stem}: PNG width"
        assert abs(png_height - svg_height * png_scale) <= 1, f"{stem}: PNG height"
        with (directory / f"{stem}.pdf").open("rb") as document:
            assert document.read(5) == b"%PDF-", f"{stem}: invalid PDF signature"
        print(f"OK {stem}: SVG content (LR where applicable), PNG dimensions, PDF signature")

    source_lines = (directory / "quality_document.md").read_text().splitlines()
    fence_line = source_lines.index("```mermaid") + 1
    for extension in ("svg", "png", "pdf"):
        rejected = directory / f"rejected_document_640.{extension}"
        assert not rejected.exists(), f"{rejected.name}: rejected output was written"
        diagnostic = Path(f"{rejected}.stderr").read_text()
        match = re.search(
            r"Mermaid diagram \(line (\d+)\): fitting this diagram would reduce text "
            r"to (\d+\.\d{3})px, below the 10px readability minimum\. "
            r"Increase --width to at least (\d+)", diagnostic)
        assert match, f"{rejected.name}: missing useful readability diagnostic: {diagnostic}"
        line, effective_size, suggested_width = match.groups()
        assert int(line) == fence_line, diagnostic
        assert 0 < float(effective_size) < 10, diagnostic
        assert 640 < int(suggested_width) <= 800, diagnostic
        assert "shorten labels, split the diagram" in diagnostic, diagnostic
        assert "markie-mermaid" in diagnostic, diagnostic
        # The same unchanged corpus has already passed all content/format checks
        # at 800px above, verifying the proposed wider retry really renders.
        print(f"OK {rejected.name}: source line, effective size, width advice, 800px retry")


if __name__ == "__main__":
    check(Path(sys.argv[1]), float(sys.argv[2]))
