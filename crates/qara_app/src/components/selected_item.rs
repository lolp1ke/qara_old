use alloc::sync::Arc;
use core::any::Any;

use crossterm::event;
use ratatui::{
  Frame,
  layout::Rect,
  text::{Line, Text},
  widgets::{Block, BorderType, Borders, Paragraph, Wrap},
};
use tokio::sync::mpsc;

use crate::{Action, AppCtxt, Component, Mode};

pub(crate) struct SelectedItemComponent {}
impl SelectedItemComponent {
  pub(crate) fn new() -> Self {
    Self {}
  }
}
impl Component for SelectedItemComponent {
  fn as_any(&mut self) -> &mut dyn Any {
    self
  }
  fn tick(&mut self) -> anyhow::Result<()> {
    Ok(())
  }
  fn draw(&mut self, frame: &mut Frame, area: Rect, cx: &AppCtxt) {
    if let Some(item) = cx.selected_item.clone() {
      let title = item.title().unwrap_or("<anon>");
      let link = item.link().unwrap_or("<yo wth?>");
      let Some(extension) = item.extensions().get("nyaa") else {
        eprintln!("Must've been the wind");
        return;
      };
      let downloads_ext = extension
        .get("downloads")
        .and_then(|e| e.first())
        .cloned()
        .unwrap_or_default();
      let seeds_ext = extension
        .get("seeders")
        .and_then(|e| e.first())
        .cloned()
        .unwrap_or_default();
      let downloads = downloads_ext.value().unwrap_or("<?>");
      let seeds = seeds_ext.value().unwrap_or("<?>");

      let paragraph = Paragraph::new(Text::from(vec![
        Line::raw(title),
        Line::raw(link),
        Line::raw(format!("downloads: {}; seeds: {}", downloads, seeds)),
      ]))
      .wrap(Wrap { trim: false })
      .block(
        Block::new()
          .borders(Borders::ALL)
          .border_type(BorderType::Thick),
      );

      frame.render_widget(paragraph, area);
    };
  }
  fn dispatch_event(
    &mut self,
    event: Arc<event::Event>,
    tx: mpsc::UnboundedSender<crate::Action>,
    cx: &AppCtxt,
  ) -> anyhow::Result<()> {
    if let Mode::Selected = cx.mode
      && let event::Event::Key(event::KeyEvent {
        code,
        modifiers: _,
        kind: event::KeyEventKind::Press,
        state: _,
      }) = *event
    {
      match code {
        event::KeyCode::Char('h') => {
          tx.send(Action::Enter(Mode::Results))?;
        }
        event::KeyCode::Esc => {
          tx.send(Action::Enter(Mode::Normal))?;
        }
        event::KeyCode::Char('i') => {
          tx.send(Action::Enter(Mode::Search))?;
        }

        event::KeyCode::Char('l')
        | event::KeyCode::Char(' ')
        | event::KeyCode::Enter => {
          if let Some(item) = cx.selected_item.clone()
            && let Some(link) = item.link()
          {
            tx.send(Action::Download(link.into()))?;
          };
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
