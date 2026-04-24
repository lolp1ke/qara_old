use alloc::sync::Arc;
use core::any::Any;

use crossterm::event;
use ratatui::{
  Frame,
  layout::{Alignment, Rect},
  text::Line,
  widgets::{Block, BorderType, Borders},
};
use tokio::sync::mpsc;

use crate::{Action, AppCtxt, Component, InputComponent, Mode};

#[derive(Debug)]
pub(crate) struct SearchComponent {
  pub(crate) input: InputComponent,
}
impl SearchComponent {
  pub(crate) fn new() -> Self {
    Self {
      input: InputComponent::new(),
    }
  }
}
impl Component for SearchComponent {
  fn as_any(&mut self) -> &mut dyn Any {
    self
  }
  fn tick(&mut self) -> anyhow::Result<()> {
    self.input.tick()
  }
  fn draw(&mut self, frame: &mut Frame, area: Rect, cx: &AppCtxt) {
    let block = Block::new()
      .borders(Borders::ALL)
      .border_type(BorderType::Rounded)
      .title(Line::raw("search bar").alignment(Alignment::Left))
      .title(Line::raw(format!("Qara[{}]", cx.vo)).alignment(Alignment::Center))
      .title(Line::raw(cx.mode.to_string()).alignment(Alignment::Right));
    let search_input = block.inner(area);
    frame.render_widget(block, area);
    self.input.draw(frame, search_input, cx)
  }
  fn dispatch_event(
    &mut self,
    event: Arc<event::Event>,
    tx: mpsc::UnboundedSender<Action>,
    cx: &AppCtxt,
  ) -> anyhow::Result<()> {
    if let Mode::Search = cx.mode
      && let event::Event::Key(event::KeyEvent {
        code: event::KeyCode::Enter,
        modifiers: _,
        kind: event::KeyEventKind::Press,
        state: _,
      }) = *event
    {
      tx.send(Action::Search(
        format!("https://nyaa.si/?page=rss&q={}", self.input.content()).into(),
      ))?;
    };

    self.input.dispatch_event(event, tx, cx)
  }
  fn dispatch_action(
    &mut self,
    action: Action,
    cx: &AppCtxt,
  ) -> anyhow::Result<()> {
    Ok(())
  }
}
