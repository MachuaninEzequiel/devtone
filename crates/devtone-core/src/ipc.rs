use crate::{Command, MusicParams, SpectrumSnap, StateFrame};

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum IpcRequest {
    Ping,
    Stop,
    Status,
    Subscribe,
    Cmd(Command),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum IpcResponse {
    Pong,
    State(StateFrame),
    Snapshot {
        state: StateFrame,
        params: MusicParams,
        spectrum: SpectrumSnap,
    },
    Ok,
    Err(String),
}

pub fn encode(msg: &impl serde::Serialize) -> std::io::Result<Vec<u8>> {
    let body = serde_json::to_vec(msg).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let mut out = Vec::with_capacity(4 + body.len());
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

pub fn decode<T: serde::de::DeserializeOwned>(frame: &[u8]) -> std::io::Result<T> {
    serde_json::from_slice(frame).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

#[cfg(test)]
mod tests {
    use super::{decode, encode, IpcRequest, IpcResponse};

    #[test]
    fn ping_frame_roundtrips() {
        let bytes = encode(&IpcRequest::Ping).unwrap();
        let len = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
        let msg: IpcRequest = decode(&bytes[4..4 + len]).unwrap();
        assert_eq!(msg, IpcRequest::Ping);
        let pong = encode(&IpcResponse::Pong).unwrap();
        assert!(pong.len() > 4);
    }
}
