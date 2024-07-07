#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaRpcCmd {
  Ping,
  //call_id, leg_id, sdp
  Call(String, String, String),
  //call_id
  End(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaRpcResult {
  Ok,
  Pong,
  //sdp
  Call(String),
  End,
  //reason_error
  Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaRpcRequest {
  pub id: String,
  pub cmd: MediaRpcCmd,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaRpcResponse {
  pub id: String,
  pub res: MediaRpcResult,
}
