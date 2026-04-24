use alloc::sync::Arc;
use core::any::Any;
use std::time;

use crossterm::event;
use ratatui::{
  Frame,
  layout::{Position, Rect},
  text::Text,
  widgets::{Block, Borders, Paragraph},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{Action, AppCtxt, Component, Mode};

#[derive(Debug)]
pub(crate) struct InputComponent {
  input: String,
  idx: usize,
  focused: bool,
  show_cursor: bool,
  last_cursor_tick: time::Instant,
}
impl InputComponent {
  pub fn new() -> Self {
    Self {
      input: String::new(),
      idx: 0,
      focused: false,
      show_cursor: true,
      last_cursor_tick: time::Instant::now(),
    }
  }
  pub(crate) fn content(&self) -> &str {
    &self.input
  }

  fn insert_at(&mut self, idx: usize, ch: char) {
    self.input.insert(idx, ch);
    self.idx += 1;
  }
  fn delete(&mut self) {
    if self.idx > 0 {
      self.idx -= 1;
      self.input.remove(self.idx);
    };
  }
  fn left(&mut self) {
    if self.idx > 0 {
      self.idx -= 1;
    };
  }
  fn right(&mut self) {
    if self.idx < self.input.len() - 1 {
      self.idx += 1;
    }
  }
}
impl Component for InputComponent {
  fn as_any(&mut self) -> &mut dyn Any {
    self
  }
  fn tick(&mut self) -> anyhow::Result<()> {
    if self.focused
      && self.last_cursor_tick.elapsed() >= time::Duration::from_millis(500)
    {
      self.show_cursor = !self.show_cursor;
      self.last_cursor_tick = time::Instant::now();
    };

    Ok(())
  }
  fn draw(&mut self, frame: &mut Frame, area: Rect, cx: &AppCtxt) {
    if self.focused && self.show_cursor {
      frame.set_cursor_position(Position::new(self.idx as u16 + 1, 1));
    };

    frame.render_widget(Paragraph::new(Text::raw(self.input.clone())), area);
  }
  fn dispatch_event(
    &mut self,
    event: Arc<event::Event>,
    tx: UnboundedSender<Action>,
    cx: &AppCtxt,
  ) -> anyhow::Result<()> {
    if let event::Event::Key(event::KeyEvent {
      code: event::KeyCode::Char('i'),
      modifiers: _,
      kind: event::KeyEventKind::Press,
      state: _,
    }) = *event
      && matches!(cx.mode, Mode::Normal)
    {
      self.last_cursor_tick = time::Instant::now();
      self.focused = true;
    };

    if let event::Event::Key(event::KeyEvent {
      code,
      modifiers: _,
      kind: event::KeyEventKind::Press,
      state: _,
    }) = *event
      && matches!(cx.mode, Mode::Search)
    {
      match code {
        event::KeyCode::Char(ch) => {
          self.insert_at(self.idx, ch);
        }
        event::KeyCode::Backspace => {
          self.delete();
        }
        event::KeyCode::Left => {
          self.left();
        }
        event::KeyCode::Right => {
          self.right();
        }

        event::KeyCode::Esc => {
          self.focused = false;
          tx.send(Action::Enter(Mode::Normal))?;
        }

        _ => {}
      };
    };
    Ok(())
  }
  fn dispatch_action(
    &mut self,
    action: Action,
    cx: &AppCtxt,
  ) -> anyhow::Result<()> {
    Ok(())
  }
}
