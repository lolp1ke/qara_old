use rss::{Channel, Item};

#[derive(Debug)]
pub(crate) struct NyaaChannel {
  pub(crate) items: Vec<Item>,
  pub(crate) selected_item: Option<Item>,
}
impl NyaaChannel {
  pub fn new(channel: Channel) -> Self {
    Self {
      items: channel.items,
      selected_item: None,
    }
  }
}
