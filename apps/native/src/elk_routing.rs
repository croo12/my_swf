//! Fixed-node routing through the native backend used by ELK's Libavoid integration.
//! The elk-rs 0.12.2 Libavoid provider is a no-op; call the same upstream server directly.
use eframe::egui::{Pos2, Rect};
use std::{
    fmt::Write as _,
    io::{Read, Write},
    process::{Command, Stdio},
    sync::mpsc,
    time::Duration,
};

pub fn route(
    obstacles: &[Rect],
    connections: &[(usize, usize)],
    ports: &[(Pos2, Pos2, Pos2, Pos2)],
) -> Result<Vec<Vec<Pos2>>, String> {
    if connections.is_empty() {
        return Ok(vec![]);
    }
    let mut request = String::from(concat!(
        "OPTION edgeRouting ORTHOGONAL\n",
        "PENALTY segmentPenalty 20\n",
        "PENALTY crossingPenalty 600\n",
        "PENALTY clusterCrossingPenalty 4000\n",
        "PENALTY fixedSharedPathPenalty 2000\n",
        "PENALTY shapeBufferDistance 8\n",
        "PENALTY idealNudgingDistance 12\n",
        "ROUTINGOPTION nudgeOrthogonalSegmentsConnectedToShapes false\n",
        "ROUTINGOPTION penaliseOrthogonalSharedPathsAtConnEnds true\n",
        "ROUTINGOPTION nudgeOrthogonalTouchingColinearSegments false\n",
        "ROUTINGOPTION nudgeSharedPathsWithCommonEndPoint false\n",
        "GRAPH\n"
    ));
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
    let executable = crate::embedded_router::executable()?;
    let mut command = Command::new(executable);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("ELK Libavoid 실행 실패: {error}"))?;
    let mut output = child.stdout.take().unwrap();
    let mut input = child.stdin.take().unwrap();
    let (sender, receiver) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut text = String::new();
        let result = output
            .by_ref()
            .take(16 * 1024 * 1024)
            .read_to_string(&mut text)
            .map(|_| text);
        let _ = sender.send(result);
    });
    let written = input.write_all(request.as_bytes());
    drop(input); // EOF finishes the one-shot native routing request.
    if let Err(error) = written {
        let _ = child.kill();
        let _ = child.wait();
        let _ = reader.join();
        return Err(format!("ELK Libavoid 요청 실패: {error}"));
    }
    let response = receiver.recv_timeout(Duration::from_secs(15));
    if response.is_err() {
        let _ = child.kill();
    }
    let status = child.wait().map_err(|error| error.to_string())?;
    let _ = reader.join();
    let response = response
        .map_err(|_| "ELK Libavoid 연결선 계산 시간이 초과되었습니다.".to_string())?
        .map_err(|error| error.to_string())?;
    if !status.success() {
        return Err(format!("ELK Libavoid 종료 오류: {status}"));
    }
    let routes = parse(&response, connections.len())?;
    for (points, &(start, _, _, end)) in routes.iter().zip(ports) {
        // Upstream prints six significant digits; tolerate its output rounding at large coordinates.
        if points[0].distance(start) > 0.2 || points.last().unwrap().distance(end) > 0.2 {
            #[cfg(test)]
            eprintln!(
                "Expected {start:?} → {end:?}; actual {:?} → {:?}",
                points[0],
                points.last().unwrap()
            );
            return Err("ELK Libavoid가 지정한 연결 지점을 유지하지 못했습니다.".into());
        }
    }
    Ok(routes)
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
mod tests {
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
}
