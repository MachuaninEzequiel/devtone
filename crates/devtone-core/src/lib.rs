pub mod ipc;
pub mod mapper;
pub mod palette;
pub mod scale;
pub mod tinystr;
pub mod types;

pub use ipc::{decode, encode, IpcRequest, IpcResponse};
pub use mapper::{ema, Mapper, ALPHA_FAST, ALPHA_MID, ALPHA_SLOW};
pub use scale::{degree_midi, scale_intervals};
pub use tinystr::TinyStr;
pub use types::*;
