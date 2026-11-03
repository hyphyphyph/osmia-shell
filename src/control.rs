//! The control socket and the command line client.
//! One request line, one reply. The socket is owner-only.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use smithay_client_toolkit::reexports::calloop::channel::Sender;

pub struct Request {
    pub line: String,
    pub reply: mpsc::Sender<String>,
}

pub fn socket_path() -> PathBuf {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    // A nested session can set its own name so tests do not touch the real one.
    let name = std::env::var("OSMIA_SOCKET").unwrap_or_else(|_| "osmia.sock".into());
    dir.join(name)
}

/// Start listening. Fails if another shell already answers on the socket.
pub fn serve(tx: Sender<Request>) -> Result<(), String> {
    let path = socket_path();
    if UnixStream::connect(&path).is_ok() {
        return Err("another osmia shell is already running".into());
    }
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path).map_err(|e| format!("bind {}: {e}", path.display()))?;
    let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let tx = tx.clone();
            std::thread::spawn(move || handle(stream, tx));
        }
    });
    Ok(())
}

fn handle(mut stream: UnixStream, tx: Sender<Request>) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut line = String::new();
    if BufReader::new(&stream).read_line(&mut line).is_err() {
        return;
    }
    log::debug!("control socket: got {:?}", line.trim());
    let (rtx, rrx) = mpsc::channel();
    if tx
        .send(Request { line: line.trim().to_owned(), reply: rtx })
        .is_err()
    {
        return;
    }
    let reply = rrx
        .recv_timeout(Duration::from_secs(3))
        .unwrap_or_else(|_| "error: the shell did not answer".into());
    let _ = stream.write_all(reply.as_bytes());
    let _ = stream.write_all(b"\n");
}

/// The client. Exit codes: 0 done, 1 bad command or failure, 2 no shell running.
pub fn client(args: &[String]) -> i32 {
    let mut stream = match UnixStream::connect(socket_path()) {
        Ok(s) => s,
        Err(_) => {
            eprintln!("osmia: no shell is running");
            return 2;
        }
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    if writeln!(stream, "{}", args.join(" ")).is_err() {
        return 1;
    }
    let mut reply = String::new();
    let _ = stream.read_to_string(&mut reply);
    let reply = reply.trim_end();
    if let Some(e) = reply.strip_prefix("error:") {
        eprintln!("osmia: {}", e.trim());
        return 1;
    }
    if !reply.is_empty() {
        println!("{reply}");
    }
    0
}
