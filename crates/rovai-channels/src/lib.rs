//! Codeg aff1e6c Telegram/Weixin transport ports (Apache-2.0).
//! No donor database, conversation registry, task engine or Tauri commands.
pub mod chat_channel;
pub use chat_channel::{error::ChatChannelError,traits::ChatChannelBackend,types::*};
pub use chat_channel::backends::{telegram::TelegramBackend,weixin::{WeixinBackend,weixin_get_qrcode,weixin_check_qrcode,WeixinQrcodeInfo,WeixinQrcodeStatus,WeixinQrcodeStatusPublic}};
