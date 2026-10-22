#![cfg(feature = "neural")]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use hf_hub::{Cache, Repo, RepoType, api::sync};

const GIT_HASH: &str = "0123456789abcdef0123456789abcdef01234567";
const LFS_HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const BODY: &[u8] = b"model bytes";

struct MockHub {
    endpoint: String,
    requests: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl MockHub {
    fn new(etag_header: &str, etag: &str, commit: &str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let thread_requests = Arc::clone(&requests);
        let thread_stop = Arc::clone(&stop);
        let etag_header = etag_header.to_owned();
        let etag = etag.to_owned();
        let commit = commit.to_owned();
        let thread = thread::spawn(move || {
            while !thread_stop.load(Ordering::Relaxed) {
                let (mut stream, _) = match listener.accept() {
                    Ok(connection) => connection,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    Err(error) => panic!("mock Hub accept failed: {error}"),
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = Vec::new();
                let mut buffer = [0; 1024];
                while !request.ends_with(b"\r\n\r\n") {
                    let read = stream.read(&mut buffer).unwrap();
                    if read == 0 {
                        break;
                    }
                    request.extend_from_slice(&buffer[..read]);
                }
                let count = thread_requests.fetch_add(1, Ordering::Relaxed);
                let response = if count == 0 {
                    format!(
                        "HTTP/1.1 206 Partial Content\r\n{etag_header}: \"{etag}\"\r\n\
                         x-repo-commit: {commit}\r\nContent-Range: bytes 0-0/{}\r\n\
                         Content-Length: 1\r\nConnection: close\r\n\r\nx",
                        BODY.len()
                    )
                    .into_bytes()
                } else {
                    let mut response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        BODY.len()
                    )
                    .into_bytes();
                    response.extend_from_slice(BODY);
                    response
                };
                stream.write_all(&response).unwrap();
            }
        });
        Self {
            endpoint,
            requests,
            stop,
            thread: Some(thread),
        }
    }

    fn request_count(&self) -> usize {
        self.requests.load(Ordering::Relaxed)
    }
}

impl Drop for MockHub {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Err(error) = self.thread.take().unwrap().join()
            && !thread::panicking()
        {
            std::panic::resume_unwind(error);
        }
    }
}

fn sync_api(cache: &Path, hub: &MockHub) -> sync::Api {
    sync::ApiBuilder::from_cache(Cache::new(cache.to_owned()))
        .with_token(None)
        .with_endpoint(hub.endpoint.clone())
        .with_progress(false)
        .build()
        .unwrap()
}

#[test]
fn rejects_untrusted_etags_before_download() {
    for etag_header in ["etag", "x-linked-etag"] {
        for traversal in ["absolute", "parent", "windows", "empty", "non-hex"] {
            let dir = tempfile::tempdir().unwrap();
            let victim = dir.path().join("victim");
            std::fs::write(&victim, b"keep").unwrap();
            let etag = match traversal {
                "absolute" => victim.to_string_lossy().into_owned(),
                "parent" => "../../../../victim".to_owned(),
                "windows" => r"C:\victim".to_owned(),
                "empty" => String::new(),
                _ => "z".repeat(40),
            };
            let hub = MockHub::new(etag_header, &etag, GIT_HASH);
            let result = sync_api(&dir.path().join("cache"), &hub)
                .model("test/model".to_owned())
                .download("model.bin");

            assert_eq!(std::fs::read(&victim).unwrap(), b"keep");
            assert!(
                matches!(result, Err(sync::ApiError::InvalidHeader(_))),
                "{etag_header} {traversal}: {result:?}"
            );
            assert_eq!(hub.request_count(), 1);
        }
    }
}

#[test]
fn rejects_untrusted_commit_before_publishing_blob() {
    let dir = tempfile::tempdir().unwrap();
    let outside = dir.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    let hub = MockHub::new("x-linked-etag", LFS_HASH, &outside.to_string_lossy());
    let cache = dir.path().join("cache");
    let result = sync_api(&cache, &hub)
        .model("test/model".to_owned())
        .download("model.bin");

    assert!(!outside.join("model.bin").exists());
    assert!(!cache.join("models--test--model/blobs").exists());
    assert!(
        matches!(result, Err(sync::ApiError::InvalidHeader(_))),
        "{result:?}"
    );
    assert_eq!(hub.request_count(), 1);
}

#[test]
fn valid_hashes_download_and_reuse_cached_files() {
    for (etag, commit) in [(GIT_HASH, GIT_HASH), (LFS_HASH, LFS_HASH)] {
        let dir = tempfile::tempdir().unwrap();
        let hub = MockHub::new("x-linked-etag", etag, commit);
        let repo = sync_api(&dir.path().join("cache"), &hub).repo(Repo::with_revision(
            "test/model".to_owned(),
            RepoType::Model,
            "refs/pr/1".to_owned(),
        ));
        let path = repo.get("nested/model.bin").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), BODY);
        assert_eq!(repo.get("nested/model.bin").unwrap(), path);
        assert_eq!(hub.request_count(), 2);
    }
}

#[test]
fn rejects_invalid_cached_refs() {
    let dir = tempfile::tempdir().unwrap();
    let outside = dir.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join("model.bin"), b"outside").unwrap();
    let cache = Cache::new(dir.path().join("cache"));
    let repo = cache.model("test/model".to_owned());
    let reference = cache.path().join("models--test--model/refs/main");
    std::fs::create_dir_all(reference.parent().unwrap()).unwrap();
    std::fs::write(&reference, outside.to_string_lossy().as_bytes()).unwrap();

    assert!(repo.get("model.bin").is_none());
    assert!(repo.create_ref("../outside").is_err());
}

#[test]
fn replacing_a_longer_cached_ref_truncates_it() {
    let dir = tempfile::tempdir().unwrap();
    let repo = Cache::new(dir.path().to_owned()).model("test/model".to_owned());
    repo.create_ref(LFS_HASH).unwrap();
    repo.create_ref(GIT_HASH).unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.path().join("models--test--model/refs/main")).unwrap(),
        GIT_HASH
    );
}

#[tokio::test]
async fn async_download_rejects_untrusted_metadata() {
    for invalid_etag in [true, false] {
        let dir = tempfile::tempdir().unwrap();
        let victim = dir.path().join("victim");
        std::fs::write(&victim, b"keep").unwrap();
        let bad_hash = victim.to_string_lossy();
        let (etag, commit) = if invalid_etag {
            (bad_hash.as_ref(), GIT_HASH)
        } else {
            (LFS_HASH, bad_hash.as_ref())
        };
        let hub = MockHub::new("x-linked-etag", etag, commit);
        let api = hf_hub::api::tokio::ApiBuilder::from_cache(Cache::new(dir.path().join("cache")))
            .with_token(None)
            .with_endpoint(hub.endpoint.clone())
            .with_progress(false)
            .build()
            .unwrap();
        let result = api
            .model("test/model".to_owned())
            .download("model.bin")
            .await;

        assert_eq!(std::fs::read(&victim).unwrap(), b"keep");
        assert!(
            matches!(result, Err(hf_hub::api::tokio::ApiError::InvalidHeader(_))),
            "{result:?}"
        );
        assert_eq!(hub.request_count(), 1);
    }
}

#[tokio::test]
async fn async_download_accepts_valid_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let hub = MockHub::new("etag", LFS_HASH, GIT_HASH);
    let api = hf_hub::api::tokio::ApiBuilder::from_cache(Cache::new(dir.path().join("cache")))
        .with_token(None)
        .with_endpoint(hub.endpoint.clone())
        .with_progress(false)
        .build()
        .unwrap();
    let repo = api.model("test/model".to_owned());
    let path = repo.get("model.bin").await.unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), BODY);
    assert_eq!(repo.get("model.bin").await.unwrap(), path);
    assert_eq!(hub.request_count(), 2);
}
