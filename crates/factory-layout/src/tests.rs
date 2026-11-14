use crate::{
    Scene,
    arrangement::FOCUS_GAP,
    focus::{Direction, inside, relation},
};
use factory_repository::projection::{Edge, Graph, Node};
fn node(path: &str, kind: &str, children: Vec<Node>) -> Node {
    Node {
        path: path.into(),
        name: path.rsplit('/').next().unwrap().into(),
        kind: kind.into(),
        index: kind == "group",
        file_count: 1,
        expandable: !children.is_empty(),
        children,
    }
}
#[test]
fn fifty_slices_stay_on_one_row_without_wrapping() {
    let graph = Graph {
        nodes: vec![node(
            "features",
            "layer",
            (0..50)
                .map(|i| node(&format!("features/slice-{i:02}"), "slice", vec![]))
                .collect(),
        )],
        edges: vec![],
    };
    let scene = Scene::focused(graph, "").unwrap();
    let slices: Vec<_> = scene
        .boxes
        .iter()
        .filter(|item| item.kind == "slice")
        .collect();
    assert_eq!(slices.len(), 50);
    assert!(
        slices
            .iter()
            .all(|item| item.rect.top() == slices[0].rect.top())
    );
    assert!(
        slices
            .windows(2)
            .all(|items| items[0].rect.right() < items[1].rect.left())
    );
}
#[test]
fn one_row_focus_docks_unrelated_cards_and_disables_layer_relations() {
    let graph = Graph {
        nodes: vec![
            node(
                "features",
                "layer",
                vec![
                    node("features/history", "slice", vec![]),
                    node("features/cart", "slice", vec![]),
                    node("features/checkout", "slice", vec![]),
                ],
            ),
            node("shared", "layer", vec![]),
        ],
        edges: [
            ("features/cart", "features/checkout"),
            ("features/cart", "shared"),
            ("features/checkout", "shared"),
        ]
        .into_iter()
        .map(|(from, to)| Edge {
            from: from.into(),
            to: to.into(),
            file_pairs: 1,
            names: vec![],
        })
        .collect(),
    };
    let scene = Scene::focused(graph.clone(), "features/cart").unwrap();
    assert_eq!(
        scene
            .edges
            .iter()
            .filter(|route| scene.shows(&route.edge, "features"))
            .count(),
        0
    );
    assert_eq!(
        scene
            .edges
            .iter()
            .filter(|route| scene.shows(&route.edge, "features/cart"))
            .count(),
        2
    );
    assert_eq!(
        scene
            .edges
            .iter()
            .filter(|route| scene.shows(&route.edge, ""))
            .count(),
        0
    );
    assert!(scene.edges[2].points.is_empty()); // Unrelated checkout → shared is hidden.
    assert_eq!(
        scene
            .edges
            .iter()
            .filter(|route| !route.points.is_empty())
            .count(),
        2
    );
    let other = Scene::focused(graph.clone(), "features/checkout").unwrap();
    assert!(other.edges[1].points.is_empty()); // Unrelated cart → shared must not be routed.
    assert_eq!(
        other
            .edges
            .iter()
            .filter(|route| !route.points.is_empty())
            .count(),
        2
    );
    assert_eq!(
        relation(&other.edges[0].edge, "features/checkout"),
        Some(Direction::Incoming)
    );
    assert_eq!(
        relation(&other.edges[1].edge, "features/cart"),
        Some(Direction::Outgoing)
    );
    let empty = Scene::focused(graph, "").unwrap();
    assert!(empty.edges.iter().all(|route| route.points.is_empty()));
    for candidate in [&other, &empty] {
        assert_eq!(scene.edges.len(), candidate.edges.len()); // All import facts survive selection.
        assert_eq!(
            scene
                .boxes
                .iter()
                .map(|item| (&item.path, item.rect.top()))
                .collect::<std::collections::BTreeMap<_, _>>(),
            candidate
                .boxes
                .iter()
                .map(|item| (&item.path, item.rect.top()))
                .collect::<std::collections::BTreeMap<_, _>>()
        );
    }
    let item = |scene: &Scene, path: &str| {
        scene
            .boxes
            .iter()
            .find(|item| item.path == path)
            .map(|item| (item.rect, item.dimmed))
            .unwrap()
    };
    assert!(item(&scene, "features/history").1);
    assert!(!item(&scene, "features/cart").1);
    assert!(!item(&scene, "features/checkout").1);
    assert!(
        item(&scene, "features/history").0.left()
            > item(&scene, "features/checkout").0.right() + FOCUS_GAP
    );
    assert!(item(&scene, "features/history").0.left() > item(&empty, "features/history").0.left());
    assert_eq!(
        item(&scene, "features/history").0.top(),
        item(&scene, "features/cart").0.top()
    );
    assert!(empty.boxes.iter().all(|item| !item.dimmed));
}
#[test]
fn compound_layout_encloses_children_and_routes_all_imports_outside_nodes_and_titles() {
    let add = "features/cart/model/actions/add.ts";
    let index = "features/cart/model/actions/index.ts";
    let graph = Graph {
        nodes: vec![
            node(
                "features",
                "layer",
                vec![
                    node(
                        "features/cart",
                        "slice",
                        vec![node(
                            "features/cart/model",
                            "folder",
                            vec![node(
                                "features/cart/model/actions",
                                "group",
                                vec![node(add, "file", vec![]), node(index, "file", vec![])],
                            )],
                        )],
                    ),
                    node("features/checkout", "slice", vec![]),
                    node("features/order", "slice", vec![]),
                ],
            ),
            node("shared", "layer", vec![]),
        ],
        edges: [
            (index, add),
            (add, "shared"),
            ("features/checkout", add),
            ("features/order", "shared"),
        ]
        .into_iter()
        .map(|(from, to)| Edge {
            from: from.into(),
            to: to.into(),
            file_pairs: 1,
            names: vec![],
        })
        .collect(),
    };
    let elk = crate::layout_engine::layout(&graph).unwrap();
    let arranged = Scene::arrange(graph.clone(), add);
    let routed = Scene::focused(graph.clone(), add).unwrap();
    let offset = routed.boxes[0].rect.min - arranged.boxes[0].rect.min;
    for (before, after) in arranged.boxes.iter().zip(&routed.boxes) {
        assert_eq!(before.rect.translate(offset), after.rect); // Native ELK routing never rearranges cards.
    }
    let scenes = [
        Scene::from_elk(graph.clone(), &elk),
        Scene::new(graph),
        routed,
    ];
    for scene in scenes {
        assert_eq!(scene.edges.len(), 4);
        for (i, a) in scene.boxes.iter().enumerate() {
            for b in scene.boxes.iter().skip(i + 1) {
                if inside(&a.path, &b.path) || inside(&b.path, &a.path) {
                    continue;
                }
                assert!(
                    !a.rect.shrink(0.1).intersects(b.rect.shrink(0.1)),
                    "{} overlaps {}",
                    a.path,
                    b.path
                );
            }
        }
        for child in &scene.boxes {
            if let Some((parent, _)) = child.path.rsplit_once('/') {
                let parent = scene.boxes.iter().find(|item| item.path == parent).unwrap();
                assert!(parent.rect.contains_rect(child.rect));
                assert!(child.rect.top() >= parent.header().bottom());
            }
        }
        for route in &scene.edges {
            for segment in route.points.windows(2) {
                let (a, b) = (segment[0], segment[1]);
                assert!(a.x == b.x || a.y == b.y);
                for item in &scene.boxes {
                    let rect = item.header().shrink(0.1);
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
                    assert!(
                        !crosses,
                        "{} -> {} crosses {}",
                        route.edge.from, route.edge.to, item.path
                    );
                }
            }
        }
    }
}
