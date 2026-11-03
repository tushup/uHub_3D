//! Localhost bridge (spec section 7): newline-delimited JSON over
//! TCP on 127.0.0.1 only. One reader task per plugin connection;
//! jobs are routed by uuid and fail fast when a connection drops.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot};

use crate::error::LibError;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionInfo {
    pub id: u64,
    pub dcc: String,
    pub dcc_version: String,
    pub pid: u32,
    pub renderers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobResult {
    pub job_id: String,
    pub ok: bool,
    pub message: String,
}

/// Bridge health, shown in the export dock (M2 Task 8).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum BridgeStatus {
    Online { port: u16 },
    Offline { port: u16, reason: String },
}

impl BridgeStatus {
    pub fn port(&self) -> u16 {
        match self {
            BridgeStatus::Online { port } => *port,
            BridgeStatus::Offline { port, .. } => *port,
        }
    }
}

type Pending = Arc<Mutex<HashMap<String, oneshot::Sender<Result<JobResult, LibError>>>>>;

struct Conn {
    info: ConnectionInfo,
    tx: mpsc::UnboundedSender<String>,
    pending: Pending,
    tasks: Mutex<Vec<tokio::task::JoinHandle<()>>>,
}

struct Inner {
    port: u16,
    next_id: AtomicU64,
    conns: Mutex<HashMap<u64, Arc<Conn>>>,
    on_change: Arc<dyn Fn(Vec<ConnectionInfo>) + Send + Sync>,
    accept_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

fn connections_of(inner: &Inner) -> Vec<ConnectionInfo> {
    let mut conns: Vec<ConnectionInfo> = lock(&inner.conns)
        .values()
        .map(|c| c.info.clone())
        .collect();
    conns.sort_by_key(|c| c.id);
    conns
}

fn notify(inner: &Inner) {
    (inner.on_change)(connections_of(inner));
}

/// Cheap to clone: the server state lives behind the `Arc`.
#[derive(Clone)]
pub struct Bridge {
    inner: Arc<Inner>,
}

impl Bridge {
    /// Bind 127.0.0.1:`port` (0 picks a free port in tests) and start
    /// accepting plugin connections.
    pub async fn start(
        port: u16,
        on_change: Arc<dyn Fn(Vec<ConnectionInfo>) + Send + Sync>,
    ) -> Result<Bridge, LibError> {
        let listener = TcpListener::bind(("127.0.0.1", port)).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::AddrInUse {
                LibError::Bridge(format!("Port {port} in use: {e}"))
            } else {
                LibError::Io(e)
            }
        })?;
        let bound_port = listener.local_addr().map(|a| a.port()).unwrap_or(port);
        let inner = Arc::new(Inner {
            port: bound_port,
            next_id: AtomicU64::new(1),
            conns: Mutex::new(HashMap::new()),
            on_change,
            accept_task: Mutex::new(None),
        });
        let accept_inner = inner.clone();
        let handle = tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                tokio::spawn(handle_connection(accept_inner.clone(), stream));
            }
        });
        *lock(&inner.accept_task) = Some(handle);
        Ok(Bridge { inner })
    }

    pub fn port(&self) -> u16 {
        self.inner.port
    }

    pub fn connections(&self) -> Vec<ConnectionInfo> {
        connections_of(&self.inner)
    }

    /// Drop the listener and every connection (used on port change).
    pub fn shutdown(&self) {
        if let Some(task) = lock(&self.inner.accept_task).take() {
            task.abort();
        }
        let conns: Vec<Arc<Conn>> = lock(&self.inner.conns).drain().map(|(_, c)| c).collect();
        for conn in conns {
            fail_pending(&conn, &conn.info.dcc);
            for task in lock(&conn.tasks).drain(..) {
                task.abort();
            }
        }
        notify(&self.inner);
    }

    /// Send one import job. The `asset` field is the asset.json file
    /// in `asset_dir`, parsed and sent exactly as stored on disk.
    pub async fn send_import(
        &self,
        conn_id: u64,
        renderer: &str,
        library_root: &Path,
        asset_dir: &Path,
        timeout: Duration,
    ) -> Result<JobResult, LibError> {
        let conn = lock(&self.inner.conns).get(&conn_id).cloned();
        let Some(conn) = conn else {
            return Err(LibError::Bridge("Not connected".to_string()));
        };
        let asset_path = asset_dir.join("asset.json");
        let bytes = std::fs::read(&asset_path)?;
        let asset: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|e| LibError::Parse(e.to_string()))?;
        let job_id = uuid::Uuid::new_v4().to_string();
        let (reply_tx, reply_rx) = oneshot::channel();
        lock(&conn.pending).insert(job_id.clone(), reply_tx);
        let line = serde_json::json!({
            "type": "import",
            "jobId": job_id,
            "renderer": renderer,
            "libraryRoot": library_root.display().to_string(),
            "assetDir": asset_dir.display().to_string(),
            "asset": asset,
        })
        .to_string();
        if conn.tx.send(line).is_err() {
            lock(&conn.pending).remove(&job_id);
            return Err(LibError::Bridge(format!(
                "Connection lost to {}",
                conn.info.dcc
            )));
        }
        match tokio::time::timeout(timeout, reply_rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(LibError::Bridge(format!(
                "Connection lost to {}",
                conn.info.dcc
            ))),
            Err(_) => {
                lock(&conn.pending).remove(&job_id);
                Err(LibError::Bridge("Timed out waiting for result".to_string()))
            }
        }
    }
}

fn fail_pending(conn: &Conn, dcc: &str) {
    let pending: Vec<_> = lock(&conn.pending).drain().collect();
    for (_, sender) in pending {
        let _ = sender.send(Err(LibError::Bridge(format!("Connection lost to {dcc}"))));
    }
}

async fn handle_connection(inner: Arc<Inner>, stream: tokio::net::TcpStream) {
    let (read_half, write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);
    let mut first = String::new();
    if reader.read_line(&mut first).await.unwrap_or(0) == 0 {
        return;
    }
    let Ok(hello) = serde_json::from_str::<serde_json::Value>(&first) else {
        return;
    };
    if hello.get("type").and_then(|v| v.as_str()) != Some("hello")
        || hello.get("protocol").and_then(|v| v.as_u64()) != Some(1)
    {
        return;
    }
    let dcc = hello
        .get("dcc")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let info = ConnectionInfo {
        id: inner.next_id.fetch_add(1, Ordering::Relaxed),
        dcc: dcc.clone(),
        dcc_version: hello
            .get("dccVersion")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        pid: hello.get("pid").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        renderers: hello
            .get("renderers")
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default(),
    };
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    let conn = Arc::new(Conn {
        info: info.clone(),
        tx,
        pending: Arc::new(Mutex::new(HashMap::new())),
        tasks: Mutex::new(Vec::new()),
    });
    let mut writer = write_half;
    let writer_task = tokio::spawn(async move {
        while let Some(line) = rx.recv().await {
            if writer.write_all(line.as_bytes()).await.is_err() {
                break;
            }
            if writer.write_all(b"\n").await.is_err() {
                break;
            }
            if writer.flush().await.is_err() {
                break;
            }
        }
    });
    lock(&conn.tasks).push(writer_task);
    lock(&inner.conns).insert(info.id, conn.clone());
    notify(&inner);

    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let Ok(msg) = serde_json::from_str::<serde_json::Value>(&line) else {
                    continue;
                };
                if msg.get("type").and_then(|v| v.as_str()) != Some("result") {
                    continue;
                }
                let Some(job_id) = msg.get("jobId").and_then(|v| v.as_str()) else {
                    continue;
                };
                let result = JobResult {
                    job_id: job_id.to_string(),
                    ok: msg.get("ok").and_then(|v| v.as_bool()).unwrap_or(false),
                    message: msg
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                };
                if let Some(sender) = lock(&conn.pending).remove(job_id) {
                    let _ = sender.send(Ok(result));
                }
            }
        }
    }
    lock(&inner.conns).remove(&info.id);
    notify(&inner);
    fail_pending(&conn, &dcc);
    for task in lock(&conn.tasks).drain(..) {
        task.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Asset, AssetType};
    use std::sync::Mutex as StdMutex;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::TcpStream;

    fn snapshots() -> (
        Arc<dyn Fn(Vec<ConnectionInfo>) + Send + Sync>,
        Arc<StdMutex<Vec<Vec<ConnectionInfo>>>>,
    ) {
        let store = Arc::new(StdMutex::new(Vec::new()));
        let captured = store.clone();
        (
            Arc::new(move |conns: Vec<ConnectionInfo>| {
                captured.lock().unwrap().push(conns);
            }),
            store,
        )
    }

    async fn wait_connections(bridge: &Bridge) -> Vec<ConnectionInfo> {
        for _ in 0..200 {
            let conns = bridge.connections();
            if !conns.is_empty() {
                return conns;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("connection never registered");
    }

    fn chair_asset() -> Asset {
        Asset {
            schema: 1,
            id: "abc".to_string(),
            r#type: AssetType::Model,
            name: "Chair".to_string(),
            category: "Furniture".to_string(),
            tags: vec![],
            created: "2026-10-30T00:00:00Z".to_string(),
            thumbnail: Some("thumbnail.png".to_string()),
            model: None,
            materials: vec![],
            hdri: None,
            view: None,
        }
    }

    fn write_asset_json(dir: &std::path::Path, asset: &Asset) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("asset.json"), serde_json::to_vec(asset).unwrap()).unwrap();
    }

    async fn connect_hello(
        bridge: &Bridge,
    ) -> (
        tokio::io::BufReader<tokio::net::tcp::OwnedReadHalf>,
        tokio::net::tcp::OwnedWriteHalf,
    ) {
        let stream = TcpStream::connect(("127.0.0.1", bridge.port()))
            .await
            .unwrap();
        let (read, mut write) = stream.into_split();
        write
            .write_all(b"{\"type\":\"hello\",\"protocol\":1,\"dcc\":\"blender\",\"dccVersion\":\"4.2.1\",\"pid\":7,\"renderers\":[\"cycles\",\"eevee\"]}\n")
            .await
            .unwrap();
        (BufReader::new(read), write)
    }

    #[tokio::test]
    async fn hello_registers_connection() {
        let (on_change, changes) = snapshots();
        let bridge = Bridge::start(0, on_change).await.unwrap();
        let (_reader, _writer) = connect_hello(&bridge).await;
        let conns = wait_connections(&bridge).await;
        assert_eq!(conns[0].dcc, "blender");
        assert_eq!(conns[0].dcc_version, "4.2.1");
        assert_eq!(conns[0].pid, 7);
        assert_eq!(conns[0].renderers, ["cycles", "eevee"]);
        assert!(!changes.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn bad_protocol_rejected() {
        let (on_change, _changes) = snapshots();
        let bridge = Bridge::start(0, on_change).await.unwrap();
        let stream = TcpStream::connect(("127.0.0.1", bridge.port()))
            .await
            .unwrap();
        let (read, mut write) = stream.into_split();
        let mut reader = BufReader::new(read);
        write
            .write_all(b"{\"type\":\"hello\",\"protocol\":2,\"dcc\":\"blender\",\"dccVersion\":\"4.2.1\",\"pid\":7,\"renderers\":[]}\n")
            .await
            .unwrap();
        let mut line = String::new();
        let read_result =
            tokio::time::timeout(Duration::from_secs(2), reader.read_line(&mut line)).await;
        assert!(bridge.connections().is_empty());
        // The server closes the socket without registering.
        assert!(matches!(read_result, Ok(Ok(0)) | Err(_)) || line.is_empty());
    }

    #[tokio::test]
    async fn import_roundtrip() {
        let (on_change, _changes) = snapshots();
        let bridge = Bridge::start(0, on_change).await.unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let asset_dir = tmp.path().join("Chair");
        write_asset_json(&asset_dir, &chair_asset());
        let (mut reader, mut writer) = connect_hello(&bridge).await;
        let conns = wait_connections(&bridge).await;
        let conn_id = conns[0].id;
        let bridge2 = bridge.clone();
        let root = tmp.path().to_path_buf();
        let dir = asset_dir.clone();
        let job = tokio::spawn(async move {
            bridge2
                .send_import(conn_id, "cycles", &root, &dir, Duration::from_secs(5))
                .await
        });
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        let msg: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(msg["type"], "import");
        assert_eq!(msg["renderer"], "cycles");
        assert_eq!(msg["assetDir"], asset_dir.to_string_lossy().as_ref());
        assert_eq!(msg["asset"]["name"], "Chair");
        let job_id = msg["jobId"].as_str().unwrap().to_string();
        writer
            .write_all(
                format!("{{\"type\":\"result\",\"jobId\":\"{job_id}\",\"ok\":true,\"message\":\"Imported Chair\"}}\n")
                    .as_bytes(),
            )
            .await
            .unwrap();
        let result = job.await.unwrap().unwrap();
        assert!(result.ok);
        assert_eq!(result.message, "Imported Chair");
        assert_eq!(result.job_id, job_id);
    }

    #[tokio::test]
    async fn import_sends_asset_json_verbatim() {
        let (on_change, _changes) = snapshots();
        let bridge = Bridge::start(0, on_change).await.unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let asset_dir = tmp.path().join("Chair");
        std::fs::create_dir_all(&asset_dir).unwrap();
        // Stored file, including an unknown key the Asset struct drops.
        let stored = "{\"schema\":1,\"id\":\"abc\",\"type\":\"model\",\"name\":\"Chair\",\"category\":\"Furniture\",\"tags\":[],\"created\":\"2026-10-30T00:00:00Z\",\"thumbnail\":null,\"materials\":[],\"futureKey\":{\"x\":1}}";
        std::fs::write(asset_dir.join("asset.json"), stored).unwrap();
        let expected: serde_json::Value = serde_json::from_str(stored).unwrap();
        let (mut reader, mut writer) = connect_hello(&bridge).await;
        let conn_id = wait_connections(&bridge).await[0].id;
        let bridge2 = bridge.clone();
        let root = tmp.path().to_path_buf();
        let dir = asset_dir.clone();
        let job = tokio::spawn(async move {
            bridge2
                .send_import(conn_id, "cycles", &root, &dir, Duration::from_secs(5))
                .await
        });
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        let msg: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(msg["asset"], expected);
        let job_id = msg["jobId"].as_str().unwrap().to_string();
        writer
            .write_all(
                format!("{{\"type\":\"result\",\"jobId\":\"{job_id}\",\"ok\":true,\"message\":\"ok\"}}\n")
                    .as_bytes(),
            )
            .await
            .unwrap();
        assert!(job.await.unwrap().unwrap().ok);
    }

    #[tokio::test]
    async fn import_times_out() {
        let (on_change, _changes) = snapshots();
        let bridge = Bridge::start(0, on_change).await.unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let asset_dir = tmp.path().join("Chair");
        write_asset_json(&asset_dir, &chair_asset());
        let (_reader, _writer) = connect_hello(&bridge).await;
        let conn_id = wait_connections(&bridge).await[0].id;
        let err = bridge
            .send_import(
                conn_id,
                "cycles",
                tmp.path(),
                &asset_dir,
                Duration::from_millis(200),
            )
            .await
            .unwrap_err();
        match err {
            LibError::Bridge(msg) => assert!(msg.contains("Timed out"), "{msg}"),
            other => panic!("expected Bridge error, got {other}"),
        }
    }

    #[tokio::test]
    async fn import_fails_fast_on_disconnect() {
        let (on_change, changes) = snapshots();
        let bridge = Bridge::start(0, on_change).await.unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let asset_dir = tmp.path().join("Chair");
        write_asset_json(&asset_dir, &chair_asset());
        let (mut reader, writer) = connect_hello(&bridge).await;
        let conn_id = wait_connections(&bridge).await[0].id;
        let bridge2 = bridge.clone();
        let root = tmp.path().to_path_buf();
        let dir = asset_dir.clone();
        let start = std::time::Instant::now();
        let job = tokio::spawn(async move {
            bridge2
                .send_import(conn_id, "cycles", &root, &dir, Duration::from_secs(10))
                .await
        });
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        assert!(line.contains("\"import\""));
        drop(reader);
        drop(writer);
        let err = job.await.unwrap().unwrap_err();
        match err {
            LibError::Bridge(msg) => assert!(msg.contains("Connection lost"), "{msg}"),
            other => panic!("expected Bridge error, got {other}"),
        }
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "took {:?}",
            start.elapsed()
        );
        for _ in 0..200 {
            if bridge.connections().is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(bridge.connections().is_empty());
        assert!(changes.lock().unwrap().last().is_some_and(|c| c.is_empty()));
    }

    #[tokio::test]
    async fn unknown_conn_id() {
        let (on_change, _changes) = snapshots();
        let bridge = Bridge::start(0, on_change).await.unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let asset_dir = tmp.path().join("Chair");
        write_asset_json(&asset_dir, &chair_asset());
        let err = bridge
            .send_import(
                999,
                "cycles",
                tmp.path(),
                &asset_dir,
                Duration::from_secs(1),
            )
            .await
            .unwrap_err();
        match err {
            LibError::Bridge(msg) => assert!(msg.contains("Not connected"), "{msg}"),
            other => panic!("expected Bridge error, got {other}"),
        }
    }

    #[tokio::test]
    async fn port_in_use_is_reported() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let (on_change, _changes) = snapshots();
        match Bridge::start(port, on_change).await {
            Err(LibError::Bridge(msg)) => assert!(msg.contains("in use"), "{msg}"),
            Err(other) => panic!("expected Bridge error, got {other}"),
            Ok(_) => panic!("expected bind to fail on an in-use port"),
        }
    }
}
