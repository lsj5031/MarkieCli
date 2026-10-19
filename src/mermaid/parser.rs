use std::collections::HashMap;

use super::types::*;

#[derive(Debug, Clone)]
pub enum MermaidDiagram {
    Flowchart(Flowchart),
    Sequence(SequenceDiagram),
    ClassDiagram(ClassDiagram),
    StateDiagram(StateDiagram),
    ErDiagram(ErDiagram),
}

/// Parse a mermaid diagram from source text
pub fn parse_mermaid(input: &str) -> Result<MermaidDiagram, String> {
    let input = input.trim();

    // Detect the diagram type from the first non-empty, non-comment line so
    // leading `%%` comments or init config blocks don't confuse detection.
    let first_line = input
        .lines()
        .find(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with("%%")
        })
        .map(str::trim)
        .unwrap_or("");

    let family = first_line
        .split(|c: char| c.is_whitespace() || c == ';')
        .next()
        .unwrap_or("");
    if matches!(family, "flowchart" | "graph") {
        let diagram =
            parse_flowchart(input).map_err(|e| format!("Flowchart parse error: {}", e))?;
        Ok(MermaidDiagram::Flowchart(diagram))
    } else if matches!(family, "sequenceDiagram" | "sequence") {
        let diagram = parse_sequence(input).map_err(|e| format!("Sequence parse error: {}", e))?;
        Ok(MermaidDiagram::Sequence(diagram))
    } else if matches!(family, "classDiagram" | "class") {
        let diagram = parse_class(input).map_err(|e| format!("Class parse error: {}", e))?;
        Ok(MermaidDiagram::ClassDiagram(diagram))
    } else if matches!(family, "stateDiagram" | "stateDiagram-v2" | "state") {
        let diagram = parse_state(input).map_err(|e| format!("State parse error: {}", e))?;
        Ok(MermaidDiagram::StateDiagram(diagram))
    } else if matches!(family, "erDiagram" | "er") {
        let diagram = parse_er(input).map_err(|e| format!("ER parse error: {}", e))?;
        Ok(MermaidDiagram::ErDiagram(diagram))
    } else if first_line.is_empty() {
        // Empty diagram (e.g. a blank ```mermaid fence): render an empty
        // flowchart instead of failing the whole document.
        let diagram =
            parse_flowchart(input).map_err(|e| format!("Flowchart parse error: {}", e))?;
        Ok(MermaidDiagram::Flowchart(diagram))
    } else {
        // Unknown diagram types (pie, gantt, journey, ...) are not supported.
        // Error loudly instead of producing a confusing flowchart fallback.
        let diagram_type = first_line.split_whitespace().next().unwrap_or(first_line);
        Err(format!(
            "Unsupported Mermaid diagram type '{}'. Supported types: flowchart, sequenceDiagram, classDiagram, stateDiagram, erDiagram",
            diagram_type
        ))
    }
}

// ============================================
// FLOWCHART PARSER
// ============================================

fn parse_flowchart(input: &str) -> Result<Flowchart, String> {
    let mut statements = Vec::new();
    for line in input
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("%%"))
    {
        let syntax = flow_syntax(line)?;
        let mut start = 0;
        for (end, _) in syntax.match_indices(';') {
            statements.push(line[start..end].trim());
            start = end + 1;
        }
        statements.push(line[start..].trim());
    }
    let mut lines = statements.into_iter().filter(|line| !line.is_empty());

    // Consume the same non-comment header used for diagram type detection.
    let first_line = lines.next().unwrap_or("");
    let direction = parse_flow_direction(first_line);

    let mut nodes: Vec<FlowchartNode> = Vec::new();
    let mut edges: Vec<FlowchartEdge> = Vec::new();
    let mut subgraphs: Vec<Subgraph> = Vec::new();
    let mut current_subgraph: Option<Subgraph> = None;
    for line in lines {
        // Subgraph start
        if line.starts_with("subgraph ") {
            let raw_title = line.strip_prefix("subgraph ").unwrap_or("").trim();
            if let Some(parent) = &current_subgraph {
                return Err(format!(
                    "Nested subgraphs are not supported: subgraph '{}' is inside '{}'. Use separate, non-nested subgraphs.",
                    raw_title, parent.title
                ));
            }
            let title = if let Some(bracket_pos) = raw_title.find("[\"") {
                let after = &raw_title[bracket_pos + 2..];
                if let Some(end) = after.find("\"]") {
                    after[..end].to_string()
                } else {
                    raw_title.to_string()
                }
            } else {
                raw_title.to_string()
            };
            let id = format!("subgraph_{}", subgraphs.len());
            current_subgraph = Some(Subgraph {
                id: id.clone(),
                title,
                nodes: Vec::new(),
            });
            continue;
        }

        // Subgraph end
        if line == "end" {
            if let Some(sg) = current_subgraph.take() {
                subgraphs.push(sg);
            }
            continue;
        }

        if ["style ", "classDef ", "class ", "linkStyle ", "direction "]
            .iter()
            .any(|prefix| line.starts_with(prefix))
        {
            return Err(format!("Flowchart directives are not supported: {line}"));
        }
        if flow_syntax(line)?.contains(']') {
            return Err(format!(
                "Asymmetric flowchart shapes are not supported: {line}"
            ));
        }
        if let Some((parsed_nodes, parsed_edges)) = parse_edge_line(line) {
            for parsed in parsed_nodes {
                if let Some(node) = nodes.iter_mut().find(|node| node.id == parsed.id) {
                    if parsed.explicit {
                        node.label = parsed.label;
                        node.shape = parsed.shape;
                    }
                } else {
                    nodes.push(FlowchartNode {
                        id: parsed.id.clone(),
                        label: parsed.label,
                        shape: parsed.shape,
                    });
                }
                if let Some(sg) = &mut current_subgraph {
                    if !sg.nodes.contains(&parsed.id) {
                        sg.nodes.push(parsed.id);
                    }
                }
            }
            edges.extend(parsed_edges);
        }
    }

    Ok(Flowchart {
        direction,
        nodes,
        edges,
        subgraphs,
    })
}

fn parse_flow_direction(line: &str) -> FlowDirection {
    let line = line.to_lowercase();
    if line.contains("tb") || line.contains("td") {
        FlowDirection::TopDown
    } else if line.contains("bt") {
        FlowDirection::BottomUp
    } else if line.contains("lr") {
        FlowDirection::LeftRight
    } else if line.contains("rl") {
        FlowDirection::RightLeft
    } else {
        FlowDirection::TopDown
    }
}

struct ParsedNodeInfo {
    id: String,
    label: String,
    shape: NodeShape,
    explicit: bool,
}

// Preserve byte offsets while hiding user text from shorthand tokenization.
fn flow_syntax(line: &str) -> Result<String, String> {
    let mut syntax = String::new();
    let mut depth = 0_usize;
    let mut quoted = false;
    let mut escaped = false;
    let mut edge_label = false;
    for c in line.chars() {
        let protected = quoted || edge_label || depth > 0;
        if escaped {
            escaped = false;
        } else if c == '\\' && quoted {
            escaped = true;
        } else if c == '"' {
            quoted = !quoted;
        } else if !quoted {
            if c == '|' && depth == 0 {
                edge_label = !edge_label;
            } else if !edge_label {
                match c {
                    '[' | '(' | '{' => depth += 1,
                    ']' | ')' | '}' if depth > 0 => depth -= 1,
                    _ => {}
                }
            }
        }
        if protected || quoted || edge_label || depth > 0 || matches!(c, '"' | '|') {
            syntax.extend(std::iter::repeat_n(' ', c.len_utf8()));
        } else {
            syntax.push(c);
        }
    }
    if quoted || edge_label || depth > 0 {
        return Err(format!("Unclosed flowchart label or shape: {line}"));
    }
    Ok(syntax)
}

fn parse_edge_line(line: &str) -> Option<(Vec<ParsedNodeInfo>, Vec<FlowchartEdge>)> {
    // Edge patterns (order matters - longer patterns first)
    let patterns = [
        ("<==>", EdgeStyle::Thick, ArrowType::Arrow, ArrowType::Arrow),
        ("<-->", EdgeStyle::Solid, ArrowType::Arrow, ArrowType::Arrow),
        (
            "<.->",
            EdgeStyle::Dotted,
            ArrowType::Arrow,
            ArrowType::Arrow,
        ),
        ("x==x", EdgeStyle::Thick, ArrowType::Cross, ArrowType::Cross),
        (
            "o==o",
            EdgeStyle::Thick,
            ArrowType::Circle,
            ArrowType::Circle,
        ),
        ("x--x", EdgeStyle::Solid, ArrowType::Cross, ArrowType::Cross),
        (
            "o--o",
            EdgeStyle::Solid,
            ArrowType::Circle,
            ArrowType::Circle,
        ),
        (
            "x-.x",
            EdgeStyle::Dotted,
            ArrowType::Cross,
            ArrowType::Cross,
        ),
        (
            "o-.o",
            EdgeStyle::Dotted,
            ArrowType::Circle,
            ArrowType::Circle,
        ),
        ("<==", EdgeStyle::Thick, ArrowType::None, ArrowType::Arrow),
        ("<--", EdgeStyle::Solid, ArrowType::None, ArrowType::Arrow),
        ("<-.", EdgeStyle::Dotted, ArrowType::None, ArrowType::Arrow),
        (
            "o-->",
            EdgeStyle::Solid,
            ArrowType::Arrow,
            ArrowType::Circle,
        ),
        (
            "--o>",
            EdgeStyle::Solid,
            ArrowType::Arrow,
            ArrowType::Circle,
        ),
        (
            "o==>",
            EdgeStyle::Thick,
            ArrowType::Arrow,
            ArrowType::Circle,
        ),
        (
            "o-.->",
            EdgeStyle::Dotted,
            ArrowType::Arrow,
            ArrowType::Circle,
        ),
        ("==o", EdgeStyle::Thick, ArrowType::Circle, ArrowType::None),
        ("==x", EdgeStyle::Thick, ArrowType::Cross, ArrowType::None),
        ("o==", EdgeStyle::Thick, ArrowType::None, ArrowType::Circle),
        ("x==", EdgeStyle::Thick, ArrowType::None, ArrowType::Cross),
        ("==>", EdgeStyle::Thick, ArrowType::Arrow, ArrowType::None),
        ("--x", EdgeStyle::Solid, ArrowType::Cross, ArrowType::None),
        ("x--", EdgeStyle::Solid, ArrowType::None, ArrowType::Cross),
        ("--o", EdgeStyle::Solid, ArrowType::Circle, ArrowType::None),
        ("o--", EdgeStyle::Solid, ArrowType::None, ArrowType::Circle),
        ("-.->", EdgeStyle::Dotted, ArrowType::Arrow, ArrowType::None),
        ("-.x", EdgeStyle::Dotted, ArrowType::Cross, ArrowType::None),
        ("x-.", EdgeStyle::Dotted, ArrowType::None, ArrowType::Cross),
        ("-.o", EdgeStyle::Dotted, ArrowType::Circle, ArrowType::None),
        ("o-.", EdgeStyle::Dotted, ArrowType::None, ArrowType::Circle),
        ("-.-", EdgeStyle::Dotted, ArrowType::None, ArrowType::None),
        ("-->", EdgeStyle::Solid, ArrowType::Arrow, ArrowType::None),
        ("---", EdgeStyle::Solid, ArrowType::None, ArrowType::None),
        ("->>", EdgeStyle::Solid, ArrowType::Arrow, ArrowType::Arrow),
        ("->", EdgeStyle::Solid, ArrowType::Arrow, ArrowType::None),
        ("--", EdgeStyle::Solid, ArrowType::None, ArrowType::None),
    ];

    let syntax = flow_syntax(line).ok()?;
    let mut operators = Vec::new();
    let mut cursor = 0;
    while let Some((pos, index)) = patterns
        .iter()
        .enumerate()
        .filter_map(|(i, (p, ..))| {
            syntax[cursor..]
                .match_indices(p)
                .map(|(n, _)| cursor + n)
                .find(|pos| {
                    // A trailing o/x in an unspaced identifier is not a tail marker.
                    !p.starts_with(['o', 'x'])
                        || (*pos > 0
                            && !syntax[..*pos].ends_with(|c: char| c.is_alphanumeric() || c == '_'))
                })
                .map(|pos| (pos, i))
        })
        .min_by_key(|(n, i)| (*n, std::cmp::Reverse(patterns[*i].0.len())))
    {
        operators.push((pos, index));
        cursor = pos + patterns[index].0.len();
    }
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut previous: Vec<String> = Vec::new();
    for segment in 0..=operators.len() {
        let start = if segment == 0 {
            0
        } else {
            let (p, i) = operators[segment - 1];
            p + patterns[i].0.len()
        };
        let end = operators.get(segment).map_or(line.len(), |(p, _)| *p);
        let mut part = line[start..end].trim();
        let mut label = None;
        if segment > 0
            && let Some(rest) = part.strip_prefix('|')
        {
            // Closing pipes inside quoted edge text are literal.
            let mut quoted = false;
            let mut escaped = false;
            let close = rest.char_indices().find_map(|(i, c)| {
                if escaped {
                    escaped = false;
                    return None;
                }
                if c == '\\' && quoted {
                    escaped = true;
                } else if c == '"' {
                    quoted = !quoted;
                } else if c == '|' && !quoted {
                    return Some(i);
                }
                None
            })?;
            label = Some(normalize_flowchart_label(&rest[..close]));
            part = rest[close + 1..].trim();
        }
        let masked = flow_syntax(part).ok()?;
        let mut group = Vec::new();
        let mut left = 0;
        for right in masked
            .match_indices('&')
            .map(|(i, _)| i)
            .chain(std::iter::once(part.len()))
        {
            let raw = part[left..right].trim();
            let (id, label, shape) = extract_node_info(raw)?;
            group.push(id.clone());
            nodes.push(ParsedNodeInfo {
                explicit: raw != id,
                id,
                label,
                shape,
            });
            left = right + 1;
        }
        if segment > 0 {
            let (_, index) = operators[segment - 1];
            let (_, style, head, tail) = &patterns[index];
            for from in &previous {
                for to in &group {
                    edges.push(FlowchartEdge {
                        from: from.clone(),
                        to: to.clone(),
                        label: label.clone(),
                        style: style.clone(),
                        arrow_head: head.clone(),
                        arrow_tail: tail.clone(),
                        min_length: 1,
                    });
                }
            }
        }
        previous = group;
    }
    Some((nodes, edges))
}

/// Extract node ID, label, and shape from a part of an edge definition
fn extract_node_info(part: &str) -> Option<(String, String, NodeShape)> {
    let part = part.trim();
    let id_end = part
        .char_indices()
        .take_while(|(_, c)| c.is_alphanumeric() || *c == '_' || *c == '-')
        .last()
        .map(|(i, c)| i + c.len_utf8())?;
    let id = part[..id_end].to_string();
    let definition = part[id_end..].trim();
    if definition.is_empty() {
        return Some((id.clone(), id, NodeShape::RoundedRect));
    }

    // Check for shaped node definitions inline
    let patterns: &[(&str, &str, NodeShape)] = &[
        ("(((", ")))", NodeShape::DoubleCircle),
        ("[[", "]]", NodeShape::Subroutine),
        ("((", "))", NodeShape::Circle),
        ("[(", ")]", NodeShape::Cylinder),
        ("([", "])", NodeShape::Stadium),
        ("[/", "/]", NodeShape::Parallelogram),
        ("[\\", "\\]", NodeShape::ParallelogramAlt),
        ("[/", "\\]", NodeShape::Trapezoid),
        ("[\\", "/]", NodeShape::TrapezoidAlt),
        ("{{", "}}", NodeShape::Hexagon),
        ("[", "]", NodeShape::Rect),
        ("(", ")", NodeShape::RoundedRect),
        ("{", "}", NodeShape::Rhombus),
    ];

    for (open, close, shape) in patterns {
        if let Some(label) = definition
            .strip_prefix(open)
            .and_then(|s| s.strip_suffix(close))
        {
            return Some((id, normalize_flowchart_label(label), shape.clone()));
        }
    }

    None
}

fn normalize_flowchart_label(raw: &str) -> String {
    let mut label = raw.trim().to_string();

    // Mermaid line-break tags should become actual newlines before layout/measurement.
    label = label
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("<br>", "\n");

    // Labels like A["Text"] should render as Text, not "Text".
    if let Some(inner) = label
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    {
        label = inner.trim().to_string();
    }

    label
}

// ============================================
// SEQUENCE DIAGRAM PARSER
// ============================================

fn parse_sequence(input: &str) -> Result<SequenceDiagram, String> {
    let lines: Vec<String> = input
        .lines()
        .skip(1)
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("%%"))
        .map(str::to_string)
        .collect();

    let mut participants: Vec<Participant> = Vec::new();
    let mut active: HashMap<String, usize> = HashMap::new();
    for line in &lines {
        // Participant declaration
        if line.starts_with("participant ") {
            let rest = line.strip_prefix("participant ").unwrap_or("");
            // Mermaid syntax: participant A as "Display Name" or participant A
            // The ID comes first, then optional "as Alias"
            let parts: Vec<&str> = rest.splitn(2, " as ").collect();
            let id = parts[0].trim();
            let alias = parts.get(1).map(|s| s.trim().trim_matches('"').to_string());
            if let Some(participant) = participants.iter_mut().find(|p| p.id == id) {
                if alias.is_some() {
                    participant.alias = alias;
                }
            } else {
                participants.push(Participant {
                    id: id.to_string(),
                    alias,
                });
            }
            continue;
        }
        if line.starts_with("actor ") || line.starts_with("and ") {
            return Err(format!(
                "Sequence actors and and-branches are not supported: {line}"
            ));
        }
        if let Some((message, activation)) = parse_sequence_message(line) {
            for endpoint in [&message.from, &message.to] {
                if !participants.iter().any(|p| &p.id == endpoint) {
                    participants.push(Participant {
                        id: endpoint.clone(),
                        alias: None,
                    });
                }
            }
            if activation == Some('+') {
                *active.entry(message.to).or_default() += 1;
            }
            if activation == Some('-') {
                let count = active.entry(message.from.clone()).or_default();
                if *count == 0 {
                    return Err(format!(
                        "Cannot deactivate inactive participant {}: {line}",
                        message.from
                    ));
                }
                *count -= 1;
            }
        }
        if let Some(id) = line.strip_prefix("activate ") {
            *active.entry(id.trim().to_string()).or_default() += 1;
        }
        if let Some(id) = line.strip_prefix("deactivate ") {
            let count = active.entry(id.trim().to_string()).or_default();
            if *count == 0 {
                return Err(format!(
                    "Cannot deactivate inactive participant {}: {line}",
                    id.trim()
                ));
            }
            *count -= 1;
        }
    }
    for line in &lines {
        if let Some(SequenceElement::Note { participant, .. }) = parse_sequence_note(line)
            && !participants.iter().any(|p| p.id == participant)
        {
            return Err(format!(
                "Multi-participant or undeclared-participant notes are not supported: {line}"
            ));
        }
    }
    let mut index = 0;
    let elements = parse_sequence_elements(&lines, &mut index, true);

    Ok(SequenceDiagram {
        participants,
        elements,
    })
}

fn parse_sequence_elements(
    lines: &[String],
    index: &mut usize,
    allow_break_tokens: bool,
) -> Vec<SequenceElement> {
    let mut elements = Vec::new();

    while *index < lines.len() {
        let line = lines[*index].trim();

        if allow_break_tokens && (line == "end" || line == "else" || line.starts_with("else ")) {
            break;
        }

        if line.starts_with("participant ") || line.starts_with("actor ") {
            *index += 1;
            continue;
        }

        if let Some((msg, activation)) = parse_sequence_message(line) {
            let participant = if activation == Some('+') {
                msg.to.clone()
            } else {
                msg.from.clone()
            };
            elements.push(SequenceElement::Message(msg));
            match activation {
                Some('+') => elements.push(SequenceElement::Activation(Activation { participant })),
                Some('-') => {
                    elements.push(SequenceElement::Deactivation(Activation { participant }))
                }
                _ => {}
            }
            *index += 1;
            continue;
        }

        if let Some(note) = parse_sequence_note(line) {
            elements.push(note);
            *index += 1;
            continue;
        }

        if let Some(bt) = parse_sequence_block_type(line) {
            let rest = line.split_once(' ').map(|(_, r)| r).unwrap_or("");
            let label = if let Some(colon_pos) = rest.find(':') {
                rest[colon_pos + 1..].trim().to_string()
            } else {
                rest.trim().to_string()
            };

            *index += 1;
            let messages = parse_sequence_elements(lines, index, true);

            let mut else_branches = Vec::new();
            while *index < lines.len() {
                let branch_line = lines[*index].trim();
                if branch_line == "else" || branch_line.starts_with("else ") {
                    let branch_label = branch_line
                        .strip_prefix("else")
                        .map(str::trim)
                        .unwrap_or("")
                        .to_string();
                    *index += 1;
                    let branch_messages = parse_sequence_elements(lines, index, true);
                    else_branches.push((branch_label, branch_messages));
                } else {
                    break;
                }
            }

            if *index < lines.len() && lines[*index].trim() == "end" {
                *index += 1;
            }

            elements.push(SequenceElement::Block(SequenceBlock {
                block_type: bt,
                label,
                messages,
                else_branches,
            }));
            continue;
        }

        if line.starts_with("activate ") {
            let participant = line.strip_prefix("activate ").unwrap_or("").trim();
            elements.push(SequenceElement::Activation(Activation {
                participant: participant.to_string(),
            }));
            *index += 1;
            continue;
        }

        if line.starts_with("deactivate ") {
            let participant = line.strip_prefix("deactivate ").unwrap_or("").trim();
            elements.push(SequenceElement::Deactivation(Activation {
                participant: participant.to_string(),
            }));
            *index += 1;
            continue;
        }

        *index += 1;
    }

    elements
}

fn parse_sequence_block_type(line: &str) -> Option<SequenceBlockType> {
    if line.starts_with("alt ") {
        Some(SequenceBlockType::Alt)
    } else if line.starts_with("opt ") {
        Some(SequenceBlockType::Opt)
    } else if line.starts_with("loop ") {
        Some(SequenceBlockType::Loop)
    } else if line.starts_with("par ") {
        Some(SequenceBlockType::Par)
    } else if line.starts_with("critical ") {
        Some(SequenceBlockType::Critical)
    } else {
        None
    }
}

fn parse_sequence_note(line: &str) -> Option<SequenceElement> {
    if !line.starts_with("Note ") {
        return None;
    }

    let rest = line.strip_prefix("Note ").unwrap_or("").trim();

    let (position, after_prefix) = if let Some(after) = rest.strip_prefix("right of ") {
        ("right", after)
    } else if let Some(after) = rest.strip_prefix("left of ") {
        ("left", after)
    } else {
        let after = rest.strip_prefix("over ")?;
        ("over", after)
    };

    let colon_pos = after_prefix.find(':')?;
    let participant = after_prefix[..colon_pos].trim();
    let text = after_prefix[colon_pos + 1..].trim();
    if participant.is_empty() {
        return None;
    }

    Some(SequenceElement::Note {
        participant: participant.to_string(),
        position: position.to_string(),
        text: text.to_string(),
    })
}

fn parse_sequence_message(line: &str) -> Option<(SequenceMessage, Option<char>)> {
    if parse_sequence_block_type(line).is_some() {
        return None;
    }
    // Order matters: longer patterns first to avoid partial matches
    let patterns = [
        ("-->>", MessageType::Dotted, MessageKind::Reply),
        ("->>", MessageType::Solid, MessageKind::Sync),
        (">>+", MessageType::Solid, MessageKind::Async),
        (">>-", MessageType::Solid, MessageKind::Async),
        ("-->", MessageType::Dotted, MessageKind::Plain),
        ("->", MessageType::Solid, MessageKind::Plain),
        ("--x", MessageType::Dotted, MessageKind::Cross),
        ("--)", MessageType::Dotted, MessageKind::Async),
        ("-x", MessageType::Solid, MessageKind::Cross),
        ("-)", MessageType::Solid, MessageKind::Async),
    ];

    let endpoints = line.split_once(':').map_or(line, |(head, _)| head);
    for (pattern, msg_type, kind) in &patterns {
        if let Some(pos) = endpoints.find(pattern) {
            let from = line[..pos].trim().to_string();
            let mut rest = line[pos + pattern.len()..].trim();
            let activation = rest.chars().next().filter(|c| matches!(c, '+' | '-'));
            if activation.is_some() {
                rest = rest[1..].trim_start();
            }

            // Parse "To: Label" or just "To"
            let (to, label) = if let Some(colon_pos) = rest.find(':') {
                let to_part = rest[..colon_pos].trim().to_string();
                let label_part = rest[colon_pos + 1..].trim().to_string();
                (to_part, Some(label_part))
            } else {
                (rest.trim().to_string(), None)
            };

            return Some((
                SequenceMessage {
                    from,
                    to,
                    label: label.unwrap_or_default(),
                    msg_type: msg_type.clone(),
                    kind: kind.clone(),
                },
                activation,
            ));
        }
    }

    None
}

// ============================================
// CLASS DIAGRAM PARSER
// ============================================

fn parse_class(input: &str) -> Result<ClassDiagram, String> {
    let mut lines = input.lines().skip(1); // Skip "classDiagram"

    let mut classes: Vec<ClassDefinition> = Vec::new();
    let mut relations: Vec<ClassRelation> = Vec::new();
    let mut current_class: Option<usize> = None;

    for (line_idx, line) in (&mut lines).enumerate() {
        let line_num = line_idx + 2; // +2: skip first line + 1-indexed
        let line = line.trim();
        if line.is_empty() || line.starts_with("%%") {
            continue;
        }

        // Class definition end
        if line == "}" {
            current_class = None;
            continue;
        }

        // Class definition start
        if line.starts_with("class ") {
            let rest = line.strip_prefix("class ").unwrap_or("");

            // Check for stereotype
            let (name, stereotype) = if rest.starts_with("<<") {
                let end = rest
                    .find(">>")
                    .ok_or_else(|| format!("line {}: missing '>>' in stereotype", line_num))?;
                let st = &rest[2..end];
                let after = rest[end + 2..].trim();
                (after.to_string(), Some(st.trim().to_string()))
            } else {
                (rest.trim().to_string(), None)
            };

            let index = ensure_class(&mut classes, name.trim_end_matches('{').trim());
            let class = &mut classes[index];
            if stereotype.is_some() {
                class.is_abstract = stereotype.as_deref() == Some("abstract");
                class.is_interface = stereotype.as_deref() == Some("interface");
                class.stereotype = stereotype;
            }
            if name.ends_with('{') {
                current_class = Some(index);
            }
            continue;
        }

        // If inside a class body
        if let Some(index) = current_class {
            let cls = &mut classes[index];
            // Stereotype annotation inside body (e.g. <<abstract>>, <<interface>>)
            if line.starts_with("<<") {
                if let Some(end) = line.find(">>") {
                    let stereo = line[2..end].trim().to_ascii_lowercase();
                    cls.stereotype = Some(line[2..end].trim().to_string());
                    if stereo == "abstract" {
                        cls.is_abstract = true;
                    } else if stereo == "interface" {
                        cls.is_interface = true;
                    }
                }
                continue;
            }

            // Attribute or method
            if line.starts_with('+')
                || line.starts_with('-')
                || line.starts_with('#')
                || line.starts_with('~')
            {
                let vis = match line.chars().next() {
                    Some('+') => Visibility::Public,
                    Some('-') => Visibility::Private,
                    Some('#') => Visibility::Protected,
                    Some('~') => Visibility::Package,
                    _ => Visibility::Public,
                };

                let member = &line[1..].trim();

                // Check if method (has parentheses)
                if member.contains('(') {
                    if let Some(method) = parse_class_method(vis, member) {
                        cls.methods.push(method);
                    }
                } else if let Some(attr) = parse_class_attribute(vis, member) {
                    cls.attributes.push(attr);
                }
            }
            continue;
        }

        // Relation
        if let Some(rel) = parse_class_relation(line) {
            ensure_class(&mut classes, &rel.from);
            ensure_class(&mut classes, &rel.to);
            relations.push(rel);
        } else if line.contains('"') {
            return Err(format!(
                "Invalid quoted class relation at line {line_num}: {line}"
            ));
        }
    }

    Ok(ClassDiagram { classes, relations })
}

fn ensure_class(classes: &mut Vec<ClassDefinition>, name: &str) -> usize {
    if let Some(index) = classes.iter().position(|class| class.name == name) {
        return index;
    }
    classes.push(ClassDefinition {
        name: name.to_string(),
        stereotype: None,
        is_abstract: false,
        is_interface: false,
        attributes: Vec::new(),
        methods: Vec::new(),
    });
    classes.len() - 1
}

fn parse_class_attribute(vis: Visibility, member: &str) -> Option<ClassAttribute> {
    let is_static = member.ends_with('$');
    let is_abstract = member.ends_with('*');
    let member = member.trim_end_matches(['$', '*']).trim();
    let parts: Vec<&str> = member.splitn(2, ':').collect();
    let name = parts[0].trim();
    let type_ann = parts.get(1).map(|s| s.trim().to_string());

    Some(ClassAttribute {
        member: ClassMember {
            visibility: vis,
            name: name.to_string(),
            is_static,
            is_abstract,
        },
        type_annotation: type_ann,
    })
}

fn parse_class_method(vis: Visibility, member: &str) -> Option<ClassMethod> {
    let trailing_static = member.ends_with('$');
    let trailing_abstract = member.ends_with('*');
    let member = member.trim_end_matches(['$', '*']).trim();
    let paren_pos = member.find('(')?;
    let name = member[..paren_pos].trim();

    let close_paren = member.find(')')?;
    let params_str = &member[paren_pos + 1..close_paren];

    let after = member[close_paren + 1..].trim();
    let is_static = trailing_static || after.starts_with('$');
    let is_abstract = trailing_abstract || after.starts_with('*');
    let after = after.trim_start_matches(['$', '*']).trim();
    let after = after.strip_prefix(':').unwrap_or(after).trim();
    let return_type = (!after.is_empty()).then(|| after.to_string());

    let parameters = if params_str.is_empty() {
        Vec::new()
    } else {
        params_str
            .split(',')
            .filter_map(|p| {
                let parts: Vec<&str> = p.trim().splitn(2, ':').collect();
                if parts.is_empty() {
                    None
                } else {
                    Some((
                        parts[0].trim().to_string(),
                        parts.get(1).map(|s| s.trim().to_string()),
                    ))
                }
            })
            .collect()
    };

    Some(ClassMethod {
        member: ClassMember {
            visibility: vis,
            name: name.to_string(),
            is_static,
            is_abstract,
        },
        parameters,
        return_type,
    })
}

fn parse_class_relation(line: &str) -> Option<ClassRelation> {
    let patterns = [
        ("<|--", ClassRelationType::Inheritance),
        ("<|..", ClassRelationType::Realization),
        ("*--", ClassRelationType::Composition),
        ("o--", ClassRelationType::Aggregation),
        ("-->", ClassRelationType::Association),
        ("--", ClassRelationType::Association),
        ("..>", ClassRelationType::Dependency),
        ("..|>", ClassRelationType::Realization),
        ("..", ClassRelationType::Dependency),
    ];

    let mut syntax = String::new();
    let mut quoted = false;
    let mut escaped = false;
    for c in line.chars() {
        let protected = quoted || c == '"';
        if escaped {
            escaped = false;
        } else if c == '\\' && quoted {
            escaped = true;
        } else if c == '"' {
            quoted = !quoted;
        }
        if protected {
            syntax.extend(std::iter::repeat_n(' ', c.len_utf8()));
        } else {
            syntax.push(c);
        }
    }
    if quoted {
        return None;
    }
    let end = syntax.find(':').unwrap_or(line.len());
    let label = (end < line.len())
        .then(|| line[end + 1..].trim().trim_matches('"').to_string())
        .filter(|s| !s.is_empty());
    for (pattern, rel_type) in &patterns {
        if let Some((pos, _)) = syntax[..end].match_indices(pattern).find(|(pos, _)| {
            !pattern.starts_with('o')
                || (*pos > 0
                    && !syntax[..*pos].ends_with(|c: char| c.is_alphanumeric() || c == '_'))
        }) {
            let (mut from, mut multiplicity_from) = parse_class_endpoint(&line[..pos], true)?;
            let (mut to, mut multiplicity_to) =
                parse_class_endpoint(&line[pos + pattern.len()..end], false)?;
            if *pattern == "<|.." {
                std::mem::swap(&mut from, &mut to);
                std::mem::swap(&mut multiplicity_from, &mut multiplicity_to);
            }

            return Some(ClassRelation {
                from,
                to,
                relation_type: rel_type.clone(),
                directed: !matches!(*pattern, "--" | ".."),
                label,
                multiplicity_from,
                multiplicity_to,
            });
        }
    }

    None
}

fn parse_class_endpoint(part: &str, source: bool) -> Option<(String, Option<String>)> {
    let part = part.trim();
    let (name, multiplicity) = if let Some(first) = part.find('"') {
        let last = part.rfind('"')?;
        if last <= first {
            return None;
        }
        let name = if source {
            if !part[last + 1..].trim().is_empty() {
                return None;
            }
            part[..first].trim()
        } else {
            if !part[..first].trim().is_empty() {
                return None;
            }
            part[last + 1..].trim()
        };
        (name, Some(part[first + 1..last].replace("\\\"", "\"")))
    } else {
        (part, None)
    };
    if name.is_empty() || name.chars().any(char::is_whitespace) {
        return None;
    }
    Some((name.to_string(), multiplicity))
}

// ============================================
// STATE DIAGRAM PARSER
// ============================================

fn parse_state(input: &str) -> Result<StateDiagram, String> {
    const START_STATE_ID: &str = "__start__";
    const END_STATE_ID: &str = "__end__";

    let mut lines = input.lines().skip(1); // Skip "stateDiagram"

    let mut states: Vec<State> = Vec::new();
    let mut transitions: Vec<StateTransition> = Vec::new();
    let mut composite_stack: Vec<String> = Vec::new();

    for (line_idx, line) in (&mut lines).enumerate() {
        let line_num = line_idx + 2; // +2: skip first line + 1-indexed
        let line = line.trim();
        if line.is_empty() || line.starts_with("%%") {
            continue;
        }

        if line == "}" {
            composite_stack.pop();
            continue;
        }

        if let Some((state_id, text, position)) = parse_state_note(line) {
            add_state_note(&mut states, &state_id, text, position);
            continue;
        }

        // State definition
        if line.starts_with("state ") {
            let rest = line.strip_prefix("state ").unwrap_or("");

            let (id, label, is_composite) =
                parse_state_definition(rest).map_err(|e| format!("line {}: {}", line_num, e))?;
            let state = ensure_state(&mut states, &id, &label, false, false, is_composite);

            if let Some(parent_id) = composite_stack.last().cloned() {
                add_state_child_state(&mut states, &parent_id, &state);
            }

            if is_composite {
                composite_stack.push(id);
            }
            continue;
        }

        // Transition
        if line.contains("-->") || line.contains("->") {
            let arrow = if line.contains("-->") { "-->" } else { "->" };
            if let Some(pos) = line.find(arrow) {
                let from = line[..pos].trim().to_string();
                let rest = &line[pos + arrow.len()..];

                let (to, label) = if rest.contains(':') {
                    let colon_pos = rest.find(':').ok_or_else(|| {
                        format!("line {}: expected ':' separator in transition", line_num)
                    })?;
                    (
                        rest[..colon_pos].trim().to_string(),
                        Some(rest[colon_pos + 1..].trim().to_string()),
                    )
                } else {
                    (rest.trim().to_string(), None)
                };

                let scope = composite_stack.last().map(String::as_str).unwrap_or("");
                let normalized_from = if from == "[*]" {
                    format!("{START_STATE_ID}{scope}")
                } else {
                    from.clone()
                };
                let normalized_to = if to == "[*]" {
                    format!("{END_STATE_ID}{scope}")
                } else {
                    to.clone()
                };

                // Add end state if needed
                if to == "[*]" {
                    ensure_state(&mut states, &normalized_to, "[*]", false, true, false);
                }

                // Add states from transition if not already present
                let from_state = if from != "[*]" {
                    Some(ensure_state(
                        &mut states,
                        &normalized_from,
                        &normalized_from,
                        false,
                        false,
                        false,
                    ))
                } else {
                    Some(ensure_state(
                        &mut states,
                        &normalized_from,
                        "[*]",
                        true,
                        false,
                        false,
                    ))
                };
                let to_state = if to != "[*]" {
                    Some(ensure_state(
                        &mut states,
                        &normalized_to,
                        &normalized_to,
                        false,
                        false,
                        false,
                    ))
                } else {
                    Some(ensure_state(
                        &mut states,
                        &normalized_to,
                        "[*]",
                        false,
                        true,
                        false,
                    ))
                };

                let transition = StateTransition {
                    from: normalized_from,
                    to: normalized_to,
                    label,
                };

                if let Some(parent_id) = composite_stack.last().cloned() {
                    if let Some(state) = from_state.as_ref() {
                        add_state_child_state(&mut states, &parent_id, state);
                    }
                    if let Some(state) = to_state.as_ref() {
                        add_state_child_state(&mut states, &parent_id, state);
                    }
                    add_state_child_transition(&mut states, &parent_id, &transition);
                }

                if composite_stack.is_empty() {
                    transitions.push(transition);
                }
            }
        }
    }

    // Resolve child references only after every definition and note is complete.
    let state_index: HashMap<String, State> = states
        .iter()
        .map(|state| (state.id.clone(), state.clone()))
        .collect();
    fn resolve(
        state: &mut State,
        index: &HashMap<String, State>,
        ancestors: &mut Vec<String>,
    ) -> Result<(), String> {
        if ancestors.contains(&state.id) {
            return Err(format!(
                "Recursive state containment is not supported: {}",
                state.id
            ));
        }
        ancestors.push(state.id.clone());
        for child in &mut state.children {
            if let StateElement::State(child) = child {
                *child = index[&child.id].clone();
                resolve(child, index, ancestors)?;
            }
        }
        ancestors.pop();
        Ok(())
    }
    for state in &mut states {
        resolve(state, &state_index, &mut Vec::new())?;
    }
    let child_ids: std::collections::HashSet<&str> = states
        .iter()
        .flat_map(|s| &s.children)
        .filter_map(|e| match e {
            StateElement::State(s) => Some(s.id.as_str()),
            _ => None,
        })
        .collect();
    if transitions
        .iter()
        .any(|t| child_ids.contains(t.from.as_str()) || child_ids.contains(t.to.as_str()))
    {
        return Err("Cross-boundary state transitions are not supported; connect the composite state instead of its child".to_string());
    }

    Ok(StateDiagram {
        states,
        transitions,
    })
}

fn parse_state_definition(rest: &str) -> Result<(String, String, bool), String> {
    let mut value = rest.trim();
    let mut is_composite = false;

    if value.ends_with('{') {
        is_composite = true;
        value = value[..value.len() - 1].trim();
    }

    if value.is_empty() {
        return Err("Missing state definition".to_string());
    }

    if let Some((left, right)) = value.split_once(" as ") {
        let id = right.trim();
        if id.is_empty() {
            return Err("Missing state alias identifier".to_string());
        }
        let label = left.trim().trim_matches('"');
        return Ok((id.to_string(), label.to_string(), is_composite));
    }

    if value.contains('"') {
        let label_start = value
            .find('"')
            .ok_or("Missing opening quote in state label")?;
        let label_end = value[label_start + 1..]
            .find('"')
            .ok_or("Missing closing quote in state label")?
            + label_start
            + 1;
        let id = value[..label_start].trim();
        if id.is_empty() {
            return Err("Missing state identifier".to_string());
        }
        let label = value[label_start + 1..label_end].to_string();
        return Ok((id.to_string(), label, is_composite));
    }

    Ok((value.to_string(), value.to_string(), is_composite))
}

fn parse_state_note(line: &str) -> Option<(String, String, String)> {
    let lower = line.to_ascii_lowercase();
    let prefixes = ["note right of ", "note left of ", "note over "];

    for prefix in prefixes {
        if lower.starts_with(prefix) {
            let after = line[prefix.len()..].trim();
            let colon_pos = after.find(':')?;
            let state_id = after[..colon_pos].trim();
            let text = after[colon_pos + 1..].trim();
            if state_id.is_empty() {
                return None;
            }
            return Some((
                state_id.to_string(),
                text.to_string(),
                prefix[5..].trim().trim_end_matches(" of").to_string(),
            ));
        }
    }

    None
}

fn ensure_state(
    states: &mut Vec<State>,
    id: &str,
    label: &str,
    is_start: bool,
    is_end: bool,
    is_composite: bool,
) -> State {
    if let Some(state) = states.iter_mut().find(|state| state.id == id) {
        if !label.is_empty() && (label != id || state.label == id) {
            state.label = label.to_string();
        }
        state.is_start |= is_start;
        state.is_end |= is_end;
        state.is_composite |= is_composite;
        return state.clone();
    }

    let state = State {
        id: id.to_string(),
        label: if label.is_empty() {
            id.to_string()
        } else {
            label.to_string()
        },
        is_start,
        is_end,
        is_composite,
        children: Vec::new(),
    };
    states.push(state.clone());
    state
}

fn add_state_child_state(states: &mut [State], parent_id: &str, child: &State) {
    if parent_id == child.id {
        return;
    }

    if let Some(parent) = states.iter_mut().find(|state| state.id == parent_id) {
        parent.is_composite = true;
        let has_child = parent
            .children
            .iter()
            .any(|element| matches!(element, StateElement::State(state) if state.id == child.id));
        if !has_child {
            parent.children.push(StateElement::State(child.clone()));
        }
    }
}

fn add_state_child_transition(states: &mut [State], parent_id: &str, transition: &StateTransition) {
    if let Some(parent) = states.iter_mut().find(|state| state.id == parent_id) {
        parent.is_composite = true;
        let has_transition = parent.children.iter().any(|element| {
            matches!(
                element,
                StateElement::Transition(existing)
                    if existing.from == transition.from
                        && existing.to == transition.to
                        && existing.label == transition.label
            )
        });
        if !has_transition {
            parent
                .children
                .push(StateElement::Transition(transition.clone()));
        }
    }
}

fn add_state_note(states: &mut Vec<State>, state_id: &str, text: String, position: String) {
    let state = ensure_state(states, state_id, state_id, false, false, false);
    if let Some(target_state) = states.iter_mut().find(|s| s.id == state.id) {
        let has_note = target_state.children.iter().any(|element| {
            matches!(
                element,
                StateElement::Note {
                    state,
                    text: existing_text,
                    ..
                } if state == state_id && existing_text == &text
            )
        });
        if !has_note {
            target_state.children.push(StateElement::Note {
                state: state_id.to_string(),
                text: text.clone(),
                position: position.clone(),
            });
        }
    }
}

// ============================================
// ER DIAGRAM PARSER
// ============================================

fn parse_er(input: &str) -> Result<ErDiagram, String> {
    let mut lines = input.lines().skip(1); // Skip "erDiagram"

    let mut entities: Vec<ErEntity> = Vec::new();
    let mut relationships: Vec<ErRelationship> = Vec::new();
    let mut current_entity: Option<String> = None;
    let mut current_attributes: Vec<ErAttribute> = Vec::new();

    for raw_line in &mut lines {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with("%%") {
            continue;
        }

        // With leading comment lines, `.skip(1)` skips the comment instead of the
        // type declaration, so the `erDiagram` line would otherwise become a phantom
        // entity.
        if trimmed.eq_ignore_ascii_case("erDiagram") || trimmed.eq_ignore_ascii_case("er") {
            continue;
        }

        // Inside an entity block
        if current_entity.is_some() {
            if trimmed == "}" {
                let entity_name = current_entity.take().unwrap();
                entities.push(ErEntity {
                    name: entity_name,
                    attributes: std::mem::take(&mut current_attributes),
                });
                continue;
            }

            let is_indented = raw_line
                .chars()
                .next()
                .map(|c| c.is_whitespace())
                .unwrap_or(false);

            if is_indented {
                let attr_line = trimmed;
                let is_key = attr_line.starts_with('*');
                let name = if is_key {
                    attr_line[1..].trim()
                } else {
                    attr_line
                };

                current_attributes.push(ErAttribute {
                    name: name.to_string(),
                    is_key,
                    is_composite: false,
                });
                continue;
            }

            // Unexpected unindented line while inside a block: close the block and
            // re-process this line as a top-level statement.
            let entity_name = current_entity.take().unwrap();
            entities.push(ErEntity {
                name: entity_name,
                attributes: std::mem::take(&mut current_attributes),
            });
            // fallthrough
        }

        if trimmed == "}" {
            continue;
        }

        // Relationship
        if let Some(rel) = parse_er_relationship(trimmed) {
            relationships.push(rel);
            continue;
        }

        // Entity definition (block)
        if trimmed.contains('{') {
            let name = trimmed.trim_end_matches('{').trim().to_string();
            current_entity = Some(name);
            current_attributes.clear();
            continue;
        }

        // Simple entity declaration
        entities.push(ErEntity {
            name: trimmed.to_string(),
            attributes: Vec::new(),
        });
    }

    // Save last entity
    if let Some(entity_name) = current_entity {
        entities.push(ErEntity {
            name: entity_name,
            attributes: current_attributes,
        });
    }

    // Auto-create entities referenced in relationships but not explicitly declared
    let mut existing: std::collections::HashSet<&str> =
        entities.iter().map(|e| e.name.as_str()).collect();
    let mut new_entities = Vec::new();
    for rel in &relationships {
        for name in [&rel.from, &rel.to] {
            if !existing.contains(name.as_str()) {
                existing.insert(name.as_str());
                new_entities.push(ErEntity {
                    name: name.clone(),
                    attributes: Vec::new(),
                });
            }
        }
    }
    entities.extend(new_entities);

    Ok(ErDiagram {
        entities,
        relationships,
    })
}

fn parse_er_relationship(line: &str) -> Option<ErRelationship> {
    let patterns = [
        (
            "||--o|",
            ErCardinality::ExactlyOne,
            ErCardinality::ZeroOrOne,
        ),
        (
            "||..o|",
            ErCardinality::ExactlyOne,
            ErCardinality::ZeroOrOne,
        ),
        ("|o--o|", ErCardinality::ZeroOrOne, ErCardinality::ZeroOrOne),
        ("|o..o|", ErCardinality::ZeroOrOne, ErCardinality::ZeroOrOne),
        (
            "}o--o|",
            ErCardinality::ZeroOrMore,
            ErCardinality::ZeroOrOne,
        ),
        (
            "}o..o|",
            ErCardinality::ZeroOrMore,
            ErCardinality::ZeroOrOne,
        ),
        ("}|--o|", ErCardinality::OneOrMore, ErCardinality::ZeroOrOne),
        ("}|..o|", ErCardinality::OneOrMore, ErCardinality::ZeroOrOne),
        // Solid line patterns (--)
        (
            "||--||",
            ErCardinality::ExactlyOne,
            ErCardinality::ExactlyOne,
        ),
        (
            "||--o{",
            ErCardinality::ExactlyOne,
            ErCardinality::ZeroOrMore,
        ),
        (
            "||--|{",
            ErCardinality::ExactlyOne,
            ErCardinality::OneOrMore,
        ),
        (
            "}o--o{",
            ErCardinality::ZeroOrMore,
            ErCardinality::ZeroOrMore,
        ),
        (
            "}o--||",
            ErCardinality::ZeroOrMore,
            ErCardinality::ExactlyOne,
        ),
        (
            "}o--|{",
            ErCardinality::ZeroOrMore,
            ErCardinality::OneOrMore,
        ),
        (
            "|o--o{",
            ErCardinality::ZeroOrOne,
            ErCardinality::ZeroOrMore,
        ),
        (
            "|o--||",
            ErCardinality::ZeroOrOne,
            ErCardinality::ExactlyOne,
        ),
        ("|o--|{", ErCardinality::ZeroOrOne, ErCardinality::OneOrMore),
        // Dotted line patterns (..)
        (
            "||..||",
            ErCardinality::ExactlyOne,
            ErCardinality::ExactlyOne,
        ),
        (
            "||..o{",
            ErCardinality::ExactlyOne,
            ErCardinality::ZeroOrMore,
        ),
        (
            "||..|{",
            ErCardinality::ExactlyOne,
            ErCardinality::OneOrMore,
        ),
        (
            "}o..o{",
            ErCardinality::ZeroOrMore,
            ErCardinality::ZeroOrMore,
        ),
        (
            "}o..||",
            ErCardinality::ZeroOrMore,
            ErCardinality::ExactlyOne,
        ),
        (
            "}o..|{",
            ErCardinality::ZeroOrMore,
            ErCardinality::OneOrMore,
        ),
        (
            "|o..o{",
            ErCardinality::ZeroOrOne,
            ErCardinality::ZeroOrMore,
        ),
        (
            "|o..||",
            ErCardinality::ZeroOrOne,
            ErCardinality::ExactlyOne,
        ),
        ("|o..|{", ErCardinality::ZeroOrOne, ErCardinality::OneOrMore),
        // }| patterns (OneOrMore from-side) - solid
        (
            "}|--||",
            ErCardinality::OneOrMore,
            ErCardinality::ExactlyOne,
        ),
        (
            "}|--o{",
            ErCardinality::OneOrMore,
            ErCardinality::ZeroOrMore,
        ),
        ("}|--|{", ErCardinality::OneOrMore, ErCardinality::OneOrMore),
        // }| patterns (OneOrMore from-side) - dotted
        (
            "}|..||",
            ErCardinality::OneOrMore,
            ErCardinality::ExactlyOne,
        ),
        (
            "}|..o{",
            ErCardinality::OneOrMore,
            ErCardinality::ZeroOrMore,
        ),
        ("}|..|{", ErCardinality::OneOrMore, ErCardinality::OneOrMore),
    ];

    for (pattern, from_card, to_card) in &patterns {
        if let Some(pos) = line.find(pattern) {
            let from = line[..pos].trim().to_string();
            let rest = line[pos + pattern.len()..].trim();

            let (to, label) = if let Some((to_part, label_part)) = rest.split_once(':') {
                let to = to_part.trim().to_string();
                let label = label_part.trim().trim_matches('"').trim().to_string();
                let label = if label.is_empty() { None } else { Some(label) };
                (to, label)
            } else if let Some(stripped) = rest.strip_prefix('"') {
                let end = stripped.find('"')?;
                (stripped[..end].to_string(), None)
            } else {
                (rest.trim().to_string(), None)
            };

            return Some(ErRelationship {
                from,
                to,
                from_cardinality: from_card.clone(),
                to_cardinality: to_card.clone(),
                dotted: pattern.contains(".."),
                label,
            });
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_unknown_diagram_type_errors() {
        let result = parse_mermaid("%% comment\n\npie title Pets\n  \"Dogs\" : 50");
        let err = result.expect_err("unknown diagram type should error");
        assert!(
            err.contains("Unsupported Mermaid diagram type 'pie'"),
            "error should name the type, got: {err}"
        );
        assert!(
            err.contains("flowchart"),
            "error should list supported types, got: {err}"
        );
    }

    #[test]
    fn test_parse_leading_comments_do_not_break_type_detection() {
        // `%%{init: ...}` config blocks are commonly the first line of a diagram.
        let flowchart = parse_mermaid("%%{init: {'theme': 'base'}}\nflowchart TD\n    A --> B")
            .expect("flowchart with leading comment should parse");
        if let MermaidDiagram::Flowchart(fc) = flowchart {
            assert_eq!(fc.nodes.len(), 2);
        } else {
            panic!("Expected flowchart");
        }

        let er = parse_mermaid("%% comment\nerDiagram\n    CUSTOMER")
            .expect("er with leading comment should parse");
        if let MermaidDiagram::ErDiagram(er_diagram) = er {
            assert!(
                !er_diagram.entities.iter().any(|e| e.name == "erDiagram"),
                "the erDiagram type line must not become a phantom entity"
            );
            assert_eq!(er_diagram.entities.len(), 1);
        } else {
            panic!("Expected er diagram");
        }
    }

    #[test]
    fn test_parse_flowchart_direction_with_leading_comments() {
        for header in ["flowchart", "graph"] {
            for (direction, expected) in [
                ("LR", FlowDirection::LeftRight),
                ("RL", FlowDirection::RightLeft),
                ("BT", FlowDirection::BottomUp),
                ("TB", FlowDirection::TopDown),
                ("TD", FlowDirection::TopDown),
            ] {
                for prefix in [
                    "",
                    "%% comment\n\n",
                    "%%{init: {'theme': 'base'}}\n%% TB\n\n",
                ] {
                    let input = format!("{prefix}{header} {direction}\n    A[Start] --> B[Finish]");
                    let MermaidDiagram::Flowchart(fc) = parse_mermaid(&input).unwrap() else {
                        panic!("Expected flowchart");
                    };
                    assert_eq!(fc.direction, expected, "input: {input}");
                    assert_eq!(
                        fc.nodes
                            .iter()
                            .map(|node| node.id.as_str())
                            .collect::<Vec<_>>(),
                        ["A", "B"],
                        "header must not become a node: {input}"
                    );
                    assert_eq!(fc.edges.len(), 1);
                    assert_eq!(fc.edges[0].from, "A");
                    assert_eq!(fc.edges[0].to, "B");
                }
            }
        }
    }

    #[test]
    fn test_parse_flowchart_rejects_nested_subgraphs() {
        let input = "flowchart LR\nsubgraph Outer\nA[Before]\nsubgraph Inner\nB[Inside]\nend\nC[After]\nend";
        let error =
            parse_mermaid(input).expect_err("nested grouping must not silently lose membership");
        assert!(error.contains("Flowchart parse error:"), "{error}");
        assert!(
            error.contains("Nested subgraphs are not supported"),
            "{error}"
        );
        assert!(error.contains("Inner"), "{error}");
        assert!(error.contains("Outer"), "{error}");
    }

    #[test]
    fn test_parse_flowchart_flat_subgraphs_preserve_membership() {
        let input = "%% comment\nflowchart LR\nsubgraph first[\"First group\"]\nA[Start]\nA --> B[Middle]\nend\nsubgraph Second\nC[Finish]\nend\nB --> C";
        let MermaidDiagram::Flowchart(fc) = parse_mermaid(input).unwrap() else {
            panic!("Expected flowchart");
        };
        assert_eq!(fc.direction, FlowDirection::LeftRight);
        assert_eq!(fc.nodes.len(), 3);
        assert_eq!(fc.edges.len(), 2);
        assert_eq!(fc.subgraphs.len(), 2);
        assert_eq!(fc.subgraphs[0].title, "First group");
        assert_eq!(fc.subgraphs[0].nodes, ["A", "B"]);
        assert_eq!(fc.subgraphs[1].title, "Second");
        assert_eq!(fc.subgraphs[1].nodes, ["C"]);
        assert_ne!(fc.subgraphs[0].id, fc.subgraphs[1].id);
    }

    #[test]
    fn test_parse_empty_input_is_ok() {
        for input in ["", "\n", "%% just a comment"] {
            let result = parse_mermaid(input);
            assert!(
                result.is_ok(),
                "empty/comment-only input should not fail, got: {:?}",
                result.err()
            );
            assert!(matches!(result.unwrap(), MermaidDiagram::Flowchart(_)));
        }
    }

    #[test]
    fn test_parse_simple_flowchart() {
        let input = r#"
flowchart TD
    A --> B
"#;
        let result = parse_mermaid(input).unwrap();
        if let MermaidDiagram::Flowchart(fc) = result {
            assert_eq!(fc.nodes.len(), 2);
            assert_eq!(fc.edges.len(), 1);
            assert_eq!(fc.nodes[0].id, "A");
            assert_eq!(fc.nodes[0].label, "A");
            assert_eq!(fc.nodes[1].id, "B");
            assert_eq!(fc.edges[0].from, "A");
            assert_eq!(fc.edges[0].to, "B");
        } else {
            panic!("Expected flowchart");
        }
    }

    #[test]
    fn test_parse_flowchart_with_labels() {
        let input = r#"
flowchart TD
    A[Start] --> B{Decision?}
    B -->|Yes| C[Continue]
    B -->|No| D[Stop]
"#;
        let result = parse_mermaid(input).unwrap();
        if let MermaidDiagram::Flowchart(fc) = result {
            assert_eq!(fc.nodes.len(), 4);

            // Check node A
            let node_a = fc.nodes.iter().find(|n| n.id == "A").unwrap();
            assert_eq!(node_a.label, "Start", "Node A should have label 'Start'");
            assert_eq!(node_a.shape, NodeShape::Rect);

            // Check node B (decision diamond)
            let node_b = fc.nodes.iter().find(|n| n.id == "B").unwrap();
            assert_eq!(
                node_b.label, "Decision?",
                "Node B should have label 'Decision?'"
            );
            assert_eq!(node_b.shape, NodeShape::Rhombus);

            // Check node C
            let node_c = fc.nodes.iter().find(|n| n.id == "C").unwrap();
            assert_eq!(
                node_c.label, "Continue",
                "Node C should have label 'Continue'"
            );

            // Check node D
            let node_d = fc.nodes.iter().find(|n| n.id == "D").unwrap();
            assert_eq!(node_d.label, "Stop", "Node D should have label 'Stop'");

            // Check edge labels
            let edge_bc = fc
                .edges
                .iter()
                .find(|e| e.from == "B" && e.to == "C")
                .unwrap();
            assert_eq!(edge_bc.label, Some("Yes".to_string()));

            let edge_bd = fc
                .edges
                .iter()
                .find(|e| e.from == "B" && e.to == "D")
                .unwrap();
            assert_eq!(edge_bd.label, Some("No".to_string()));
        } else {
            panic!("Expected flowchart");
        }
    }

    #[test]
    fn test_parse_flowchart_shapes() {
        let input = r#"
flowchart LR
    A([Stadium]) --> B[[Subroutine]]
    B --> C[(Database)]
    C --> D((Circle))
    D --> E{Diamond}
"#;
        let result = parse_mermaid(input).unwrap();
        if let MermaidDiagram::Flowchart(fc) = result {
            assert_eq!(fc.nodes.len(), 5);

            let node_a = fc.nodes.iter().find(|n| n.id == "A").unwrap();
            assert_eq!(node_a.label, "Stadium");
            assert_eq!(node_a.shape, NodeShape::Stadium);

            let node_b = fc.nodes.iter().find(|n| n.id == "B").unwrap();
            assert_eq!(node_b.label, "Subroutine");
            assert_eq!(node_b.shape, NodeShape::Subroutine);

            let node_c = fc.nodes.iter().find(|n| n.id == "C").unwrap();
            assert_eq!(node_c.label, "Database");
            assert_eq!(node_c.shape, NodeShape::Cylinder);

            let node_d = fc.nodes.iter().find(|n| n.id == "D").unwrap();
            assert_eq!(node_d.label, "Circle");
            assert_eq!(node_d.shape, NodeShape::Circle);

            let node_e = fc.nodes.iter().find(|n| n.id == "E").unwrap();
            assert_eq!(node_e.label, "Diamond");
            assert_eq!(node_e.shape, NodeShape::Rhombus);
        } else {
            panic!("Expected flowchart");
        }
    }

    #[test]
    fn test_extract_node_info() {
        // Rect
        let (id, label, shape) = extract_node_info("A[Start]").unwrap();
        assert_eq!(id, "A");
        assert_eq!(label, "Start");
        assert_eq!(shape, NodeShape::Rect);

        // Rhombus
        let (id, label, shape) = extract_node_info("B{Decision?}").unwrap();
        assert_eq!(id, "B");
        assert_eq!(label, "Decision?");
        assert_eq!(shape, NodeShape::Rhombus);

        // Circle
        let (id, label, shape) = extract_node_info("C((Circle))").unwrap();
        assert_eq!(id, "C");
        assert_eq!(label, "Circle");
        assert_eq!(shape, NodeShape::Circle);

        // Simple node
        let (id, label, shape) = extract_node_info("D").unwrap();
        assert_eq!(id, "D");
        assert_eq!(label, "D");
        assert_eq!(shape, NodeShape::RoundedRect);
    }

    #[test]
    fn test_extract_node_info_normalizes_quoted_labels_and_breaks() {
        let (id, label, shape) = extract_node_info(r#"A["Build<br/>Render"]"#).unwrap();
        assert_eq!(id, "A");
        assert_eq!(label, "Build\nRender");
        assert_eq!(shape, NodeShape::Rect);
    }

    #[test]
    fn test_parse_flowchart_normalizes_quoted_labels() {
        let input = r#"
flowchart TD
    A["Start Here"] --> B["Ship<br>Now"]
"#;
        let result = parse_mermaid(input).unwrap();

        if let MermaidDiagram::Flowchart(fc) = result {
            let node_a = fc.nodes.iter().find(|n| n.id == "A").unwrap();
            let node_b = fc.nodes.iter().find(|n| n.id == "B").unwrap();
            assert_eq!(node_a.label, "Start Here");
            assert_eq!(node_b.label, "Ship\nNow");
        } else {
            panic!("Expected flowchart");
        }
    }

    #[test]
    fn test_parse_sequence_diagram() {
        let input = r#"
sequenceDiagram
    participant Alice
    participant Bob
    Alice->>Bob: Hello!
    Bob-->>Alice: Hi!
"#;
        let result = parse_mermaid(input).unwrap();
        if let MermaidDiagram::Sequence(seq) = result {
            assert_eq!(seq.participants.len(), 2);
            assert_eq!(seq.participants[0].id, "Alice");
            assert_eq!(seq.participants[1].id, "Bob");

            // Check messages exist
            let msg_count = seq
                .elements
                .iter()
                .filter(|e| matches!(e, SequenceElement::Message(_)))
                .count();
            assert_eq!(msg_count, 2);
        } else {
            panic!("Expected sequence diagram");
        }
    }

    #[test]
    fn implicit_participants_and_inline_activations_preserve_event_order() {
        let MermaidDiagram::Sequence(sequence) = parse_mermaid("sequenceDiagram\nZ->>+A: request\nparticipant A as Alpha\nA->>+A: nested\nA-->>-A: inner reply\nA-->>-Z: reply\nparticipant Z as Zed\nparticipant M as Em").unwrap() else { panic!("Expected sequence"); };
        assert_eq!(
            sequence
                .participants
                .iter()
                .map(|p| (p.id.as_str(), p.alias.as_deref()))
                .collect::<Vec<_>>(),
            vec![("Z", Some("Zed")), ("A", Some("Alpha")), ("M", Some("Em"))]
        );
        let events = sequence
            .elements
            .iter()
            .map(|event| match event {
                SequenceElement::Message(message) => {
                    ("message", message.from.as_str(), message.to.as_str())
                }
                SequenceElement::Activation(activation) => {
                    ("activate", activation.participant.as_str(), "")
                }
                SequenceElement::Deactivation(activation) => {
                    ("deactivate", activation.participant.as_str(), "")
                }
                _ => panic!("Unexpected sequence event"),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            events,
            vec![
                ("message", "Z", "A"),
                ("activate", "A", ""),
                ("message", "A", "A"),
                ("activate", "A", ""),
                ("message", "A", "A"),
                ("deactivate", "A", ""),
                ("message", "A", "Z"),
                ("deactivate", "A", "")
            ]
        );
        assert!(
            parse_mermaid("sequenceDiagram\nA-->>-Z: inactive")
                .unwrap_err()
                .contains("Cannot deactivate inactive participant A")
        );
    }

    #[test]
    fn test_parse_sequence_messages() {
        let input = r#"
sequenceDiagram
    participant Alice
    participant Bob
    Alice->>Bob: Hello
    Bob-->>Alice: Hi there
"#;
        let result = parse_mermaid(input).unwrap();
        if let MermaidDiagram::Sequence(seq) = result {
            assert_eq!(seq.participants.len(), 2, "Should have 2 participants");

            // Check messages are parsed
            let msg_count = seq
                .elements
                .iter()
                .filter(|e| matches!(e, SequenceElement::Message(_)))
                .count();
            assert_eq!(msg_count, 2, "Should have 2 messages, got {}", msg_count);

            // Check first message
            let first_msg = seq.elements.iter().find_map(|e| {
                if let SequenceElement::Message(m) = e {
                    Some(m)
                } else {
                    None
                }
            });
            assert!(first_msg.is_some(), "Should have at least one message");
            let msg = first_msg.unwrap();
            assert_eq!(msg.from, "Alice", "From should be Alice, got {}", msg.from);
            assert_eq!(msg.to, "Bob", "To should be Bob, got {}", msg.to);
            assert_eq!(
                msg.label, "Hello",
                "Label should be Hello, got '{}'",
                msg.label
            );
        } else {
            panic!("Expected sequence diagram");
        }
    }

    #[test]
    fn test_parse_sequence_with_aliases() {
        let input = r#"
sequenceDiagram
    participant U as User
    participant S as Server
    U->>S: Request
"#;
        let result = parse_mermaid(input).unwrap();
        if let MermaidDiagram::Sequence(seq) = result {
            assert_eq!(seq.participants.len(), 2);

            let user = &seq.participants[0];
            assert_eq!(user.id, "U");
            assert_eq!(user.alias, Some("User".to_string()));

            let server = &seq.participants[1];
            assert_eq!(server.id, "S");
            assert_eq!(server.alias, Some("Server".to_string()));
        } else {
            panic!("Expected sequence diagram");
        }
    }

    #[test]
    fn test_parse_sequence_notes_and_blocks() {
        let input = r#"
sequenceDiagram
    participant Alice
    participant Bob
    Note right of Alice: Start here
    alt success path
        Alice->>Bob: do work
    else retry
        Bob-->>Alice: try again
    end
"#;

        let result = parse_mermaid(input).unwrap();
        if let MermaidDiagram::Sequence(seq) = result {
            assert!(seq
                .elements
                .iter()
                .any(|e| matches!(e, SequenceElement::Note { position, participant, text } if position == "right" && participant == "Alice" && text == "Start here")));

            let block = seq.elements.iter().find_map(|e| {
                if let SequenceElement::Block(block) = e {
                    Some(block)
                } else {
                    None
                }
            });
            assert!(block.is_some());
            let block = block.unwrap();
            assert_eq!(block.messages.len(), 1);
            assert_eq!(block.else_branches.len(), 1);
            assert_eq!(block.else_branches[0].0, "retry");
            assert_eq!(block.else_branches[0].1.len(), 1);
        } else {
            panic!("Expected sequence diagram");
        }
    }

    #[test]
    fn test_parse_flowchart_bidirectional_arrows_precedence() {
        let input = r#"
flowchart TD
    A <==> B
    C <--> D
    E <.-> F
"#;

        let result = parse_mermaid(input).unwrap();
        if let MermaidDiagram::Flowchart(fc) = result {
            assert_eq!(fc.edges.len(), 3);

            let a_b = fc
                .edges
                .iter()
                .find(|e| e.from == "A" && e.to == "B")
                .unwrap();
            assert_eq!(a_b.style, EdgeStyle::Thick);
            assert_eq!(a_b.arrow_head, ArrowType::Arrow);
            assert_eq!(a_b.arrow_tail, ArrowType::Arrow);

            let c_d = fc
                .edges
                .iter()
                .find(|e| e.from == "C" && e.to == "D")
                .unwrap();
            assert_eq!(c_d.style, EdgeStyle::Solid);
            assert_eq!(c_d.arrow_head, ArrowType::Arrow);
            assert_eq!(c_d.arrow_tail, ArrowType::Arrow);

            let e_f = fc
                .edges
                .iter()
                .find(|e| e.from == "E" && e.to == "F")
                .unwrap();
            assert_eq!(e_f.style, EdgeStyle::Dotted);
            assert_eq!(e_f.arrow_head, ArrowType::Arrow);
            assert_eq!(e_f.arrow_tail, ArrowType::Arrow);
        } else {
            panic!("Expected flowchart");
        }
    }

    #[test]
    fn test_parse_class_diagram() {
        let input = r#"
classDiagram
    class Animal {
        +String name
        +int age
        +makeSound()
    }
"#;
        let result = parse_mermaid(input).unwrap();
        if let MermaidDiagram::ClassDiagram(cls) = result {
            assert!(!cls.classes.is_empty(), "Should have at least one class");
            let animal = cls.classes.iter().find(|c| c.name == "Animal");
            assert!(animal.is_some(), "Should find Animal class");
            let animal = animal.unwrap();
            assert!(
                animal.attributes.len() >= 2,
                "Animal should have at least 2 attributes, got {}",
                animal.attributes.len()
            );
            assert!(
                !animal.methods.is_empty(),
                "Animal should have at least 1 method, got {}",
                animal.methods.len()
            );
            // Attribute name includes type in current parsing
            assert!(
                animal
                    .attributes
                    .iter()
                    .any(|a| a.member.name.contains("name")),
                "Should have name attribute"
            );
            assert!(
                animal
                    .methods
                    .iter()
                    .any(|m| m.member.name.contains("makeSound")),
                "Should have makeSound method"
            );
        } else {
            panic!("Expected class diagram");
        }
    }

    #[test]
    fn test_parse_class_relations_with_labels() {
        let input = r#"
classDiagram
    class User {
        +String id
    }
    class Session {
        +String token
    }
    class AuditLog {
        +record(event: String): void
    }

    User --> Session : creates
    User ..> AuditLog : writes
"#;
        let result = parse_mermaid(input).unwrap();
        if let MermaidDiagram::ClassDiagram(cls) = result {
            assert_eq!(cls.classes.len(), 3);
            assert_eq!(cls.relations.len(), 2);

            let creates = cls
                .relations
                .iter()
                .find(|r| r.from == "User" && r.to == "Session")
                .expect("missing creates relation");
            assert_eq!(creates.label.as_deref(), Some("creates"));

            let writes = cls
                .relations
                .iter()
                .find(|r| r.from == "User" && r.to == "AuditLog")
                .expect("missing writes relation");
            assert_eq!(writes.label.as_deref(), Some("writes"));
            assert!(matches!(
                writes.relation_type,
                ClassRelationType::Dependency
            ));
        } else {
            panic!("Expected class diagram");
        }
    }

    #[test]
    fn test_parse_er_entity_blocks_and_relationship_labels() {
        let input = r#"
erDiagram
    USER ||--o{ ORDER : places
    ORDER ||--|{ ORDER_ITEM : contains

    USER {
        string id
        string email
    }

    ORDER {
        string id
    }

    ORDER_ITEM {
        string order_id
    }
"#;
        let result = parse_mermaid(input).unwrap();
        if let MermaidDiagram::ErDiagram(er) = result {
            assert_eq!(er.entities.len(), 3);
            assert_eq!(er.relationships.len(), 2);

            let user = er
                .entities
                .iter()
                .find(|e| e.name == "USER")
                .expect("missing USER");
            assert_eq!(user.attributes.len(), 2);
            assert_eq!(user.attributes[0].name, "string id");
            assert_eq!(user.attributes[1].name, "string email");

            let places = er
                .relationships
                .iter()
                .find(|r| r.from == "USER" && r.to == "ORDER")
                .expect("missing places relationship");
            assert_eq!(places.label.as_deref(), Some("places"));
        } else {
            panic!("Expected ER diagram");
        }
    }

    #[test]
    fn test_parse_state_diagram() {
        let input = r#"
stateDiagram
    [*] --> Idle
    Idle --> Processing
    Processing --> [*]
"#;
        let result = parse_mermaid(input).unwrap();
        if let MermaidDiagram::StateDiagram(st) = result {
            assert!(!st.states.is_empty());
            assert!(!st.transitions.is_empty());

            // Check start state exists
            let has_start = st.states.iter().any(|s| s.is_start);
            assert!(has_start);

            // Check transitions
            let idle_to_processing = st
                .transitions
                .iter()
                .any(|t| t.from == "Idle" && t.to == "Processing");
            assert!(idle_to_processing);
        } else {
            panic!("Expected state diagram");
        }
    }

    #[test]
    fn test_parse_state_composite_with_note_children() {
        let input = r#"
stateDiagram
    state Parent {
        state Child
        Child --> Child: loop
    }
    Note right of Child: child note
"#;
        let result = parse_mermaid(input).unwrap();
        if let MermaidDiagram::StateDiagram(st) = result {
            let parent = st.states.iter().find(|s| s.id == "Parent").unwrap();
            assert!(parent.is_composite);
            assert!(
                parent
                    .children
                    .iter()
                    .any(|e| matches!(e, StateElement::State(s) if s.id == "Child"))
            );
            assert!(parent.children.iter().any(
                |e| matches!(e, StateElement::Transition(t) if t.from == "Child" && t.to == "Child")
            ));

            let child = st.states.iter().find(|s| s.id == "Child").unwrap();
            assert!(child.children.iter().any(|e| {
                matches!(
                    e,
                    StateElement::Note {
                        state,
                        text,
                        ..
                    } if state == "Child" && text == "child note"
                )
            }));
        } else {
            panic!("Expected state diagram");
        }
    }

    #[test]
    fn test_flowchart_cycle() {
        let input = r#"
flowchart TD
    A --> B
    B --> C
    C --> A
"#;
        let result = parse_mermaid(input).unwrap();
        if let MermaidDiagram::Flowchart(fc) = result {
            assert_eq!(fc.nodes.len(), 3);
            assert_eq!(fc.edges.len(), 3);
        } else {
            panic!("Expected flowchart");
        }
    }

    #[test]
    fn test_parse_flowchart_cross_and_circle_arrows() {
        let input = r#"
flowchart TD
    A --x B
    C x-- D
    E --o F
    G o-- H
    I x--x J
    K o--o L
"#;

        let result = parse_mermaid(input).unwrap();
        if let MermaidDiagram::Flowchart(fc) = result {
            assert_eq!(fc.edges.len(), 6);

            let a_b = fc
                .edges
                .iter()
                .find(|e| e.from == "A" && e.to == "B")
                .unwrap();
            assert_eq!(a_b.arrow_head, ArrowType::Cross);
            assert_eq!(a_b.arrow_tail, ArrowType::None);

            let c_d = fc
                .edges
                .iter()
                .find(|e| e.from == "C" && e.to == "D")
                .unwrap();
            assert_eq!(c_d.arrow_head, ArrowType::None);
            assert_eq!(c_d.arrow_tail, ArrowType::Cross);

            let e_f = fc
                .edges
                .iter()
                .find(|e| e.from == "E" && e.to == "F")
                .unwrap();
            assert_eq!(e_f.arrow_head, ArrowType::Circle);
            assert_eq!(e_f.arrow_tail, ArrowType::None);

            let g_h = fc
                .edges
                .iter()
                .find(|e| e.from == "G" && e.to == "H")
                .unwrap();
            assert_eq!(g_h.arrow_head, ArrowType::None);
            assert_eq!(g_h.arrow_tail, ArrowType::Circle);

            let i_j = fc
                .edges
                .iter()
                .find(|e| e.from == "I" && e.to == "J")
                .unwrap();
            assert_eq!(i_j.arrow_head, ArrowType::Cross);
            assert_eq!(i_j.arrow_tail, ArrowType::Cross);

            let k_l = fc
                .edges
                .iter()
                .find(|e| e.from == "K" && e.to == "L")
                .unwrap();
            assert_eq!(k_l.arrow_head, ArrowType::Circle);
            assert_eq!(k_l.arrow_tail, ArrowType::Circle);
        } else {
            panic!("Expected flowchart");
        }
    }

    #[test]
    fn test_parse_flowchart_dotted_cross_and_circle_arrows() {
        let input = r#"
flowchart TD
    A -.x B
    C x-. D
    E -.o F
    G o-. H
"#;

        let result = parse_mermaid(input).unwrap();
        if let MermaidDiagram::Flowchart(fc) = result {
            assert_eq!(fc.edges.len(), 4);

            let a_b = fc
                .edges
                .iter()
                .find(|e| e.from == "A" && e.to == "B")
                .unwrap();
            assert_eq!(a_b.style, EdgeStyle::Dotted);
            assert_eq!(a_b.arrow_head, ArrowType::Cross);
            assert_eq!(a_b.arrow_tail, ArrowType::None);

            let c_d = fc
                .edges
                .iter()
                .find(|e| e.from == "C" && e.to == "D")
                .unwrap();
            assert_eq!(c_d.style, EdgeStyle::Dotted);
            assert_eq!(c_d.arrow_head, ArrowType::None);
            assert_eq!(c_d.arrow_tail, ArrowType::Cross);

            let e_f = fc
                .edges
                .iter()
                .find(|e| e.from == "E" && e.to == "F")
                .unwrap();
            assert_eq!(e_f.style, EdgeStyle::Dotted);
            assert_eq!(e_f.arrow_head, ArrowType::Circle);
            assert_eq!(e_f.arrow_tail, ArrowType::None);

            let g_h = fc
                .edges
                .iter()
                .find(|e| e.from == "G" && e.to == "H")
                .unwrap();
            assert_eq!(g_h.style, EdgeStyle::Dotted);
            assert_eq!(g_h.arrow_head, ArrowType::None);
            assert_eq!(g_h.arrow_tail, ArrowType::Circle);
        } else {
            panic!("Expected flowchart");
        }
    }

    #[test]
    fn test_parse_flowchart_open_arrow_variant() {
        let input = r#"
flowchart TD
    A --o> B
"#;

        let result = parse_mermaid(input).unwrap();
        if let MermaidDiagram::Flowchart(fc) = result {
            assert_eq!(fc.edges.len(), 1);
            let edge = &fc.edges[0];
            assert_eq!(edge.from, "A");
            assert_eq!(edge.to, "B");
            assert_eq!(edge.arrow_head, ArrowType::Arrow);
            assert_eq!(edge.arrow_tail, ArrowType::Circle);
        } else {
            panic!("Expected flowchart");
        }
    }
}

#[cfg(test)]
mod error_tests {
    use super::*;

    #[test]
    fn test_parse_class_missing_stereotype_end() {
        let input = "classDiagram\nclass <<abstract Animal";
        let result = parse_mermaid(input);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("missing '>>' in stereotype"));
    }

    #[test]
    fn test_parse_state_missing_definition() {
        let input = "stateDiagram\nstate {";
        let result = parse_mermaid(input);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing state definition"));
    }

    #[test]
    fn test_parse_state_missing_closing_quote() {
        let input = "stateDiagram\nstate \"Label";
        let result = parse_mermaid(input);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Missing closing quote in state label")
        );
    }

    #[test]
    fn test_parse_state_missing_identifier() {
        let input = "stateDiagram\nstate \"Label\"";
        let result = parse_mermaid(input);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing state identifier"));
    }

    #[test]
    fn test_unknown_diagram_type_errors() {
        let result = parse_mermaid("unknownDiagram\n  A --> B");
        let err = result.expect_err("Unknown diagram type should error");
        assert!(err.contains("Unsupported Mermaid diagram type 'unknownDiagram'"));
    }
}
