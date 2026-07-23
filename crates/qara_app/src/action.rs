use alloc::sync::Arc;

use librqbit::{ManagedTorrent, dht::Id20};

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
  DeleteHandle(Id20, usize),

  Popup(Arc<str>, Arc<str>, Arc<[Arc<str>]>),

  Play(Arc<str>),
  NextVo,
  PrevVo,
}
