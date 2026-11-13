//! Orthogonal import routing on a shared visibility grid; node bodies and group titles are obstacles.
use eframe::egui::{Pos2, Rect, Vec2};
use std::{cmp::Reverse, collections::BinaryHeap};

pub struct Router {
    xs: Vec<f32>,
    ys: Vec<f32>,
    horizontal: Vec<bool>,
    vertical: Vec<bool>,
    traffic: Vec<u16>,
}

impl Router {
    pub fn new(obstacles: &[Rect], endpoints: &[(Pos2, Pos2, Pos2, Pos2)], size: Vec2) -> Self {
        let obstacles: Vec<_> = obstacles.iter().map(|rect| rect.expand(6.0)).collect();
        let mut xs = vec![14.0, size.x - 14.0];
        let mut ys = vec![12.0, size.y - 12.0];
        for rect in &obstacles {
            xs.extend([
                rect.left() - 10.0,
                rect.left(),
                rect.right(),
                rect.right() + 10.0,
            ]);
            ys.extend([
                rect.top() - 10.0,
                rect.top(),
                rect.bottom(),
                rect.bottom() + 10.0,
            ]);
        }
        for (_, a, b, _) in endpoints {
            xs.extend([a.x, b.x]);
            ys.extend([a.y, b.y]);
        }
        for i in 0..8 {
            xs.push(size.x - 24.0 - i as f32 * 12.0);
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
                let next_state = next * 2 + direction;
                let step = (distance * 10.0).round() as usize
                    + if state % 2 == direction { 0 } else { 180 }
                    + self.traffic[wire] as usize * 45;
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
        let Some(mut state) = final_state else {
            return vec![start, Pos2::new(start.x, end.y), end];
        };
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
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
