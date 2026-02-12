use std::collections::{HashMap, HashSet, VecDeque};

use super::types::*;

/// Bounding box for layout elements
#[derive(Debug, Clone, Copy, Default)]
pub struct BBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl BBox {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    pub fn center_x(&self) -> f32 {
        self.x + self.width / 2.0
    }

    pub fn center_y(&self) -> f32 {
        self.y + self.height / 2.0
    }

    pub fn with_padding(&self, padding: f32) -> Self {
        Self::new(
            self.x - padding,
            self.y - padding,
            self.width + padding * 2.0,
            self.height + padding * 2.0,
        )
    }
}

/// Layout position for a node
#[derive(Debug, Clone, Copy)]
pub struct LayoutPos {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl LayoutPos {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn center(&self) -> (f32, f32) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }
}

/// Layout engine for diagrams
pub struct LayoutEngine {
    pub node_spacing_x: f32,
    pub node_spacing_y: f32,
    pub edge_label_padding: f32,
    pub node_padding: f32,
}

impl Default for LayoutEngine {
    fn default() -> Self {
        Self {
            node_spacing_x: 60.0,
            node_spacing_y: 50.0,
            edge_label_padding: 8.0,
            node_padding: 12.0,
        }
    }
}

impl LayoutEngine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Layout a flowchart diagram using hierarchical layout
    pub fn layout_flowchart(&self, flowchart: &Flowchart) -> (HashMap<String, LayoutPos>, BBox) {
        let mut positions: HashMap<String, LayoutPos> = HashMap::new();

        if flowchart.nodes.is_empty() {
            return (positions, BBox::default());
        }

        // Build adjacency lists
        let mut outgoing: HashMap<&str, Vec<(&str, usize)>> = HashMap::new();
        let mut incoming: HashMap<&str, Vec<&str>> = HashMap::new();

        for node in &flowchart.nodes {
            outgoing.entry(node.id.as_str()).or_default();
            incoming.entry(node.id.as_str()).or_default();
        }

        for edge in &flowchart.edges {
            outgoing
                .entry(edge.from.as_str())
                .or_default()
                .push((&edge.to, edge.min_length.max(1)));
            incoming
                .entry(edge.to.as_str())
                .or_default()
                .push(&edge.from);
        }

        // Find root nodes (no incoming edges)
        let mut roots: Vec<&str> = flowchart
            .nodes
            .iter()
            .filter(|n| incoming.get(n.id.as_str()).map_or(true, |v| v.is_empty()))
            .map(|n| n.id.as_str())
            .collect();

        if roots.is_empty() {
            roots.push(flowchart.nodes[0].id.as_str());
        }

        // Assign levels using BFS. Cycles are supported by assigning each node once.
        let mut levels: HashMap<&str, usize> = HashMap::new();
        let mut max_level = 0;
        let mut queue: VecDeque<&str> = VecDeque::new();

        for root in &roots {
            if levels.insert(root, 0).is_none() {
                queue.push_back(root);
            }
        }

        while let Some(node_id) = queue.pop_front() {
            let current_level = *levels.get(node_id).unwrap_or(&0);
            if let Some(neighbors) = outgoing.get(node_id) {
                for &(neighbor, min_len) in neighbors {
                    if !levels.contains_key(neighbor) {
                        let neighbor_level = current_level + min_len;
                        levels.insert(neighbor, neighbor_level);
                        max_level = max_level.max(neighbor_level);
                        queue.push_back(neighbor);
                    }
                }
            }
        }

        // Assign any unassigned nodes (disconnected or cyclic subgraphs without roots)
        for node in &flowchart.nodes {
            let node_id = node.id.as_str();
            if levels.contains_key(node_id) {
                continue;
            }

            max_level += 1;
            levels.insert(node_id, max_level);
            queue.push_back(node_id);

            while let Some(node_id) = queue.pop_front() {
                let current_level = *levels.get(node_id).unwrap_or(&0);
                if let Some(neighbors) = outgoing.get(node_id) {
                    for &(neighbor, min_len) in neighbors {
                        if !levels.contains_key(neighbor) {
                            let neighbor_level = current_level + min_len;
                            levels.insert(neighbor, neighbor_level);
                            max_level = max_level.max(neighbor_level);
                            queue.push_back(neighbor);
                        }
                    }
                }
            }
        }

        // Group nodes by level
        let mut level_nodes: Vec<Vec<&str>> = vec![Vec::new(); max_level + 1];
        for (&node_id, &level) in &levels {
            level_nodes[level].push(node_id);
        }

        // Calculate node sizes based on label
        let node_sizes: HashMap<&str, (f32, f32)> = flowchart
            .nodes
            .iter()
            .map(|n| {
                let (w, h) = self.calculate_node_size(&n.label, &n.shape);
                (n.id.as_str(), (w, h))
            })
            .collect();

        // Position nodes
        let is_vertical = matches!(
            flowchart.direction,
            FlowDirection::TopDown | FlowDirection::BottomUp
        );

        if is_vertical {
            let mut y = 20.0;
            for level in &level_nodes {
                let mut x = 20.0;
                let mut max_height: f32 = 0.0;

                for &node_id in level {
                    let (w, h) = node_sizes.get(node_id).copied().unwrap_or((80.0, 40.0));
                    positions.insert(node_id.to_string(), LayoutPos::new(x, y, w, h));
                    x += w + self.node_spacing_x;
                    max_height = max_height.max(h);
                }

                y += max_height + self.node_spacing_y;
            }
        } else {
            let mut x = 20.0;
            for level in &level_nodes {
                let mut y = 20.0;
                let mut max_width: f32 = 0.0;

                for &node_id in level {
                    let (w, h) = node_sizes.get(node_id).copied().unwrap_or((80.0, 40.0));
                    positions.insert(node_id.to_string(), LayoutPos::new(x, y, w, h));
                    y += h + self.node_spacing_y;
                    max_width = max_width.max(w);
                }

                x += max_width + self.node_spacing_x;
            }
        }

        // Calculate bounding box
        let bbox = self.calculate_bbox(&positions);

        (positions, bbox)
    }

    fn calculate_node_size(&self, label: &str, shape: &NodeShape) -> (f32, f32) {
        let char_width = 8.0;
        let line_height = 16.0;
        let padding = self.node_padding * 2.0;

        let lines: Vec<&str> = label.lines().collect();
        let max_line_chars = lines.iter().map(|l| l.len()).max().unwrap_or(1);
        let num_lines = lines.len().max(1);

        let text_width = (max_line_chars as f32 * char_width).max(20.0);
        let text_height = (num_lines as f32 * line_height).max(line_height);

        let (base_width, base_height) = match shape {
            NodeShape::Circle => {
                let size = text_width.max(text_height) + padding;
                (size, size)
            }
            NodeShape::DoubleCircle => {
                let size = text_width.max(text_height) + padding + 8.0;
                (size, size)
            }
            NodeShape::Rhombus => {
                let size = text_width.max(text_height) + padding + 20.0;
                (size, size)
            }
            NodeShape::Hexagon => (text_width + padding + 30.0, text_height + padding + 10.0),
            NodeShape::Stadium | NodeShape::Subroutine => {
                (text_width + padding + 20.0, text_height + padding + 8.0)
            }
            NodeShape::Cylinder => (text_width + padding, text_height + padding + 15.0),
            NodeShape::Parallelogram | NodeShape::ParallelogramAlt => {
                (text_width + padding + 30.0, text_height + padding)
            }
            NodeShape::Trapezoid | NodeShape::TrapezoidAlt => {
                (text_width + padding + 30.0, text_height + padding)
            }
            _ => (text_width + padding, text_height + padding),
        };

        (base_width.max(40.0), base_height.max(30.0))
    }

    fn calculate_bbox(&self, positions: &HashMap<String, LayoutPos>) -> BBox {
        if positions.is_empty() {
            return BBox::new(0.0, 0.0, 100.0, 100.0);
        }

        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_right = f32::MIN;
        let mut max_bottom = f32::MIN;

        for pos in positions.values() {
            let node_bbox = BBox::new(pos.x, pos.y, pos.width, pos.height);
            min_x = min_x.min(node_bbox.x);
            min_y = min_y.min(node_bbox.y);
            max_right = max_right.max(node_bbox.right());
            max_bottom = max_bottom.max(node_bbox.bottom());
        }

        BBox::new(min_x, min_y, max_right - min_x, max_bottom - min_y)
    }

    /// Layout a sequence diagram
    pub fn layout_sequence(
        &self,
        diagram: &SequenceDiagram,
    ) -> (HashMap<String, LayoutPos>, Vec<SequenceLayoutElement>, BBox) {
        let mut positions: HashMap<String, LayoutPos> = HashMap::new();
        let mut elements: Vec<SequenceLayoutElement> = Vec::new();

        if diagram.participants.is_empty() {
            return (positions, elements, BBox::default());
        }

        // Position participants horizontally
        let participant_width = 120.0;
        let participant_height = 40.0;
        let spacing = 80.0;
        let start_x = 40.0;
        let start_y = 20.0;

        for (i, participant) in diagram.participants.iter().enumerate() {
            let x = start_x + i as f32 * (participant_width + spacing);
            positions.insert(
                participant.id.clone(),
                LayoutPos::new(x, start_y, participant_width, participant_height),
            );
        }

        let mut current_y = start_y + participant_height + 40.0;
        self.collect_sequence_layout_elements(
            &diagram.elements,
            &positions,
            &mut current_y,
            &mut elements,
        );

        if elements.is_empty() {
            current_y += 40.0;
        }

        let max_label_half_width = elements
            .iter()
            .filter_map(|el| {
                if let SequenceLayoutElement::Message { label, .. } = el {
                    Some(label.chars().count() as f32 * 3.5 + self.edge_label_padding)
                } else {
                    None
                }
            })
            .fold(0.0_f32, f32::max);

        let width = start_x * 2.0
            + diagram.participants.len() as f32 * (participant_width + spacing)
            - spacing;
        let bbox = BBox::new(
            0.0,
            0.0,
            width + max_label_half_width * 2.0,
            current_y + 20.0,
        )
        .with_padding(self.edge_label_padding / 2.0);

        (positions, elements, bbox)
    }

    fn collect_sequence_layout_elements(
        &self,
        elements: &[SequenceElement],
        positions: &HashMap<String, LayoutPos>,
        current_y: &mut f32,
        out: &mut Vec<SequenceLayoutElement>,
    ) {
        for element in elements {
            match element {
                SequenceElement::Message(msg) => {
                    if let (Some(from), Some(to)) =
                        (positions.get(&msg.from), positions.get(&msg.to))
                    {
                        let (from_x, _) = from.center();
                        let (to_x, _) = to.center();
                        out.push(SequenceLayoutElement::Message {
                            from_x,
                            to_x,
                            y: *current_y,
                            label: msg.label.clone(),
                        });
                    }
                    *current_y += 50.0;
                }
                SequenceElement::Activation(activation)
                | SequenceElement::Deactivation(activation) => {
                    if let Some(pos) = positions.get(&activation.participant) {
                        let (cx, _) = pos.center();
                        out.push(SequenceLayoutElement::Activation {
                            x: cx,
                            y: *current_y - 10.0,
                            height: 30.0,
                        });
                    }
                    *current_y += 24.0;
                }
                SequenceElement::Note { .. } => {
                    *current_y += 42.0;
                }
                SequenceElement::Block(block) => {
                    *current_y += 28.0;
                    self.collect_sequence_layout_elements(
                        &block.messages,
                        positions,
                        current_y,
                        out,
                    );
                    for (_, branch_elements) in &block.else_branches {
                        *current_y += 22.0;
                        self.collect_sequence_layout_elements(
                            branch_elements,
                            positions,
                            current_y,
                            out,
                        );
                    }
                    *current_y += 20.0;
                }
            }
        }
    }

    /// Layout a class diagram
    pub fn layout_class(&self, diagram: &ClassDiagram) -> (HashMap<String, LayoutPos>, BBox) {
        let mut positions: HashMap<String, LayoutPos> = HashMap::new();

        if diagram.classes.is_empty() {
            return (positions, BBox::default());
        }

        let class_width = 180.0;
        let spacing_x = 100.0;
        let spacing_y = 80.0;
        let start_x = 40.0;
        let start_y = 40.0;

        // Simple grid layout
        let cols = (diagram.classes.len() as f32).sqrt().ceil() as usize;
        let cols = cols.max(1);

        for (i, class) in diagram.classes.iter().enumerate() {
            let col = i % cols;
            let row = i / cols;

            // Calculate height based on attributes and methods
            let num_attrs = class.attributes.len();
            let num_methods = class.methods.len();
            let header_height = 30.0;
            let attr_height = 20.0 * num_attrs as f32;
            let method_height = 20.0 * num_methods as f32;
            let class_height = header_height + attr_height + method_height + 20.0;

            let x = start_x + col as f32 * (class_width + spacing_x);
            let y = start_y + row as f32 * (class_height + spacing_y);

            positions.insert(
                class.name.clone(),
                LayoutPos::new(x, y, class_width, class_height),
            );
        }

        let bbox = self.calculate_bbox(&positions);
        (positions, bbox)
    }

    /// Layout a state diagram
    pub fn layout_state(&self, diagram: &StateDiagram) -> (HashMap<String, LayoutPos>, BBox) {
        let mut positions: HashMap<String, LayoutPos> = HashMap::new();

        if diagram.states.is_empty() {
            return (positions, BBox::default());
        }

        // Build transition graph
        let mut outgoing: HashMap<&str, Vec<&str>> = HashMap::new();
        for state in &diagram.states {
            outgoing.entry(state.id.as_str()).or_default();
        }
        for trans in &diagram.transitions {
            outgoing
                .entry(trans.from.as_str())
                .or_default()
                .push(&trans.to);
        }

        // Assign levels using BFS from start state
        let mut levels: HashMap<&str, usize> = HashMap::new();
        let mut queue: VecDeque<&str> = VecDeque::new();
        if let Some(start_state) = diagram.states.iter().find(|state| state.is_start) {
            levels.insert(start_state.id.as_str(), 0);
            queue.push_back(start_state.id.as_str());
        }

        if queue.is_empty() {
            if let Some(state) = diagram.states.first() {
                levels.insert(state.id.as_str(), 0);
                queue.push_back(state.id.as_str());
            }
        }

        while let Some(node) = queue.pop_front() {
            let current_level = *levels.get(node).unwrap_or(&0);
            if let Some(neighbors) = outgoing.get(node) {
                for &neighbor in neighbors {
                    let neighbor_level = levels.entry(neighbor).or_insert(usize::MAX);
                    if current_level + 1 < *neighbor_level {
                        *neighbor_level = current_level + 1;
                        queue.push_back(neighbor);
                    }
                }
            }
        }

        // Assign levels to any unassigned states
        let mut max_level = levels.values().copied().max().unwrap_or(0);
        for state in &diagram.states {
            if !levels.contains_key(state.id.as_str()) {
                max_level += 1;
                levels.insert(state.id.as_str(), max_level);
            }
        }

        // Group by level
        let mut level_states: Vec<Vec<&str>> = vec![Vec::new(); max_level + 1];
        for (&id, &level) in &levels {
            if level <= max_level {
                level_states[level].push(id);
            }
        }

        // Position states
        let state_width = 120.0;
        let state_height = 40.0;
        let spacing_x = 60.0;
        let spacing_y = 50.0;
        let start_x = 40.0;
        let start_y = 40.0;

        let special_states: HashSet<&str> = diagram
            .states
            .iter()
            .filter(|state| state.is_start || state.is_end)
            .map(|state| state.id.as_str())
            .collect();

        for (level, states) in level_states.iter().enumerate() {
            let y = start_y + level as f32 * (state_height + spacing_y);
            let _total_width = states.len() as f32 * state_width
                + (states.len().saturating_sub(1)) as f32 * spacing_x;
            let mut x = start_x;

            for &state_id in states {
                let (w, h) = if special_states.contains(state_id) {
                    (24.0, 24.0)
                } else {
                    (state_width, state_height)
                };
                positions.insert(state_id.to_string(), LayoutPos::new(x, y, w, h));
                x += w + spacing_x;
            }
        }

        let bbox = self.calculate_bbox(&positions);
        (positions, bbox)
    }

    /// Layout an ER diagram
    pub fn layout_er(&self, diagram: &ErDiagram) -> (HashMap<String, LayoutPos>, BBox) {
        let mut positions: HashMap<String, LayoutPos> = HashMap::new();

        if diagram.entities.is_empty() {
            return (positions, BBox::default());
        }

        let entity_width = 140.0;
        let spacing_x = 120.0;
        let spacing_y = 100.0;
        let start_x = 40.0;
        let start_y = 40.0;

        // Grid layout
        let cols = (diagram.entities.len() as f32).sqrt().ceil() as usize;
        let cols = cols.max(1);

        for (i, entity) in diagram.entities.iter().enumerate() {
            let col = i % cols;
            let row = i / cols;

            // Calculate height based on attributes
            let header_height = 30.0;
            let attr_height = 18.0 * entity.attributes.len() as f32;
            let entity_height = header_height + attr_height + 10.0;

            let x = start_x + col as f32 * (entity_width + spacing_x);
            let y = start_y + row as f32 * (entity_height + spacing_y);

            positions.insert(
                entity.name.clone(),
                LayoutPos::new(x, y, entity_width, entity_height),
            );
        }

        let bbox = self.calculate_bbox(&positions);
        (positions, bbox)
    }
}

/// Layout element for sequence diagrams
#[derive(Debug, Clone)]
pub enum SequenceLayoutElement {
    Message {
        from_x: f32,
        to_x: f32,
        y: f32,
        label: String,
    },
    Activation {
        x: f32,
        y: f32,
        height: f32,
    },
}
