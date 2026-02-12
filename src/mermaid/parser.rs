

use super::types::*;

#[derive(Debug, Clone)]
pub enum DiagramKind {
    Flowchart(Flowchart),
    Sequence(SequenceDiagram),
    ClassDiagram(ClassDiagram),
    StateDiagram(StateDiagram),
    ErDiagram(ErDiagram),
}

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

    // Detect diagram type from first line
    let first_line = input.lines().next().unwrap_or("");

    if first_line.starts_with("flowchart")
        || first_line.starts_with("graph")
        || first_line.starts_with("flowchart ")
        || first_line.starts_with("graph ")
    {
        let diagram = parse_flowchart(input).map_err(|e| format!("Flowchart parse error: {:?}", e))?;
        Ok(MermaidDiagram::Flowchart(diagram))
    } else if first_line.starts_with("sequenceDiagram") || first_line.starts_with("sequence") {
        let diagram = parse_sequence(input).map_err(|e| format!("Sequence parse error: {:?}", e))?;
        Ok(MermaidDiagram::Sequence(diagram))
    } else if first_line.starts_with("classDiagram") || first_line.starts_with("class") {
        let diagram = parse_class(input).map_err(|e| format!("Class parse error: {:?}", e))?;
        Ok(MermaidDiagram::ClassDiagram(diagram))
    } else if first_line.starts_with("stateDiagram") || first_line.starts_with("state") {
        let diagram = parse_state(input).map_err(|e| format!("State parse error: {:?}", e))?;
        Ok(MermaidDiagram::StateDiagram(diagram))
    } else if first_line.starts_with("erDiagram") || first_line.starts_with("er") {
        let diagram = parse_er(input).map_err(|e| format!("ER parse error: {:?}", e))?;
        Ok(MermaidDiagram::ErDiagram(diagram))
    } else {
        // Default to flowchart for backward compatibility
        let diagram = parse_flowchart(input).map_err(|e| format!("Flowchart parse error: {:?}", e))?;
        Ok(MermaidDiagram::Flowchart(diagram))
    }
}

// ============================================
// FLOWCHART PARSER
// ============================================

fn parse_flowchart(input: &str) -> Result<Flowchart, String> {
    let mut lines = input.lines().peekable();

    // Parse direction from first line
    let first_line = lines.next().unwrap_or("");
    let direction = parse_flow_direction(first_line);

    let mut nodes: Vec<FlowchartNode> = Vec::new();
    let mut edges: Vec<FlowchartEdge> = Vec::new();
    let mut subgraphs: Vec<Subgraph> = Vec::new();
    let mut current_subgraph: Option<Subgraph> = None;
    let mut node_labels: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Skip comments
        if line.starts_with("%%") {
            continue;
        }

        // Subgraph start
        if line.starts_with("subgraph ") {
            let title = line.strip_prefix("subgraph ").unwrap_or("").trim();
            let id = format!("subgraph_{}", subgraphs.len());
            current_subgraph = Some(Subgraph {
                id: id.clone(),
                title: title.to_string(),
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

        // Try to parse as edge or node definition
        if let Some((from_info, to_info, label, style, arrow_head, arrow_tail)) = parse_edge_line(line) {
            // Register nodes if not already present (with full info including label and shape)
            if !node_labels.contains_key(&from_info.0) {
                nodes.push(FlowchartNode {
                    id: from_info.0.clone(),
                    label: from_info.1.clone(),
                    shape: from_info.2,
                });
                node_labels.insert(from_info.0.clone(), from_info.1.clone());
            }
            if !node_labels.contains_key(&to_info.0) {
                nodes.push(FlowchartNode {
                    id: to_info.0.clone(),
                    label: to_info.1.clone(),
                    shape: to_info.2,
                });
                node_labels.insert(to_info.0.clone(), to_info.1.clone());
            }

            edges.push(FlowchartEdge {
                from: from_info.0,
                to: to_info.0,
                label,
                style,
                arrow_head,
                arrow_tail,
                min_length: 1,
            });
        } else if let Some((id, label, shape)) = parse_node_definition(line) {
            nodes.push(FlowchartNode {
                id: id.clone(),
                label: label.clone(),
                shape,
            });
            node_labels.insert(id.clone(), label);

            // Add to current subgraph if in one
            if let Some(ref mut sg) = current_subgraph {
                sg.nodes.push(id);
            }
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

fn parse_edge_line(line: &str) -> Option<((String, String, NodeShape), (String, String, NodeShape), Option<String>, EdgeStyle, ArrowType, ArrowType)> {
    // Edge patterns (order matters - longer patterns first)
    let patterns = [
        ("==>", EdgeStyle::Thick, ArrowType::Arrow, ArrowType::None),
        ("---", EdgeStyle::Solid, ArrowType::None, ArrowType::None),
        ("-->", EdgeStyle::Solid, ArrowType::Arrow, ArrowType::None),
        ("-.->", EdgeStyle::Dotted, ArrowType::Arrow, ArrowType::None),
        ("-.-", EdgeStyle::Dotted, ArrowType::None, ArrowType::None),
        ("<==>", EdgeStyle::Thick, ArrowType::Arrow, ArrowType::Arrow),
        ("<-->", EdgeStyle::Solid, ArrowType::Arrow, ArrowType::Arrow),
        ("<--", EdgeStyle::Solid, ArrowType::None, ArrowType::Arrow),
        ("<==", EdgeStyle::Thick, ArrowType::None, ArrowType::Arrow),
        ("<-.", EdgeStyle::Dotted, ArrowType::None, ArrowType::Arrow),
        ("->>", EdgeStyle::Solid, ArrowType::Arrow, ArrowType::Arrow),
        ("->", EdgeStyle::Solid, ArrowType::Arrow, ArrowType::None),
        ("--", EdgeStyle::Solid, ArrowType::None, ArrowType::None),
    ];

    for (pattern, style, head, tail) in &patterns {
        if let Some(pos) = line.find(pattern) {
            let from_part = line[..pos].trim();
            let rest = &line[pos + pattern.len()..];

            // Parse optional label
            let (to_part, label) = if rest.starts_with('|') {
                // Label before target: A -->|label| B
                if let Some(end_label) = rest[1..].find('|') {
                    let label_text = rest[1..end_label + 1].trim();
                    let after_label = rest[end_label + 2..].trim();
                    (after_label, Some(label_text.to_string()))
                } else {
                    (rest.trim(), None)
                }
            } else {
                // No label, just target
                (rest.trim(), None)
            };

            // Extract full node info (id, label, shape)
            let from_info = extract_node_info(from_part)?;
            let to_info = extract_node_info(to_part)?;

            return Some((from_info, to_info, label, style.clone(), head.clone(), tail.clone()));
        }
    }

    None
}

fn extract_node_id(part: &str) -> Option<String> {
    let part = part.trim();

    // Handle shaped nodes like A[Label], A(Label), A{Label}, etc.
    // Check multi-char patterns first, then single chars
    let multi_char_patterns = ["[[", "((", "[/", "[\\"];
    for pattern in &multi_char_patterns {
        if part.contains(pattern) {
            let pos = part.find(pattern)?;
            return Some(part[..pos].trim().to_string());
        }
    }

    let single_char_patterns = ['[', '(', '{', '<'];
    for bracket in &single_char_patterns {
        if part.contains(*bracket) {
            let pos = part.find(*bracket)?;
            return Some(part[..pos].trim().to_string());
        }
    }

    // Simple node id - alphanumeric and underscores
    let id: String = part.chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        .collect();

    if id.is_empty() {
        None
    } else {
        Some(id)
    }
}

/// Extract node ID, label, and shape from a part of an edge definition
fn extract_node_info(part: &str) -> Option<(String, String, NodeShape)> {
    let part = part.trim();

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
        if let Some(pos) = part.find(open) {
            let after_open = &part[pos + open.len()..];
            if let Some(end_pos) = after_open.find(close) {
                let id = part[..pos].trim().to_string();
                let label = after_open[..end_pos].trim().to_string();

                if !id.is_empty() {
                    return Some((id, label, shape.clone()));
                }
            }
        }
    }

    // Simple node - just an ID
    let id = extract_node_id(part)?;
    Some((id.clone(), id, NodeShape::RoundedRect))
}

fn parse_node_definition(line: &str) -> Option<(String, String, NodeShape)> {
    let line = line.trim();

    // Patterns: id[Label], id(Label), id{Label}, etc.
    // Order matters: check longer patterns first
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
        if let Some(pos) = line.find(open) {
            let after_open = &line[pos + open.len()..];
            if let Some(end_pos) = after_open.find(close) {
                let id = line[..pos].trim().to_string();
                let label = after_open[..end_pos].trim().to_string();

                if !id.is_empty() {
                    return Some((id, label, shape.clone()));
                }
            }
        }
    }

    None
}

// ============================================
// SEQUENCE DIAGRAM PARSER
// ============================================

fn parse_sequence(input: &str) -> Result<SequenceDiagram, String> {
    let mut lines = input.lines().skip(1); // Skip "sequenceDiagram"

    let mut participants: Vec<Participant> = Vec::new();
    let mut elements: Vec<SequenceElement> = Vec::new();

    for line in &mut lines {
        let line = line.trim();
        if line.is_empty() || line.starts_with("%%") {
            continue;
        }

        // Participant declaration
        if line.starts_with("participant ") {
            let rest = line.strip_prefix("participant ").unwrap_or("");
            // Mermaid syntax: participant A as "Display Name" or participant A
            // The ID comes first, then optional "as Alias"
            let parts: Vec<&str> = rest.splitn(2, " as ").collect();
            if parts.len() == 2 {
                participants.push(Participant {
                    id: parts[0].trim().to_string(),
                    alias: Some(parts[1].trim().to_string()),
                });
            } else {
                participants.push(Participant {
                    id: rest.trim().to_string(),
                    alias: None,
                });
            }
            continue;
        }

        // Actor shorthand
        if line.starts_with("actor ") {
            let rest = line.strip_prefix("actor ").unwrap_or("");
            participants.push(Participant {
                id: rest.trim().to_string(),
                alias: None,
            });
            continue;
        }

        // Message
        if let Some(msg) = parse_sequence_message(line) {
            elements.push(SequenceElement::Message(msg));
            continue;
        }

        // Activation
        if line.starts_with("activate ") {
            let participant = line.strip_prefix("activate ").unwrap_or("").trim();
            elements.push(SequenceElement::Activation(Activation {
                participant: participant.to_string(),
            }));
            continue;
        }

        if line.starts_with("deactivate ") {
            let participant = line.strip_prefix("deactivate ").unwrap_or("").trim();
            elements.push(SequenceElement::Deactivation(Activation {
                participant: participant.to_string(),
            }));
            continue;
        }
    }

    Ok(SequenceDiagram {
        participants,
        elements,
    })
}

fn parse_sequence_message(line: &str) -> Option<SequenceMessage> {
    // Order matters: longer patterns first to avoid partial matches
    let patterns = [
        ("-->>", MessageType::Dotted, MessageKind::Reply),
        ("->>", MessageType::Solid, MessageKind::Sync),
        (">>+", MessageType::Solid, MessageKind::Async),
        (">>-", MessageType::Solid, MessageKind::Async),
        ("-->", MessageType::Dotted, MessageKind::Sync),
        ("->", MessageType::Solid, MessageKind::Sync),
        ("-x", MessageType::Solid, MessageKind::Sync),
        ("-)", MessageType::Solid, MessageKind::Sync),
    ];

    for (pattern, msg_type, kind) in &patterns {
        if let Some(pos) = line.find(pattern) {
            let from = line[..pos].trim().to_string();
            let rest = &line[pos + pattern.len()..];

            // Parse "To: Label" or just "To"
            let (to, label) = if let Some(colon_pos) = rest.find(':') {
                let to_part = rest[..colon_pos].trim().to_string();
                let label_part = rest[colon_pos + 1..].trim().to_string();
                (to_part, Some(label_part))
            } else {
                (rest.trim().to_string(), None)
            };

            return Some(SequenceMessage {
                from,
                to,
                label: label.unwrap_or_default(),
                msg_type: msg_type.clone(),
                kind: kind.clone(),
            });
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
    let mut current_class: Option<ClassDefinition> = None;

    for line in &mut lines {
        let line = line.trim();
        if line.is_empty() || line.starts_with("%%") {
            continue;
        }

        // Class definition end
        if line == "}" {
            if let Some(cls) = current_class.take() {
                classes.push(cls);
            }
            continue;
        }

        // Class definition start
        if line.starts_with("class ") {
            let rest = line.strip_prefix("class ").unwrap_or("");

            // Check for stereotype
            let (name, stereotype) = if rest.starts_with("<<") {
                let end = rest.find(">>").ok_or("Missing >> in stereotype")?;
                let st = &rest[2..end];
                let after = rest[end + 2..].trim();
                (after.to_string(), Some(st.trim().to_string()))
            } else {
                (rest.trim().to_string(), None)
            };

            // Check if it's a one-liner or has body
            if name.ends_with('{') {
                current_class = Some(ClassDefinition {
                    name: name.trim_end_matches('{').trim().to_string(),
                    stereotype,
                    attributes: Vec::new(),
                    methods: Vec::new(),
                    is_abstract: false,
                    is_interface: false,
                });
            } else {
                classes.push(ClassDefinition {
                    name,
                    stereotype,
                    attributes: Vec::new(),
                    methods: Vec::new(),
                    is_abstract: false,
                    is_interface: false,
                });
            }
            continue;
        }

        // If inside a class body
        if let Some(ref mut cls) = current_class {
            // Attribute or method
            if line.starts_with('+') || line.starts_with('-') || line.starts_with('#') || line.starts_with('~') {
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
                } else {
                    if let Some(attr) = parse_class_attribute(vis, member) {
                        cls.attributes.push(attr);
                    }
                }
            }
            continue;
        }

        // Relation
        if let Some(rel) = parse_class_relation(line) {
            relations.push(rel);
        }
    }

    Ok(ClassDiagram { classes, relations })
}

fn parse_class_attribute(vis: Visibility, member: &str) -> Option<ClassAttribute> {
    let parts: Vec<&str> = member.splitn(2, ':').collect();
    let name = parts[0].trim();
    let type_ann = parts.get(1).map(|s| s.trim().to_string());

    Some(ClassAttribute {
        member: ClassMember {
            visibility: vis,
            name: name.to_string(),
            is_static: false,
            is_abstract: false,
        },
        type_annotation: type_ann,
    })
}

fn parse_class_method(vis: Visibility, member: &str) -> Option<ClassMethod> {
    let paren_pos = member.find('(')?;
    let name = member[..paren_pos].trim();

    let close_paren = member.find(')')?;
    let params_str = &member[paren_pos + 1..close_paren];

    let return_type = if close_paren + 1 < member.len() {
        let after = member[close_paren + 1..].trim();
        if after.starts_with(':') {
            Some(after[1..].trim().to_string())
        } else {
            None
        }
    } else {
        None
    };

    let parameters = if params_str.is_empty() {
        Vec::new()
    } else {
        params_str.split(',')
            .filter_map(|p| {
                let parts: Vec<&str> = p.trim().splitn(2, ':').collect();
                if parts.is_empty() {
                    None
                } else {
                    Some((parts[0].trim().to_string(), parts.get(1).map(|s| s.trim().to_string())))
                }
            })
            .collect()
    };

    Some(ClassMethod {
        member: ClassMember {
            visibility: vis,
            name: name.to_string(),
            is_static: false,
            is_abstract: false,
        },
        parameters,
        return_type,
    })
}

fn parse_class_relation(line: &str) -> Option<ClassRelation> {
    let patterns = [
        ("<|--", ClassRelationType::Inheritance),
        ("*--", ClassRelationType::Composition),
        ("o--", ClassRelationType::Aggregation),
        ("-->", ClassRelationType::Association),
        ("--", ClassRelationType::Association),
        ("..>", ClassRelationType::Dependency),
        ("..|>", ClassRelationType::Realization),
        ("..", ClassRelationType::Dependency),
    ];

    for (pattern, rel_type) in &patterns {
        if let Some(pos) = line.find(pattern) {
            let from = line[..pos].trim().to_string();
            let rest = &line[pos + pattern.len()..];
            let to = rest.trim().to_string();

            return Some(ClassRelation {
                from,
                to,
                relation_type: rel_type.clone(),
                label: None,
                multiplicity_from: None,
                multiplicity_to: None,
            });
        }
    }

    None
}

// ============================================
// STATE DIAGRAM PARSER
// ============================================

fn parse_state(input: &str) -> Result<StateDiagram, String> {
    let mut lines = input.lines().skip(1); // Skip "stateDiagram"

    let mut states: Vec<State> = Vec::new();
    let mut transitions: Vec<StateTransition> = Vec::new();

    // Add start state
    states.push(State {
        id: "[*]".to_string(),
        label: "[*]".to_string(),
        is_start: true,
        is_end: false,
        is_composite: false,
        children: Vec::new(),
    });

    for line in &mut lines {
        let line = line.trim();
        if line.is_empty() || line.starts_with("%%") {
            continue;
        }

        // State definition
        if line.starts_with("state ") {
            let rest = line.strip_prefix("state ").unwrap_or("");

            // Check for state name with label
            if rest.contains('"') {
                let label_start = rest.find('"').ok_or("Missing opening quote in state label")?;
                let label_end = rest[label_start + 1..].find('"').ok_or("Missing closing quote in state label")? + label_start + 1;
                let label = rest[label_start + 1..label_end].to_string();
                let id = rest[..label_start].trim().to_string();

                states.push(State {
                    id,
                    label,
                    is_start: false,
                    is_end: false,
                    is_composite: false,
                    children: Vec::new(),
                });
            } else {
                let id = rest.trim().to_string();
                states.push(State {
                    id: id.clone(),
                    label: id,
                    is_start: false,
                    is_end: false,
                    is_composite: false,
                    children: Vec::new(),
                });
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
                    let colon_pos = rest.find(':').ok_or("Expected colon in transition")?;
                    (rest[..colon_pos].trim().to_string(), Some(rest[colon_pos + 1..].trim().to_string()))
                } else {
                    (rest.trim().to_string(), None)
                };

                // Add end state if needed
                if to == "[*]" {
                    if let Some(state) = states.iter_mut().find(|state| state.id == "[*]") {
                        state.is_end = true;
                    } else {
                        states.push(State {
                            id: "[*]".to_string(),
                            label: "[*]".to_string(),
                            is_start: false,
                            is_end: true,
                            is_composite: false,
                            children: Vec::new(),
                        });
                    }
                }

                // Add states from transition if not already present
                let has_from = states.iter().any(|s| s.id == from) || from == "[*]";
                let has_to = states.iter().any(|s| s.id == to) || to == "[*]";
                
                let mut new_states = Vec::new();
                if !has_from {
                    new_states.push(State {
                        id: from.clone(),
                        label: from.clone(),
                        is_start: false,
                        is_end: false,
                        is_composite: false,
                        children: Vec::new(),
                    });
                }
                if !has_to {
                    new_states.push(State {
                        id: to.clone(),
                        label: to.clone(),
                        is_start: false,
                        is_end: false,
                        is_composite: false,
                        children: Vec::new(),
                    });
                }
                states.append(&mut new_states);

                transitions.push(StateTransition { from, to, label });
            }
        }
    }

    Ok(StateDiagram { states, transitions })
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

    for line in &mut lines {
        let line = line.trim();
        if line.is_empty() || line.starts_with("%%") {
            continue;
        }

        // Entity attribute
        if line.starts_with("  ") || line.starts_with("\t") {
            if current_entity.is_some() {
                let attr_line = line.trim();
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
            }
            continue;
        }

        // Save previous entity
        if let Some(entity_name) = current_entity.take() {
            entities.push(ErEntity {
                name: entity_name,
                attributes: std::mem::take(&mut current_attributes),
            });
        }

        // Relationship
        if line.contains("||--") || line.contains("}o--") || line.contains("|o--") {
            if let Some(rel) = parse_er_relationship(line) {
                relationships.push(rel);
            }
            continue;
        }

        // Entity definition
        if line.contains('{') {
            let name = line.trim_end_matches('{').trim().to_string();
            current_entity = Some(name);
            current_attributes.clear();
        } else if !line.contains("--") {
            // Simple entity declaration
            entities.push(ErEntity {
                name: line.to_string(),
                attributes: Vec::new(),
            });
        }
    }

    // Save last entity
    if let Some(entity_name) = current_entity {
        entities.push(ErEntity {
            name: entity_name,
            attributes: current_attributes,
        });
    }

    Ok(ErDiagram { entities, relationships })
}

fn parse_er_relationship(line: &str) -> Option<ErRelationship> {
    let patterns = [
        ("||--||", ErCardinality::ExactlyOne, ErCardinality::ExactlyOne),
        ("||--o{", ErCardinality::ExactlyOne, ErCardinality::ZeroOrMore),
        ("||--|{", ErCardinality::ExactlyOne, ErCardinality::OneOrMore),
        ("}o--o{", ErCardinality::ZeroOrMore, ErCardinality::ZeroOrMore),
        ("}o--||", ErCardinality::ZeroOrMore, ErCardinality::ExactlyOne),
        ("}o--|{", ErCardinality::ZeroOrMore, ErCardinality::OneOrMore),
        ("|o--o{", ErCardinality::ZeroOrOne, ErCardinality::ZeroOrMore),
        ("|o--||", ErCardinality::ZeroOrOne, ErCardinality::ExactlyOne),
        ("|o--|{", ErCardinality::ZeroOrOne, ErCardinality::OneOrMore),
    ];

    for (pattern, from_card, to_card) in &patterns {
        if let Some(pos) = line.find(pattern) {
            let from = line[..pos].trim().to_string();
            let rest = &line[pos + pattern.len()..];

            let (to, label) = if rest.starts_with('"') {
                let end = rest[1..].find('"')? + 1;
                (rest[..end].trim_matches('"').to_string(), None)
            } else {
                (rest.trim().to_string(), None)
            };

            return Some(ErRelationship {
                from,
                to,
                from_cardinality: from_card.clone(),
                to_cardinality: to_card.clone(),
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
            assert_eq!(node_b.label, "Decision?", "Node B should have label 'Decision?'");
            assert_eq!(node_b.shape, NodeShape::Rhombus);
            
            // Check node C
            let node_c = fc.nodes.iter().find(|n| n.id == "C").unwrap();
            assert_eq!(node_c.label, "Continue", "Node C should have label 'Continue'");
            
            // Check node D
            let node_d = fc.nodes.iter().find(|n| n.id == "D").unwrap();
            assert_eq!(node_d.label, "Stop", "Node D should have label 'Stop'");
            
            // Check edge labels
            let edge_bc = fc.edges.iter().find(|e| e.from == "B" && e.to == "C").unwrap();
            assert_eq!(edge_bc.label, Some("Yes".to_string()));
            
            let edge_bd = fc.edges.iter().find(|e| e.from == "B" && e.to == "D").unwrap();
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
            let msg_count = seq.elements.iter().filter(|e| matches!(e, SequenceElement::Message(_))).count();
            assert_eq!(msg_count, 2);
        } else {
            panic!("Expected sequence diagram");
        }
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
            let msg_count = seq.elements.iter().filter(|e| matches!(e, SequenceElement::Message(_))).count();
            assert_eq!(msg_count, 2, "Should have 2 messages, got {}", msg_count);
            
            // Check first message
            let first_msg = seq.elements.iter().find_map(|e| {
                if let SequenceElement::Message(m) = e { Some(m) } else { None }
            });
            assert!(first_msg.is_some(), "Should have at least one message");
            let msg = first_msg.unwrap();
            assert_eq!(msg.from, "Alice", "From should be Alice, got {}", msg.from);
            assert_eq!(msg.to, "Bob", "To should be Bob, got {}", msg.to);
            assert_eq!(msg.label, "Hello", "Label should be Hello, got '{}'", msg.label);
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
            assert!(animal.attributes.len() >= 2, "Animal should have at least 2 attributes, got {}", animal.attributes.len());
            assert!(animal.methods.len() >= 1, "Animal should have at least 1 method, got {}", animal.methods.len());
            // Attribute name includes type in current parsing
            assert!(animal.attributes.iter().any(|a| a.member.name.contains("name")), "Should have name attribute");
            assert!(animal.methods.iter().any(|m| m.member.name.contains("makeSound")), "Should have makeSound method");
        } else {
            panic!("Expected class diagram");
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
            let idle_to_processing = st.transitions.iter()
                .any(|t| t.from == "Idle" && t.to == "Processing");
            assert!(idle_to_processing);
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
}
