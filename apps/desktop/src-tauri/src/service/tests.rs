use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
#[test]
fn registration_failures_distinguish_version_host_conflict_and_temporary_errors() {
    for code in [400, 404, 405, 415, 422, 501] {
        assert_eq!(
            registration_status(reqwest::StatusCode::from_u16(code).unwrap()).phase,
            Phase::Incompatible
        );
    }
    assert_eq!(
        registration_status(reqwest::StatusCode::CONFLICT).phase,
        Phase::OtherDesktop
    );
    assert_eq!(
        registration_status(reqwest::StatusCode::SERVICE_UNAVAILABLE).phase,
        Phase::Disconnected
    );
    assert_eq!(
        registration_status(reqwest::StatusCode::UNAUTHORIZED).phase,
        Phase::Disconnected
    );
}
fn descriptor(dir: &Path, version: u32) {
    let instance = Instance {
        protocol_version: version,
        pid: 12345,
        api_url: "http://127.0.0.1:1".into(),
        socket: dir.join("control.sock").to_string_lossy().into_owned(),
        token: "synthetic-recovery-token".into(),
    };
    std::fs::write(
        dir.join("instance.json"),
        serde_json::to_vec(&instance).unwrap(),
    )
    .unwrap();
}
fn owner(dir: &Path) -> std::fs::File {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join("daemon.lock"))
        .unwrap();
    file.try_lock_exclusive().unwrap();
    file
}
fn status(probe: Probe) -> Status {
    match probe {
        Probe::Unavailable(s) => s,
        Probe::Ready => panic!("Expected unavailable service"),
    }
}
#[tokio::test]
async fn missing_or_stale_service_can_start_but_an_owned_workspace_cannot() {
    let dir = tempfile::tempdir().unwrap();
    assert!(status(probe(dir.path()).await).can_start);
    descriptor(dir.path(), PROTOCOL_VERSION);
    assert!(status(probe(dir.path()).await).can_start);
    let lock = owner(dir.path());
    assert!(!status(probe(dir.path()).await).can_start);
    drop(lock);
    assert!(status(probe(dir.path()).await).can_start);
}
#[tokio::test]
async fn protocol_mismatch_requires_repair_only_while_a_service_owns_the_workspace() {
    let dir = tempfile::tempdir().unwrap();
    descriptor(dir.path(), PROTOCOL_VERSION + 1);
    let lock = owner(dir.path());
    let s = status(probe(dir.path()).await);
    assert_eq!(s.phase, Phase::Incompatible);
    assert!(!s.can_start);
    drop(lock);
    assert!(status(probe(dir.path()).await).can_start);
}
#[tokio::test]
async fn unreadable_oversized_or_symlink_descriptors_do_not_replace_a_live_owner() {
    let dir = tempfile::tempdir().unwrap();
    let _lock = owner(dir.path());
    for body in ["invalid".to_owned(), "x".repeat(32_001)] {
        std::fs::write(dir.path().join("instance.json"), body).unwrap();
        assert!(!status(probe(dir.path()).await).can_start);
    }
    std::fs::remove_file(dir.path().join("instance.json")).unwrap();
    std::os::unix::fs::symlink("/dev/null", dir.path().join("instance.json")).unwrap();
    assert!(!status(probe(dir.path()).await).can_start);
    let other = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(
        dir.path().join("daemon.lock"),
        other.path().join("daemon.lock"),
    )
    .unwrap();
    assert!(!status(probe(other.path()).await).can_start);
}
async fn server(dir: &Path, cap_status: u16, malformed: bool) -> tokio::task::JoinHandle<()> {
    descriptor(dir, PROTOCOL_VERSION);
    let listener = tokio::net::UnixListener::bind(dir.join("control.sock")).unwrap();
    tokio::spawn(async move {
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = vec![];
            loop {
                let mut part = [0; 1024];
                let n = stream.read(&mut part).await.unwrap();
                request.extend_from_slice(&part[..n]);
                if n == 0 || request.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            let request = String::from_utf8(request).unwrap();
            assert!(
                request
                    .to_lowercase()
                    .contains("authorization: bearer synthetic-recovery-token")
            );
            let (code, body) = if request.starts_with("GET /health ") {
                (
                    200,
                    format!("{{\"status\":\"ready\",\"protocol_version\":{PROTOCOL_VERSION}}}"),
                )
            } else {
                assert!(request.starts_with("GET /api/v1/plugin-capabilities "));
                (
                    cap_status,
                    if malformed { "{}".into() } else { "[]".into() },
                )
            };
            stream.write_all(format!("HTTP/1.1 {code} Result\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
        }
    })
}
#[tokio::test]
async fn health_alone_does_not_prove_plugin_interface_compatibility() {
    for (code, malformed) in [(404, false), (200, true)] {
        let dir = tempfile::tempdir().unwrap();
        let _lock = owner(dir.path());
        let task = server(dir.path(), code, malformed).await;
        let s = status(probe(dir.path()).await);
        assert_eq!(s.phase, Phase::Incompatible);
        assert!(!s.can_start);
        task.await.unwrap();
    }
    let dir = tempfile::tempdir().unwrap();
    let _lock = owner(dir.path());
    let task = server(dir.path(), 200, false).await;
    assert!(matches!(probe(dir.path()).await, Probe::Ready));
    task.await.unwrap();
}
#[tokio::test]
async fn stalled_service_probe_is_bounded_and_never_enables_another_start() {
    let dir = tempfile::tempdir().unwrap();
    descriptor(dir.path(), PROTOCOL_VERSION);
    let _lock = owner(dir.path());
    let listener = tokio::net::UnixListener::bind(dir.path().join("control.sock")).unwrap();
    let server = tokio::spawn(async move {
        let (_stream, _) = listener.accept().await.unwrap();
        tokio::time::sleep(Duration::from_secs(10)).await;
    });
    let s = tokio::time::timeout(Duration::from_secs(3), probe(dir.path()))
        .await
        .unwrap();
    assert!(!status(s).can_start);
    server.abort();
}
