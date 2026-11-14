//! ELK adapter. Both shapes and edge sections use root coordinates.
use eframe::egui::{Pos2, Rect, Vec2, pos2};
use factory_core::repository_graph::{Graph, Node};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub struct Layout {
    pub nodes: BTreeMap<String, Rect>,
    pub edges: BTreeMap<String, Vec<Pos2>>,
    pub size: Vec2,
    pub input: Value,
    pub output: Value,
}

pub fn layout(graph: &Graph) -> Result<Layout, String> {
    fn node(item: &Node, partition: Option<usize>) -> Value {
        let mut options = json!({
            "elk.padding": "[top=86,left=36,bottom=36,right=36]",
            "elk.spacing.nodeNode": "48",
            "elk.layered.spacing.nodeNodeBetweenLayers": "90"
        });
        if let Some(index) = partition {
            options["elk.partitioning.partition"] = json!(index.to_string());
        }
        let mut value = json!({
            "id": item.path,
            "width": 234,
            "height": 108,
            "layoutOptions": options,
        });
        if !item.children.is_empty() {
            value["children"] = item
                .children
                .iter()
                .map(|child| node(child, None))
                .collect();
        }
        value
    }
    let input = json!({
        "id": "@repository",
        "layoutOptions": {
            "elk.algorithm": "layered",
            "elk.direction": "RIGHT",
            "elk.hierarchyHandling": "INCLUDE_CHILDREN",
            "elk.edgeRouting": "ORTHOGONAL",
            "elk.partitioning.activate": "true",
            "elk.spacing.nodeNode": "48",
            "elk.spacing.edgeNode": "24",
            "elk.spacing.edgeEdge": "14",
            "elk.layered.spacing.nodeNodeBetweenLayers": "110",
            "elk.layered.spacing.edgeNodeBetweenLayers": "28",
            "elk.layered.spacing.edgeEdgeBetweenLayers": "14",
            "elk.layered.mergeEdges": "false",
            "elk.layered.mergeHierarchyEdges": "false",
            "elk.padding": "[top=32,left=32,bottom=32,right=32]",
            "elk.json.shapeCoords": "ROOT",
            "elk.json.edgeCoords": "ROOT"
        },
        "children": graph.nodes.iter().enumerate().map(|(i, item)| node(item, Some(i))).collect::<Vec<_>>(),
        "edges": graph.edges.iter().enumerate().map(|(i, edge)| json!({
            "id": edge_id(i), "sources": [edge.from], "targets": [edge.to]
        })).collect::<Vec<_>>()
    });
    let output = elk_layout::layout_value(input.clone(), "{}")?;
    decode(graph, input, output)
}

pub fn edge_id(index: usize) -> String {
    format!("@dependency/{index}")
}

fn number(value: &Value, key: &str) -> Result<f32, String> {
    let number = value[key]
        .as_f64()
        .ok_or_else(|| format!("ELK: missing {key} in {}", value["id"]))? as f32;
    if !number.is_finite() {
        return Err(format!("ELK: non-finite {key}"));
    }
    Ok(number)
}

fn point(value: &Value) -> Result<Pos2, String> {
    Ok(pos2(number(value, "x")?, number(value, "y")?))
}

fn decode(graph: &Graph, input: Value, output: Value) -> Result<Layout, String> {
    fn collect(
        value: &Value,
        nodes: &mut BTreeMap<String, Rect>,
        edges: &mut BTreeMap<String, Vec<Pos2>>,
    ) -> Result<(), String> {
        if let Some(children) = value["children"].as_array() {
            for child in children {
                let id = child["id"].as_str().ok_or("ELK: node without id")?;
                let rect = Rect::from_min_size(
                    point(child)?,
                    Vec2::new(number(child, "width")?, number(child, "height")?),
                );
                if rect.width() <= 0.0 || rect.height() <= 0.0 {
                    return Err(format!("ELK: invalid node bounds for {id}"));
                }
                if nodes.insert(id.into(), rect).is_some() {
                    return Err(format!("ELK: duplicate node {id}"));
                }
                collect(child, nodes, edges)?;
            }
        }
        if let Some(items) = value["edges"].as_array() {
            for edge in items {
                let id = edge["id"].as_str().ok_or("ELK: edge without id")?;
                let sections = edge["sections"]
                    .as_array()
                    .ok_or_else(|| format!("ELK: unrouted edge {id}"))?;
                if sections.is_empty() {
                    return Err(format!("ELK: empty route {id}"));
                }
                // Compound edges can consist of several sections. Join the complete chain;
                // accepting only its first section would silently hide dependencies.
                let mut remaining = sections
                    .iter()
                    .map(|section| {
                        let mut points = vec![point(&section["startPoint"])?];
                        if let Some(bends) = section["bendPoints"].as_array() {
                            for bend in bends {
                                points.push(point(bend)?);
                            }
                        }
                        points.push(point(&section["endPoint"])?);
                        Ok(points)
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                let mut chain = remaining.remove(0);
                while !remaining.is_empty() {
                    let last = *chain.last().unwrap();
                    let first = chain[0];
                    if let Some(i) = remaining.iter().position(|p| p[0].distance(last) < 0.01) {
                        chain.extend(remaining.remove(i).into_iter().skip(1));
                    } else if let Some(i) = remaining
                        .iter()
                        .position(|p| p.last().unwrap().distance(first) < 0.01)
                    {
                        let mut prefix = remaining.remove(i);
                        prefix.pop();
                        prefix.extend(chain);
                        chain = prefix;
                    } else {
                        return Err(format!("ELK: disconnected or branching sections for {id}"));
                    }
                }
                if chain
                    .windows(2)
                    .any(|p| (p[0].x - p[1].x).abs() > 0.01 && (p[0].y - p[1].y).abs() > 0.01)
                {
                    return Err(format!("ELK: non-orthogonal route {id}"));
                }
                if edges.insert(id.into(), chain).is_some() {
                    return Err(format!("ELK: duplicate edge {id}"));
                }
            }
        }
        Ok(())
    }
    let mut nodes = BTreeMap::new();
    let mut edges = BTreeMap::new();
    collect(&output, &mut nodes, &mut edges)?;
    fn paths(nodes: &[Node], ids: &mut BTreeSet<String>) {
        for node in nodes {
            ids.insert(node.path.clone());
            paths(&node.children, ids);
        }
    }
    let mut expected = BTreeSet::new();
    paths(&graph.nodes, &mut expected);
    if expected != nodes.keys().cloned().collect()
        || edges.len() != graph.edges.len()
        || (0..graph.edges.len()).any(|i| !edges.contains_key(&edge_id(i)))
    {
        return Err("ELK: input/output graph mismatch".into());
    }
    let size = Vec2::new(number(&output, "width")?, number(&output, "height")?);
    Ok(Layout {
        nodes,
        edges,
        size,
        input,
        output,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejoins_every_compound_section_in_root_coordinates() {
        let graph = Graph {
            nodes: vec![],
            edges: vec![factory_core::repository_graph::Edge {
                from: "a".into(),
                to: "b".into(),
                file_pairs: 1,
                names: vec![],
            }],
        };
        let output = json!({"id":"root", "width":300,"height":200,"edges":[{
            "id":edge_id(0),"sections":[
                {"startPoint":{"x":80,"y":100},"endPoint":{"x":80,"y":160}},
                {"startPoint":{"x":40,"y":100},"endPoint":{"x":80,"y":100}}
            ]
        }]});
        let result = decode(&graph, Value::Null, output).unwrap();
        assert_eq!(
            result.edges[&edge_id(0)],
            vec![pos2(40.0, 100.0), pos2(80.0, 100.0), pos2(80.0, 160.0)]
        );
    }
}
