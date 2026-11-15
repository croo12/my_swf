use std::{
    fmt,
    io::{self, Read, Write},
    path::Path,
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

pub(super) enum Error {
    Native { status: ExitStatus, detail: String },
    Other(String),
}
impl Error {
    pub(super) fn is_nudging_assertion(&self) -> bool {
        matches!(self, Self::Native { detail, .. } if detail.to_ascii_lowercase().contains("assert") && detail.contains("orthogonal.cpp"))
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Native { status, detail } => {
                write!(f, "ELK Libavoid 종료 오류: {status}")?;
                if !detail.is_empty() {
                    write!(f, "\n{detail}")?;
                }
                Ok(())
            }
            Self::Other(message) => f.write_str(message),
        }
    }
}

// Drain the complete pipe even after the retained prefix is full, so stderr cannot
// deadlock the child or introduce a broken pipe while reporting the original failure.
fn drain(mut pipe: impl Read, limit: usize) -> io::Result<(Vec<u8>, bool)> {
    let mut bytes = Vec::new();
    let mut truncated = false;
    let mut buffer = [0; 4096];
    loop {
        let count = pipe.read(&mut buffer)?;
        if count == 0 {
            return Ok((bytes, truncated));
        }
        let retained = count.min(limit.saturating_sub(bytes.len()));
        bytes.extend_from_slice(&buffer[..retained]);
        truncated |= retained != count;
    }
}

fn wait(child: &mut Child, timeout: Duration) -> Result<ExitStatus, Error> {
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if started.elapsed() < timeout => thread::sleep(Duration::from_millis(5)),
            result => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Error::Other(match result {
                    Err(error) => format!("ELK Libavoid 대기 실패: {error}"),
                    _ => "ELK Libavoid 연결선 계산 시간이 초과되었습니다.".into(),
                }));
            }
        }
    }
}

pub(super) fn execute(
    executable: &Path,
    request: String,
    timeout: Duration,
) -> Result<String, Error> {
    let mut command = Command::new(executable);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let mut child = command
        .spawn()
        .map_err(|e| Error::Other(format!("ELK Libavoid 실행 실패: {e}")))?;
    let mut input = child.stdin.take().unwrap();
    let output = child.stdout.take().unwrap();
    let errors = child.stderr.take().unwrap();
    let writer = thread::spawn(move || input.write_all(request.as_bytes()));
    let reader = thread::spawn(move || drain(output, 16 * 1024 * 1024));
    let error_reader = thread::spawn(move || drain(errors, 32 * 1024));
    let status = wait(&mut child, timeout);
    let written = writer.join();
    let response = reader.join();
    let errors = error_reader.join();
    let status = status?;
    let detail = errors
        .ok()
        .and_then(Result::ok)
        .map(|(bytes, _)| String::from_utf8_lossy(&bytes).trim().to_owned())
        .unwrap_or_default();
    if !status.success() {
        return Err(Error::Native { status, detail });
    }
    written
        .map_err(|_| Error::Other("ELK Libavoid 요청 작업이 중단되었습니다.".into()))?
        .map_err(|e| Error::Other(format!("ELK Libavoid 요청 실패: {e}")))?;
    let (bytes, truncated) = response
        .map_err(|_| Error::Other("ELK Libavoid 응답 작업이 중단되었습니다.".into()))?
        .map_err(|e| Error::Other(format!("ELK Libavoid 응답 실패: {e}")))?;
    if truncated {
        return Err(Error::Other(
            "ELK Libavoid 응답이 크기 제한을 초과했습니다.".into(),
        ));
    }
    String::from_utf8(bytes).map_err(|e| Error::Other(format!("ELK Libavoid 응답 형식 오류: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pipe_capture_stays_bounded_and_drains_the_unretained_tail() {
        let mut source = io::Cursor::new(vec![b'x'; 20_000]);
        let (bytes, truncated) = drain(&mut source, 100).unwrap();
        assert_eq!(bytes.len(), 100);
        assert!(truncated);
        assert_eq!(source.position(), 20_000);
    }
}
