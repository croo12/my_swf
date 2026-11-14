//! Reproducible comparison using the same projected repository graph for both engines.
use super::*;
use serde_json::{Value, json};
use std::{fs, path::Path};

pub fn run(repository: &Path, output: &Path) -> Result<(), String> {
    fs::create_dir_all(output).map_err(|e| e.to_string())?;
    let start = Instant::now();
    let snapshot = factory_core::repository::analyze(repository)?;
    let scan_ms = start.elapsed().as_secs_f64() * 1000.0;
    let mut scenarios = vec![("layers", BTreeSet::new())];
    scenarios.push((
        "slices",
        snapshot
            .tree
            .children
            .iter()
            .map(|node| node.path.clone())
            .collect(),
    ));
    let mut features = BTreeSet::new();
    if snapshot.tree.find("features").is_some() {
        features.insert("features".to_string());
        scenarios.push(("features", features.clone()));
    }
    let target = "features/variants-filter/api/add-preset-api.ts";
    if snapshot.tree.find(target).is_some() {
        repository_graph::reveal(&snapshot.tree, target, &mut features);
        scenarios.push(("nested-api", features));
    }
    let target = "features/variants-filter/lib/snv/index.ts";
    if snapshot.tree.find(target).is_some() {
        let mut expanded = BTreeSet::new();
        repository_graph::reveal(&snapshot.tree, target, &mut expanded);
        scenarios.push(("nested-index", expanded));
    }
    let mut results = vec![];
    for (name, expanded) in scenarios {
        let graph = repository_graph::project(&snapshot, &expanded);
        let started = Instant::now();
        let layout = crate::layout_engine::layout(&graph)?;
        let elk_ms = started.elapsed().as_secs_f64() * 1000.0;
        let elk = Scene::from_elk(graph.clone(), &layout);
        write_json(&output.join(format!("{name}-input.json")), &layout.input)?;
        write_json(&output.join(format!("{name}-elk.json")), &layout.output)?;
        fs::write(output.join(format!("{name}-elk.svg")), svg(&elk)).map_err(|e| e.to_string())?;
        let started = Instant::now();
        let previous = Scene::new(graph.clone());
        let previous_ms = started.elapsed().as_secs_f64() * 1000.0;
        fs::write(output.join(format!("{name}-previous.svg")), svg(&previous))
            .map_err(|e| e.to_string())?;
        results.push(json!({
            "scenario": name, "expanded": expanded,
            "nodes": graph.node_count(), "edges": graph.edges.len(),
            "elk": metrics(&elk, elk_ms), "previous": metrics(&previous, previous_ms)
        }));
        // Persist each completed scenario so an interrupted comparison remains inspectable.
        write_json(
            &output.join("report.json"),
            &json!({
                "repository": snapshot.repository, "source_root": snapshot.source_root,
                "files": snapshot.tree.file_count, "scan_ms": scan_ms,
                "notes": "Crossings count strict interior intersections between segments of distinct edges. Shared length sums collinear overlap per edge pair. Node overlaps exclude ancestor/descendant containers. Timings are a single local run, not a statistical benchmark.",
                "scenarios": results
            }),
        )?;
    }
    Ok(())
}

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    fs::write(
        path,
        serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

fn metrics(scene: &Scene, milliseconds: f64) -> Value {
    let mut overlaps = 0;
    let mut containment_errors = 0;
    for (i, a) in scene.boxes.iter().enumerate() {
        for b in scene.boxes.iter().skip(i + 1) {
            if inside(&a.path, &b.path) || inside(&b.path, &a.path) {
                continue;
            }
            let common = a.rect.intersect(b.rect);
            if common.width() > 0.1 && common.height() > 0.1 {
                overlaps += 1;
            }
        }
        if let Some((parent, _)) = a.path.rsplit_once('/')
            && let Some(parent) = scene.boxes.iter().find(|b| b.path == parent)
            && (!parent.rect.expand(0.1).contains_rect(a.rect)
                || a.rect.top() < parent.header().bottom() - 0.1)
        {
            containment_errors += 1;
        }
    }
    let mut crossings = 0;
    let mut shared_length = 0.0f32;
    let mut body_crossings = 0;
    for (i, a) in scene.edges.iter().enumerate() {
        for segment in a.points.windows(2) {
            for node in &scene.boxes {
                if segment_hits(segment[0], segment[1], node.header().shrink(0.1)) {
                    body_crossings += 1;
                }
            }
        }
        for b in scene.edges.iter().skip(i + 1) {
            for sa in a.points.windows(2) {
                for sb in b.points.windows(2) {
                    let av = (sa[0].x - sa[1].x).abs() < 0.01;
                    let bv = (sb[0].x - sb[1].x).abs() < 0.01;
                    if av == bv {
                        let (fixed_a, fixed_b, a0, a1, b0, b1) = if av {
                            (sa[0].x, sb[0].x, sa[0].y, sa[1].y, sb[0].y, sb[1].y)
                        } else {
                            (sa[0].y, sb[0].y, sa[0].x, sa[1].x, sb[0].x, sb[1].x)
                        };
                        if (fixed_a - fixed_b).abs() < 0.01 {
                            shared_length +=
                                (a0.max(a1).min(b0.max(b1)) - a0.min(a1).max(b0.min(b1))).max(0.0);
                        }
                    } else {
                        let (v, h) = if av { (sa, sb) } else { (sb, sa) };
                        if v[0].x > h[0].x.min(h[1].x) + 0.01
                            && v[0].x < h[0].x.max(h[1].x) - 0.01
                            && h[0].y > v[0].y.min(v[1].y) + 0.01
                            && h[0].y < v[0].y.max(v[1].y) - 0.01
                        {
                            crossings += 1;
                        }
                    }
                }
            }
        }
    }
    json!({"milliseconds":milliseconds,"width":scene.size.x,"height":scene.size.y,
        "node_overlaps":overlaps,"containment_errors":containment_errors,
        "edge_body_intersections":body_crossings,"proper_segment_crossings":crossings,
        "shared_segment_length":shared_length,"routed_edges":scene.edges.len()})
}

fn segment_hits(a: Pos2, b: Pos2, rect: Rect) -> bool {
    if (a.x - b.x).abs() < 0.01 {
        a.x > rect.left()
            && a.x < rect.right()
            && a.y.max(b.y) > rect.top()
            && a.y.min(b.y) < rect.bottom()
    } else {
        a.y > rect.top()
            && a.y < rect.bottom()
            && a.x.max(b.x) > rect.left()
            && a.x.min(b.x) < rect.right()
    }
}

fn svg(scene: &Scene) -> String {
    use std::fmt::Write;
    fn escape(value: &str) -> String {
        value
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    }
    let mut out = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}"><rect width="100%" height="100%" fill="#12131b"/><g font-family="sans-serif" font-size="15">"##,
        scene.size.x, scene.size.y, scene.size.x, scene.size.y
    );
    for node in &scene.boxes {
        let color = canvas::layer_color(&node.path);
        let _ = write!(
            out,
            r##"<rect x="{}" y="{}" width="{}" height="{}" rx="10" fill="{}" stroke="#{:02x}{:02x}{:02x}" stroke-opacity=".7"/>"##,
            node.rect.left(),
            node.rect.top(),
            node.rect.width(),
            node.rect.height(),
            if node.expanded { "#191b28" } else { "#242636" },
            color.r(),
            color.g(),
            color.b()
        );
    }
    for route in &scene.edges {
        let points = route
            .points
            .iter()
            .map(|p| format!("{},{}", p.x, p.y))
            .collect::<Vec<_>>()
            .join(" ");
        let _ = write!(
            out,
            r##"<polyline points="{points}" fill="none" stroke="#12131b" stroke-width="5"/><polyline points="{points}" fill="none" stroke="#a995ec" stroke-width="1.5"><title>{} → {}</title></polyline>"##,
            escape(&route.edge.from),
            escape(&route.edge.to)
        );
    }
    for node in &scene.boxes {
        let _ = write!(
            out,
            r##"<text x="{}" y="{}" fill="#e8e6f0">{}</text>"##,
            node.rect.left() + 14.0,
            node.rect.top() + 30.0,
            escape(&node.name)
        );
    }
    out.push_str("</g></svg>");
    out
}
