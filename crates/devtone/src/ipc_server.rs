use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crossbeam_channel::Sender;
use devtone_core::{decode, encode, Command, IpcRequest, IpcResponse, StateFrame};

pub fn socket_path() -> PathBuf {
    if let Ok(p) = std::env::var("DEVTONE_SOCK") {
        return PathBuf::from(p);
    }
    let base = std::env::var("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs_state_home()
        });
    base.join("devtone").join("devtone.sock")
}

fn dirs_state_home() -> PathBuf {
    std::env::var("HOME")
        .map(|h| PathBuf::from(h).join(".local/state"))
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
}

pub fn ping(path: &Path) -> bool {
    match send_request(path, &IpcRequest::Ping) {
        Ok(IpcResponse::Pong) => true,
        _ => false,
    }
}

pub fn send_request(path: &Path, req: &IpcRequest) -> std::io::Result<IpcResponse> {
    let mut stream = UnixStream::connect(path)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    write_msg(&mut stream, req)?;
    read_msg(&mut stream)
}

fn write_msg<T: serde::Serialize>(stream: &mut UnixStream, msg: &T) -> std::io::Result<()> {
    let bytes = encode(msg)?;
    stream.write_all(&bytes)?;
    stream.flush()
}

fn read_msg<T: serde::de::DeserializeOwned>(stream: &mut UnixStream) -> std::io::Result<T> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf)?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > 1_000_000 {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "frame too large"));
    }
    let mut body = vec![0u8; len];
    stream.read_exact(&mut body)?;
    decode(&body)
}

pub struct IpcServer {
    listener: UnixListener,
    path: PathBuf,
}

impl IpcServer {
    pub fn bind(path: &Path) -> std::io::Result<Self> {
        if path.exists() {
            match UnixStream::connect(path) {
                Ok(_) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::AddrInUse,
                        "already running",
                    ));
                }
                Err(_) => {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let listener = UnixListener::bind(path)?;
        listener.set_nonblocking(true)?;
        Ok(Self {
            listener,
            path: path.to_path_buf(),
        })
    }

    #[allow(dead_code)]
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn poll(&self, tx: &Sender<Command>, state: StateFrame) -> std::io::Result<()> {
        match self.listener.accept() {
            Ok((mut stream, _)) => {
                stream.set_nonblocking(false)?;
                stream.set_read_timeout(Some(Duration::from_millis(200)))?;
                let req: IpcRequest = match read_msg(&mut stream) {
                    Ok(r) => r,
                    Err(_) => return Ok(()),
                };
                let resp = match req {
                    IpcRequest::Ping => IpcResponse::Pong,
                    IpcRequest::Status => IpcResponse::State(state),
                    IpcRequest::Stop => {
                        let _ = tx.send(Command::Quit);
                        IpcResponse::Ok
                    }
                    IpcRequest::Subscribe => IpcResponse::Snapshot {
                        state,
                        params: Default::default(),
                        spectrum: Default::default(),
                    },
                    IpcRequest::Cmd(cmd) => {
                        let _ = tx.send(cmd);
                        IpcResponse::Ok
                    }
                };
                let _ = write_msg(&mut stream, &resp);
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) => return Err(e),
        }
        Ok(())
    }
}

impl Drop for IpcServer {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

pub fn run_test_loop(path: &Path) {
    let (tx, rx) = crossbeam_channel::bounded::<Command>(4);
    let server = IpcServer::bind(path).expect("bind");
    loop {
        let _ = server.poll(&tx, StateFrame::default());
        if let Ok(Command::Quit) = rx.try_recv() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(test)]
mod tests {
    use super::{ping, run_test_loop, send_request};
    use devtone_core::IpcRequest;

    #[test]
    fn ping_roundtrip_on_temp_socket() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("devtone.sock");
        let p = path.clone();
        let server = std::thread::spawn(move || run_test_loop(&p));
        std::thread::sleep(std::time::Duration::from_millis(80));
        assert!(ping(&path));
        let _ = send_request(&path, &IpcRequest::Stop);
        let _ = server.join();
    }
}
