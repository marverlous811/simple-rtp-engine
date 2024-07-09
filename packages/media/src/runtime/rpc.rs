use async_trait::async_trait;

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

pub struct Rpc<Req, Res> {
  pub req: Req,
  pub answer_tx: tokio::sync::oneshot::Sender<Res>,
}

impl<Req, Res> Rpc<Req, Res> {
  pub fn new(req: Req) -> (Self, tokio::sync::oneshot::Receiver<Res>) {
    let (answer_tx, answer_rx) = tokio::sync::oneshot::channel();
    (Self { req, answer_tx }, answer_rx)
  }
}

#[async_trait]
pub trait RpcHandler {
  async fn handle(&self, req: MediaRpcRequest) -> MediaRpcResponse;
}
