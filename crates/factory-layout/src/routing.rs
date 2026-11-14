//! Orthogonal import routing on a shared visibility grid; node bodies and group titles are obstacles.
use emath::{Pos2, Rect, Vec2};
use std::{
    cmp::Reverse,
    collections::{BTreeMap, BinaryHeap},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Side {
    Left,
    Right,
    Top,
    Bottom,
}
type PortRequests = BTreeMap<(usize, Side), Vec<(usize, bool, Pos2)>>;

/// Allocate a distinct, ordered port to every connection, including incoming and outgoing
/// connections sharing a side. Reusing a node center makes otherwise separate routes overlap.
pub fn endpoints(nodes: &[Rect], edges: &[(usize, usize)]) -> Vec<(Pos2, Pos2, Pos2, Pos2)> {
    fn sides(from: Rect, to: Rect) -> (Side, Side) {
        if to.top() > from.bottom() + 1.0 {
            (Side::Bottom, Side::Top)
        } else if from.top() > to.bottom() + 1.0 {
            (Side::Top, Side::Bottom)
        } else if from.center().x < to.center().x {
            (Side::Right, Side::Left)
        } else {
            (Side::Left, Side::Right)
        }
    }
    fn port(rect: Rect, side: Side, index: usize, count: usize) -> (Pos2, Vec2) {
        let fraction = (index + 1) as f32 / (count + 1) as f32;
        let x = rect.left() + 16.0 + (rect.width() - 32.0) * fraction;
        let y = rect.top() + 16.0 + (rect.height() - 32.0) * fraction;
        match side {
            Side::Left => (Pos2::new(rect.left(), y), -Vec2::X),
            Side::Right => (Pos2::new(rect.right(), y), Vec2::X),
            Side::Top => (Pos2::new(x, rect.top()), -Vec2::Y),
            Side::Bottom => (Pos2::new(x, rect.bottom()), Vec2::Y),
        }
    }
    let mut groups = PortRequests::new();
    for (i, &(from, to)) in edges.iter().enumerate() {
        let (a, b) = sides(nodes[from], nodes[to]);
        groups
            .entry((from, a))
            .or_default()
            .push((i, true, nodes[to].center()));
        groups
            .entry((to, b))
            .or_default()
            .push((i, false, nodes[from].center()));
    }
    let mut result = vec![(Pos2::ZERO, Pos2::ZERO, Pos2::ZERO, Pos2::ZERO); edges.len()];
    for ((node, side), mut ports) in groups {
        ports.sort_by(|a, b| {
            let (primary_a, secondary_a, primary_b, secondary_b) = match side {
                Side::Top | Side::Bottom => (a.2.x, a.2.y, b.2.x, b.2.y),
                _ => (a.2.y, a.2.x, b.2.y, b.2.x),
            };
            primary_a
                .total_cmp(&primary_b)
                .then(secondary_a.total_cmp(&secondary_b))
                .then(a.0.cmp(&b.0))
        });
        for (index, &(edge, source, _)) in ports.iter().enumerate() {
            let (point, direction) = port(nodes[node], side, index, ports.len());
            if source {
                result[edge].0 = point;
                result[edge].1 = point + direction * 18.0;
            } else {
                result[edge].3 = point;
                result[edge].2 = point + direction * 18.0;
            }
        }
    }
    result
}

pub struct Router {
    xs: Vec<f32>,
    ys: Vec<f32>,
    horizontal: Vec<bool>,
    vertical: Vec<bool>,
    traffic: Vec<u16>,
}

impl Router {
    pub fn new(obstacles: &[Rect], endpoints: &[(Pos2, Pos2, Pos2, Pos2)], size: Vec2) -> Self {
        let obstacles: Vec<_> = obstacles.iter().map(|rect| rect.expand(8.0)).collect();
        let mut xs = vec![14.0, size.x - 14.0];
        let mut ys = vec![12.0, size.y - 12.0];
        for rect in &obstacles {
            for offset in [0.0, 9.0, 18.0, 27.0] {
                xs.extend([rect.left() - offset, rect.right() + offset]);
                ys.extend([rect.top() - offset, rect.bottom() + offset]);
            }
        }
        for (_, a, b, _) in endpoints {
            xs.extend([a.x, b.x]);
            ys.extend([a.y, b.y]);
        }
        for i in 0..endpoints.len().clamp(8, 40) {
            xs.push(size.x - 16.0 - i as f32 * 9.0);
        }
        xs.sort_by(f32::total_cmp);
        xs.dedup();
        ys.sort_by(f32::total_cmp);
        ys.dedup();
        let blocked = |point: Pos2| {
            obstacles.iter().any(|rect| {
                point.x > rect.left() + 0.01
                    && point.x < rect.right() - 0.01
                    && point.y > rect.top() + 0.01
                    && point.y < rect.bottom() - 0.01
            })
        };
        let (nx, ny) = (xs.len(), ys.len());
        let mut horizontal = vec![false; nx * ny];
        let mut vertical = vec![false; nx * ny];
        for y in 0..ny {
            for x in 0..nx {
                let point = Pos2::new(xs[x], ys[y]);
                if blocked(point) {
                    continue;
                }
                if x + 1 < nx {
                    horizontal[y * nx + x] = !blocked(Pos2::new(xs[x + 1], ys[y]))
                        && !blocked(Pos2::new((xs[x] + xs[x + 1]) * 0.5, ys[y]));
                }
                if y + 1 < ny {
                    vertical[y * nx + x] = !blocked(Pos2::new(xs[x], ys[y + 1]))
                        && !blocked(Pos2::new(xs[x], (ys[y] + ys[y + 1]) * 0.5));
                }
            }
        }
        Self {
            xs,
            ys,
            horizontal,
            vertical,
            traffic: vec![0; nx * ny * 2],
        }
    }

    pub fn route(&mut self, start: Pos2, end: Pos2) -> Vec<Pos2> {
        // Independent imports get separate tracks. Only a completely saturated channel may
        // reuse a track, with a length-based penalty instead of the previous tiny fixed cost.
        self.find_route(start, end, false)
            .or_else(|| self.find_route(start, end, true))
            .unwrap_or_else(|| vec![start, Pos2::new(start.x, end.y), end])
    }

    fn find_route(&mut self, start: Pos2, end: Pos2, share: bool) -> Option<Vec<Pos2>> {
        let nx = self.xs.len();
        let index = |point: Pos2| {
            self.ys.binary_search_by(|y| y.total_cmp(&point.y)).unwrap() * nx
                + self.xs.binary_search_by(|x| x.total_cmp(&point.x)).unwrap()
        };
        let start_index = index(start);
        let end_index = index(end);
        let mut cost = vec![usize::MAX; self.xs.len() * self.ys.len() * 2];
        let mut previous = vec![usize::MAX; cost.len()];
        let mut queue = BinaryHeap::new();
        for direction in 0..2 {
            cost[start_index * 2 + direction] = 0;
            queue.push((Reverse(0), start_index * 2 + direction));
        }
        let mut final_state = None;
        while let Some((Reverse(estimate), state)) = queue.pop() {
            let cell = state / 2;
            let (x, y) = (cell % nx, cell / nx);
            let heuristic = ((self.xs[x] - end.x).abs() + (self.ys[y] - end.y).abs()) * 10.0;
            if estimate > cost[state].saturating_add(heuristic.round() as usize) {
                continue;
            }
            if cell == end_index {
                final_state = Some(state);
                break;
            }
            let mut visit = |next: usize, direction: usize, distance: f32, wire: usize| {
                if !share && self.traffic[wire] > 0 {
                    return;
                }
                let next_state = next * 2 + direction;
                let crossing = if direction == 0 {
                    self.traffic[cell * 2 + 1] as usize
                        + if y > 0 {
                            self.traffic[(cell - nx) * 2 + 1] as usize
                        } else {
                            0
                        }
                } else {
                    self.traffic[cell * 2] as usize
                        + if x > 0 {
                            self.traffic[(cell - 1) * 2] as usize
                        } else {
                            0
                        }
                };
                let step = (distance * 10.0).round() as usize
                    + if state % 2 == direction { 0 } else { 180 }
                    + self.traffic[wire] as usize * (distance * 200.0).ceil() as usize
                    + crossing * 600;
                let next_cost = cost[state].saturating_add(step);
                if next_cost < cost[next_state] {
                    cost[next_state] = next_cost;
                    previous[next_state] = state;
                    let h = ((self.xs[next % nx] - end.x).abs()
                        + (self.ys[next / nx] - end.y).abs())
                        * 10.0;
                    queue.push((Reverse(next_cost + h.round() as usize), next_state));
                }
            };
            if x + 1 < nx && self.horizontal[cell] {
                visit(cell + 1, 0, self.xs[x + 1] - self.xs[x], cell * 2);
            }
            if x > 0 && self.horizontal[cell - 1] {
                visit(cell - 1, 0, self.xs[x] - self.xs[x - 1], (cell - 1) * 2);
            }
            if y + 1 < self.ys.len() && self.vertical[cell] {
                visit(cell + nx, 1, self.ys[y + 1] - self.ys[y], cell * 2 + 1);
            }
            if y > 0 && self.vertical[cell - nx] {
                visit(
                    cell - nx,
                    1,
                    self.ys[y] - self.ys[y - 1],
                    (cell - nx) * 2 + 1,
                );
            }
        }
        let mut state = final_state?;
        let mut path = vec![];
        loop {
            let cell = state / 2;
            path.push(Pos2::new(self.xs[cell % nx], self.ys[cell / nx]));
            let parent = previous[state];
            if parent == usize::MAX {
                break;
            }
            let lower = cell.min(parent / 2);
            let axis = usize::from(cell.abs_diff(parent / 2) == nx);
            self.traffic[lower * 2 + axis] = self.traffic[lower * 2 + axis].saturating_add(1);
            state = parent;
        }
        path.reverse();
        Some(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fanout_and_fanin_use_distinct_ports_and_separate_wire_tracks() {
        let nodes: Vec<_> = [
            (30.0, 30.0),
            (30.0, 400.0),
            (360.0, 400.0),
            (690.0, 400.0),
            (360.0, 30.0),
        ]
        .into_iter()
        .map(|(x, y)| Rect::from_min_size(Pos2::new(x, y), Vec2::new(220.0, 90.0)))
        .collect();
        let edges = [(0, 1), (0, 2), (0, 3), (4, 1), (4, 2), (4, 3), (1, 2)];
        let ports = endpoints(&nodes, &edges);
        for i in 0..3 {
            for j in i + 1..3 {
                assert_ne!(ports[i].0, ports[j].0);
            }
        }
        assert_ne!(ports[0].3, ports[3].3);
        let mut router = Router::new(&nodes, &ports, Vec2::new(1180.0, 600.0));
        let routes: Vec<_> = ports
            .into_iter()
            .map(|(a, start, end, b)| {
                let mut points = vec![a];
                points.extend(router.route(start, end));
                points.push(b);
                points
            })
            .collect();
        for i in 0..routes.len() {
            for j in i + 1..routes.len() {
                for a in routes[i].windows(2) {
                    for b in routes[j].windows(2) {
                        let overlap = if a[0].x == a[1].x && b[0].x == b[1].x && a[0].x == b[0].x {
                            a[0].y.max(a[1].y).min(b[0].y.max(b[1].y))
                                - a[0].y.min(a[1].y).max(b[0].y.min(b[1].y))
                        } else if a[0].y == a[1].y && b[0].y == b[1].y && a[0].y == b[0].y {
                            a[0].x.max(a[1].x).min(b[0].x.max(b[1].x))
                                - a[0].x.min(a[1].x).max(b[0].x.min(b[1].x))
                        } else {
                            0.0
                        };
                        assert!(
                            overlap < 0.1,
                            "routes {i} and {j} share {overlap}px of track"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn imports_route_around_node_bodies_with_orthogonal_segments() {
        let obstacle = Rect::from_min_max(Pos2::new(90.0, 30.0), Pos2::new(170.0, 110.0));
        let start = Pos2::new(30.0, 70.0);
        let end = Pos2::new(230.0, 70.0);
        let mut router = Router::new(
            &[obstacle],
            &[(start, start, end, end)],
            Vec2::new(280.0, 160.0),
        );
        let route = router.route(start, end);
        assert_eq!(route.first(), Some(&start));
        assert_eq!(route.last(), Some(&end));
        assert!(
            route
                .iter()
                .any(|point| point.y < obstacle.top() || point.y > obstacle.bottom())
        );
        for segment in route.windows(2) {
            assert!(segment[0].x == segment[1].x || segment[0].y == segment[1].y);
            assert!(!obstacle.contains(segment[0].lerp(segment[1], 0.5)));
        }
    }
}
