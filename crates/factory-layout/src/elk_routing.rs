//! Fixed-node routing through the native backend used by ELK's Libavoid integration.
//! The elk-rs 0.12.2 Libavoid provider is a no-op; call the same upstream server directly.
use emath::{Pos2, Rect};
mod process;
mod spacing;
use std::{
    fmt::Write as _,
    time::{Duration, Instant},
};

pub fn route(
    obstacles: &[Rect],
    connections: &[(usize, usize)],
    ports: &[(Pos2, Pos2, Pos2, Pos2)],
) -> Result<Vec<Vec<Pos2>>, String> {
    if connections.is_empty() {
        return Ok(vec![]);
    }
    let executable = crate::embedded_router::executable()?;
    let timeout = Duration::from_secs(15);
    let started = Instant::now();
    let nudging = spacing::nudging_distance(connections, ports);
    let request = build_request(obstacles, connections, ports, nudging);
    let response = match process::execute(&executable, request, timeout) {
        Ok(response) => response,
        Err(error) if error.is_nudging_assertion() && nudging > 0.0 => {
            // Some valid compound layouts trip Libavoid's optional separation solver.
            // Keep the same native router and fixed pins; retry once without separation.
            let remaining = timeout.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                return Err(error.to_string());
            }
            let request = build_request(obstacles, connections, ports, 0.0);
            process::execute(&executable, request, remaining)
                .map_err(|retry| format!("{error}\n간격 정리 없이 재계산 실패: {retry}"))?
        }
        Err(error) => return Err(error.to_string()),
    };
    let routes = parse(&response, connections.len())?;
    for (points, &(start, _, _, end)) in routes.iter().zip(ports) {
        // Upstream prints six significant digits; tolerate its output rounding at large coordinates.
        if points[0].distance(start) > 0.2 || points.last().unwrap().distance(end) > 0.2 {
            return Err("ELK Libavoid가 지정한 연결 지점을 유지하지 못했습니다.".into());
        }
    }
    Ok(routes)
}

fn build_request(
    obstacles: &[Rect],
    connections: &[(usize, usize)],
    ports: &[(Pos2, Pos2, Pos2, Pos2)],
    nudging: f32,
) -> String {
    let mut request = String::from(concat!(
        "OPTION edgeRouting ORTHOGONAL\n",
        "PENALTY segmentPenalty 20\n",
        "PENALTY crossingPenalty 600\n",
        "PENALTY clusterCrossingPenalty 4000\n",
        "PENALTY fixedSharedPathPenalty 2000\n",
        "PENALTY shapeBufferDistance 8\n",
        "ROUTINGOPTION nudgeOrthogonalSegmentsConnectedToShapes false\n",
        "ROUTINGOPTION penaliseOrthogonalSharedPathsAtConnEnds true\n",
        "ROUTINGOPTION nudgeOrthogonalTouchingColinearSegments false\n",
        "ROUTINGOPTION nudgeSharedPathsWithCommonEndPoint false\n"
    ));
    let _ = writeln!(request, "PENALTY idealNudgingDistance {}", nudging);
    request.push_str("GRAPH\n");
    for (i, rect) in obstacles.iter().enumerate() {
        let _ = writeln!(
            request,
            "NODE {} {} {} {} {} 0 0",
            i + 1,
            rect.left(),
            rect.top(),
            rect.right(),
            rect.bottom()
        );
    }
    for (i, (&(from, to), &(start, a, b, end))) in connections.iter().zip(ports).enumerate() {
        // Upstream reserves port IDs 1..=4 for implicit incoming/outgoing pins.
        for (id, node, position, stub) in [(i * 2 + 5, from, start, a), (i * 2 + 6, to, end, b)] {
            let delta = stub - position;
            let side = if delta.x > 0.0 {
                "EAST"
            } else if delta.x < 0.0 {
                "WEST"
            } else if delta.y > 0.0 {
                "SOUTH"
            } else {
                "NORTH"
            };
            let offset = position - obstacles[node].min;
            let _ = writeln!(
                request,
                "PORT {id} {} {side} {} {}",
                node + 1,
                offset.x,
                offset.y
            );
        }
        let _ = writeln!(
            request,
            "PEDGEP {} {} {} {} {}",
            i + 1,
            from + 1,
            to + 1,
            i * 2 + 5,
            i * 2 + 6
        );
    }
    request.push_str("GRAPHEND\n");
    request
}

/// Packaging smoke check: exercises the actual embedded runtime without opening a window.
pub fn check_runtime() -> Result<(), String> {
    let obstacles = [
        Rect::from_min_max(Pos2::new(20.0, 80.0), Pos2::new(180.0, 160.0)),
        Rect::from_min_max(Pos2::new(240.0, 40.0), Pos2::new(360.0, 200.0)),
        Rect::from_min_max(Pos2::new(420.0, 80.0), Pos2::new(580.0, 160.0)),
    ];
    let connections = [(0, 2)];
    let ports = crate::routing::endpoints(&obstacles, &connections);
    route(&obstacles, &connections, &ports).map(|_| ())
}

fn parse(response: &str, count: usize) -> Result<Vec<Vec<Pos2>>, String> {
    if !response.lines().any(|line| line.trim() == "DONE") {
        return Err("ELK Libavoid 응답이 불완전합니다.".into());
    }
    let mut routes = vec![None; count];
    for line in response
        .lines()
        .filter_map(|line| line.strip_prefix("EDGE "))
    {
        let (id, values) = line.split_once('=').ok_or("ELK Libavoid 경로 형식 오류")?;
        let id = id
            .parse::<usize>()
            .map_err(|_| "ELK Libavoid 연결 ID 오류")?;
        if id == 0 || id > count || routes[id - 1].is_some() {
            return Err("ELK Libavoid 연결 ID 중복/누락".into());
        }
        let values = values
            .split_whitespace()
            .map(str::parse::<f32>)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "ELK Libavoid 좌표 오류")?;
        if values.len() < 4 || values.len() % 2 != 0 || values.iter().any(|v| !v.is_finite()) {
            return Err("ELK Libavoid 좌표가 유효하지 않습니다.".into());
        }
        let points: Vec<_> = values
            .as_chunks::<2>()
            .0
            .iter()
            .map(|xy| Pos2::new(xy[0], xy[1]))
            .collect();
        if points
            .windows(2)
            .any(|p| (p[0].x - p[1].x).abs() > 0.01 && (p[0].y - p[1].y).abs() > 0.01)
        {
            return Err("ELK Libavoid가 직각 경로를 반환하지 않았습니다.".into());
        }
        routes[id - 1] = Some(points);
    }
    routes
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| "ELK Libavoid 연결 경로가 누락되었습니다.".into())
}

#[cfg(test)]
mod tests;
