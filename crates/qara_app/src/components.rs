mod common;
mod downloaded_items;
mod items;
mod search;
mod selected_item;

pub(crate) use common::*;
pub(crate) use downloaded_items::*;
pub(crate) use items::*;
pub(crate) use search::*;
pub(crate) use selected_item::*;

use alloc::sync::Arc;
use core::{any::Any, fmt};

use crossterm::event;
use ratatui::{Frame, layout::Rect};
use tokio::sync::mpsc::UnboundedSender;

use crate::{Action, AppCtxt, components::items::ItemsComponent};

#[derive(Debug)]
pub(crate) struct Components {
  pub(crate) search: Box<dyn Component>,
  pub(crate) items: Box<dyn Component>,
  pub(crate) selected_item: Box<dyn Component>,
  pub(crate) downloaded_items: Box<dyn Component>,
  pub(crate) popup: Box<dyn Component>,
}
impl Components {
  pub(crate) fn new() -> Self {
    Self {
      search: Box::new(SearchComponent::new()),
      items: Box::new(ItemsComponent::new()),
      selected_item: Box::new(SelectedItemComponent::new()),
      downloaded_items: Box::new(DownloadedItemsComponent::new()),

      popup: Box::new(PopupComponent::new("Popup", "", Default::default())),
    }
  }

  pub(crate) fn tick(&mut self) -> anyhow::Result<()> {
    self.search.tick()?;
    self.items.tick()?;
    self.selected_item.tick()?;
    self.downloaded_items.tick()?;
    self.popup.tick()?;
    Ok(())
  }
  pub(crate) fn dispatch_event(
    &mut self,
    event: Arc<event::Event>,
    tx: UnboundedSender<Action>,
    cx: &AppCtxt,
  ) -> anyhow::Result<()> {
    self.search.dispatch_event(event.clone(), tx.clone(), cx)?;
    self.items.dispatch_event(event.clone(), tx.clone(), cx)?;
    self
      .selected_item
      .dispatch_event(event.clone(), tx.clone(), cx)?;
    self
      .downloaded_items
      .dispatch_event(event.clone(), tx.clone(), cx)?;
    self.popup.dispatch_event(event.clone(), tx.clone(), cx)?;
    Ok(())
  }
  pub(crate) fn dispatch_action(
    &mut self,
    action: Action,
    cx: &AppCtxt,
  ) -> anyhow::Result<()> {
    self.search.dispatch_action(action.clone(), cx)?;
    self.items.dispatch_action(action.clone(), cx)?;
    self.selected_item.dispatch_action(action.clone(), cx)?;
    self.downloaded_items.dispatch_action(action.clone(), cx)?;
    self.popup.dispatch_action(action.clone(), cx)?;
    Ok(())
  }
}

pub trait Component {
  fn as_any(&mut self) -> &mut dyn Any;
  fn tick(&mut self) -> anyhow::Result<()> {
    Ok(())
  }
  fn draw(&mut self, frame: &mut Frame, area: Rect, cx: &AppCtxt);
  fn dispatch_event(
    &mut self,
    event: Arc<event::Event>,
    tx: UnboundedSender<Action>,
    cx: &AppCtxt,
  ) -> anyhow::Result<()> {
    Ok(())
  }
  fn dispatch_action(
    &mut self,
    action: Action,
    cx: &AppCtxt,
  ) -> anyhow::Result<()> {
    Ok(())
  }
  fn update<F>(&mut self, f: F)
  where
    F: FnOnce(&mut Self),
    Self: Sized,
  {
    f(self);
  }
}
impl fmt::Debug for dyn Component {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("dyn Component").finish_non_exhaustive()
  }
}

// true for base10 numbers
fn num_str_len(num: usize) -> usize {
  if num == 0 {
    1
  } else {
    (num as f32).log10().floor() as usize + 1
  }
}
