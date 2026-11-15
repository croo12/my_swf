use emath::Pos2;
use std::collections::BTreeMap;

/// Fixed pins can be closer than the preferred track spacing on a high-degree card.
/// Keep separation constraints below that physical spacing, leaving room on both sides.
pub(super) fn nudging_distance(
    connections: &[(usize, usize)],
    ports: &[(Pos2, Pos2, Pos2, Pos2)],
) -> f32 {
    let mut sides = BTreeMap::<(usize, u8), Vec<f32>>::new();
    for (&(from, to), &(start, a, b, end)) in connections.iter().zip(ports) {
        for (node, point, stub) in [(from, start, a), (to, end, b)] {
            let direction = stub - point;
            let (side, position) = if direction.x > 0.0 {
                (0, point.y)
            } else if direction.x < 0.0 {
                (1, point.y)
            } else if direction.y > 0.0 {
                (2, point.x)
            } else {
                (3, point.x)
            };
            sides.entry((node, side)).or_default().push(position);
        }
    }
    sides.into_values().fold(12.0, |distance, mut positions| {
        positions.sort_by(f32::total_cmp);
        positions.windows(2).fold(distance, |distance, pair| {
            distance.min((pair[1] - pair[0]).max(0.0) * 0.5)
        })
    })
}
