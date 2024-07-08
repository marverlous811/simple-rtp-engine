mod call;
mod ng_control;

pub use call::*;
pub use ng_control::*;

pub enum MainEvent {
  CallAction(CallMsg),
  ActionResult(CallActionResult),
}
