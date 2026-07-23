use alloc::{borrow::Cow, sync::Arc};
use core::any::Any;

use crossterm::event;
use ratatui::{
  Frame,
  layout::{Constraint, Rect},
  style::Style,
  text::Span,
  widgets::{Block, BorderType, Borders, Row, Table, TableState},
};
use tokio::sync::mpsc;

use crate::{Action, AppCtxt, Component, Mode, components::num_str_len};

#[derive(Debug)]
pub struct ItemsComponent {
  pub table: TableState,
}
impl ItemsComponent {
  pub fn new() -> Self {
    Self {
      table: TableState::new(),
    }
  }
}
impl Component for ItemsComponent {
  fn as_any(&mut self) -> &mut dyn Any {
    self
  }
  fn tick(&mut self) -> anyhow::Result<()> {
    Ok(())
  }
  fn draw(&mut self, frame: &mut Frame, area: Rect, cx: &AppCtxt) {
    if let Some(channel) = &cx.channel {
      let items_len = channel.items.len();
      let items = channel.items.iter().enumerate().flat_map(|(idx, item)| {
        let Some(extension) = item.extensions().get("nyaa") else {
          eprintln!("Must've been the wind");
          return None;
        };
        let seeds_ext = extension
          .get("seeders")
          .and_then(|e| e.first())
          .cloned()
          .unwrap_or_default();
        let size_ext = extension
          .get("size")
          .and_then(|e| e.first())
          .cloned()
          .unwrap_or_default();

        let mut rows = Vec::new();
        rows.extend([
          Cow::Owned(format!("{}", idx)),
          Cow::Borrowed(item.title()?),
          Cow::Owned(size_ext.value()?.to_string()),
          Cow::Owned(seeds_ext.value()?.to_string()),
        ]);

        Some(Row::new(rows))
      });

      let title_row = ["idx", "title", "size", "seeders"];
      let mut rows = vec![Row::new(title_row.map(Span::raw))];
      rows.extend(items);

      let table = Table::new(
        rows,
        [
          Constraint::Max(num_str_len(items_len) as u16 + 1), // +1 cuz otherwise it is ugly
          Constraint::Fill(1),
          Constraint::Max(10),
          Constraint::Max(10),
        ],
      )
      .block(
        Block::new()
          .borders(Borders::LEFT | Borders::RIGHT)
          .border_type(BorderType::Thick),
      )
      .row_highlight_style(Style::default().cyan());

      frame.render_stateful_widget(table, area, &mut self.table);
    };
  }
  fn dispatch_event(
    &mut self,
    event: Arc<event::Event>,
    tx: mpsc::UnboundedSender<Action>,
    cx: &AppCtxt,
  ) -> anyhow::Result<()> {
    if let Mode::Results = cx.mode
      && let event::Event::Key(event::KeyEvent {
        code,
        modifiers: _,
        kind: event::KeyEventKind::Press,
        state: _,
      }) = *event
    {
      match code {
        event::KeyCode::Esc => {
          tx.send(Action::Enter(Mode::Normal))?;
        }
        event::KeyCode::Char('k') | event::KeyCode::Up => {
          if cx.channel.is_some() {
            let selected =
              self.table.selected().map(|s| s.saturating_sub(1).max(1));

            self.table.select(selected);
          } else {
            self.table.select(None);
          };
        }
        event::KeyCode::Char('j') | event::KeyCode::Down => {
          if let Some(channel) = &cx.channel {
            let max = channel.items.len();
            let selected =
              self.table.selected().map(|s| s.saturating_add(1).min(max));
            self.table.select(selected);
          };
        }

        event::KeyCode::Char('l')
        | event::KeyCode::Char(' ')
        | event::KeyCode::Enter => {
          if let Some(channel) = &cx.channel
            && let Some(item_idx) = self.table.selected()
            && channel.items.get(item_idx).is_some()
          {
            tx.send(Action::SelectItem(item_idx))?;
            tx.send(Action::Enter(Mode::Selected))?;
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
    if let Action::SetChannel(..) = action {
      self.table.select(Some(1));
    };
    Ok(())
  }
}
