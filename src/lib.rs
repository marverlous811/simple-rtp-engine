mod call;
mod ng_control;
mod sans_io_examples;

pub use call::*;
pub use ng_control::*;
pub use sans_io_examples::*;

pub enum MainEvent {
  CallAction(CallMsg),
  ActionResult(CallActionResult),
}
