use alloc::sync::Arc;
use core::any::Any;

use crossterm::event;
use ratatui::{
  Frame,
  layout::{Alignment, Constraint, Layout, Rect},
  style::Style,
  widgets::{Block, Borders, Clear, Paragraph},
};
use tokio::sync::mpsc;

use crate::{Action, AppCtxt, Component, Mode};

#[derive(Debug)]
pub(crate) struct PopupComponent {
  pub(crate) title: Arc<str>,
  pub(crate) description: Arc<str>,
  pub(crate) buttons: Arc<[Arc<str>]>,
  pub(crate) selected: usize,
  pub(crate) visible: bool,
}
impl PopupComponent {
  pub fn new<I>(title: I, description: I, buttons: Arc<[Arc<str>]>) -> Self
  where
    I: Into<Arc<str>>,
  {
    Self {
      title: title.into(),
      description: description.into(),
      buttons,
      selected: 0,
      visible: false,
    }
  }
}
impl Component for PopupComponent {
  fn as_any(&mut self) -> &mut dyn Any {
    self
  }
  fn draw(&mut self, frame: &mut Frame, area: Rect, cx: &AppCtxt) {
    if !self.visible {
      return;
    };

    let area =
      area.centered(Constraint::Percentage(35), Constraint::Percentage(35));
    frame.render_widget(Clear, area);

    let layout =
      Layout::vertical([Constraint::Min(3), Constraint::Length(3)]).split(area);

    let block = Block::new().title(&*self.title).borders(Borders::ALL);
    frame.render_widget(block, area);
    let description =
      Paragraph::new(&*self.description).alignment(Alignment::Center);
    frame.render_widget(description, layout[0]);

    let layout = Layout::horizontal(
      self
        .buttons
        .iter()
        .map(|_| Constraint::Ratio(1, self.buttons.len() as u32))
        .collect::<Vec<_>>(),
    )
    .split(layout[1]);
    for (idx, text) in self.buttons.iter().enumerate() {
      let style = if idx == self.selected {
        Style::default().bold().cyan()
      } else {
        Style::default()
      };

      let button = Paragraph::new(&**text)
        .style(style)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));

      frame.render_widget(button, layout[idx]);
    }
  }
  fn dispatch_event(
    &mut self,
    event: Arc<event::Event>,
    tx: mpsc::UnboundedSender<Action>,
    cx: &AppCtxt,
  ) -> anyhow::Result<()> {
    if let Mode::Popup(prev_mode) = cx.mode.clone()
      && self.visible
      && let event::Event::Key(event::KeyEvent {
        code,
        modifiers: _,
        kind: event::KeyEventKind::Press,
        state: _,
      }) = *event
    {
      match code {
        event::KeyCode::Char('h') | event::KeyCode::Left => {
          if self.selected > 0 {
            self.selected -= 1;
          };
        }
        event::KeyCode::Char('l') | event::KeyCode::Right => {
          if self.selected < self.buttons.len() - 1 {
            self.selected += 1;
          };
        }
        event::KeyCode::Char(' ') | event::KeyCode::Enter => {}
        event::KeyCode::Esc => {
          self.visible = false;
          tx.send(Action::Enter(*prev_mode))?;
        }
        _ => {}
      }
    };
    Ok(())
  }
}
