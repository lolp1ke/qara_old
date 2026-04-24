use alloc::sync::Arc;

use librqbit::ManagedTorrent;

use crate::{Mode, NyaaChannel};

#[derive(derive_more::Debug)]
#[derive(Clone)]
pub enum Action {
  Quit,
  Enter(Mode),

  Search(Arc<str>),
  SetChannel(Arc<NyaaChannel>),
  SelectItem(usize),
  Download(Arc<str>),

  AppendHandle(#[debug(skip)] Arc<ManagedTorrent>),

  Popup(Arc<str>, Arc<str>, Arc<[Arc<str>]>),
}
