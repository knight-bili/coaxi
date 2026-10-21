pub mod agent;
pub mod message;
pub mod conversation;
pub use agent::AgentType;
pub use message::*;
pub use conversation::*;
#[derive(Debug,Clone,Copy,serde::Serialize,serde::Deserialize)]
#[serde(rename_all="snake_case")]
pub enum ConversationKind{Regular,Chat,Loop,Delegate}
