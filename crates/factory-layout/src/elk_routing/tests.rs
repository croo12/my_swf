use super::*;

#[test]
fn rejects_missing_duplicate_and_non_orthogonal_routes() {
    for output in [
        "LAYOUT\nDONE",
        "LAYOUT\nEDGE 1=0 0 10 0\nEDGE 1=0 0 10 0\nDONE",
        "LAYOUT\nEDGE 1=0 0 10 10\nDONE",
    ] {
        assert!(parse(output, 1).is_err());
    }
}

#[test]
fn dense_fixed_pins_route_without_aborting_or_crossing_card_bodies() {
    let distance = assert_native_routes(include_str!("fixtures/dense-fixed-pins.json"));
    assert!(distance < 1.0);
}

#[test]
fn expanded_group_recovers_from_native_separation_assertions() {
    assert_native_routes(include_str!("fixtures/expanded-group.json"));
}

fn assert_native_routes(fixture: &str) -> f32 {
    // Captured failing geometry, with no repository paths or source content.
    let input: serde_json::Value = serde_json::from_str(fixture).unwrap();
    let number = |value: &serde_json::Value| value.as_f64().unwrap() as f32;
    let obstacles: Vec<_> = input["obstacles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            Rect::from_min_max(
                Pos2::new(number(&r[0]), number(&r[1])),
                Pos2::new(number(&r[2]), number(&r[3])),
            )
        })
        .collect();
    let connections: Vec<_> = input["connections"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c[0].as_u64().unwrap() as usize,
                c[1].as_u64().unwrap() as usize,
            )
        })
        .collect();
    let ports: Vec<_> = input["ports"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                Pos2::new(number(&p[0]), number(&p[1])),
                Pos2::new(number(&p[2]), number(&p[3])),
                Pos2::new(number(&p[4]), number(&p[5])),
                Pos2::new(number(&p[6]), number(&p[7])),
            )
        })
        .collect();
    let distance = spacing::nudging_distance(&connections, &ports);
    let routes = route(&obstacles, &connections, &ports).unwrap();
    assert_eq!(routes.len(), connections.len());
    for points in routes {
        for segment in points.windows(2) {
            let (a, b) = (segment[0], segment[1]);
            for rect in obstacles.iter().map(|r| r.shrink(0.1)) {
                let crosses = if a.x == b.x {
                    a.x > rect.left()
                        && a.x < rect.right()
                        && a.y.max(b.y) > rect.top()
                        && a.y.min(b.y) < rect.bottom()
                } else {
                    a.y > rect.top()
                        && a.y < rect.bottom()
                        && a.x.max(b.x) > rect.left()
                        && a.x.min(b.x) < rect.right()
                };
                assert!(!crosses, "{a:?} -> {b:?} crosses {rect:?}");
            }
        }
    }
    distance
}
