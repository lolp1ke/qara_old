use alloc::sync::Arc;
use core::any::Any;

use crossterm::event;
use ratatui::{
  Frame,
  layout::{Constraint, Rect},
  style::Style,
  text::Span,
  widgets::{Block, BorderType, Borders, Clear, Row, Table, TableState},
};
use tokio::sync::mpsc;

use crate::{Action, AppCtxt, Component, Mode, Vo, player};

#[derive(Debug)]
pub(crate) struct DownloadedItemsComponent {
  table: TableState,
  visible: bool,
}
impl DownloadedItemsComponent {
  pub(crate) fn new() -> Self {
    Self {
      visible: false,
      table: TableState::new(),
    }
  }
}
impl Component for DownloadedItemsComponent {
  fn as_any(&mut self) -> &mut dyn Any {
    self
  }
  fn draw(&mut self, frame: &mut Frame, area: Rect, cx: &AppCtxt) {
    if !self.visible {
      return;
    };
    frame.render_widget(Clear, area);

    let handles = &cx.torrent_handles;
    let items = handles.iter().flat_map(|handle| {
      Some(Row::new([
        handle.name()?,
        format!(
          "{:.2} Mb",
          handle.stats().total_bytes as f64 / 1024.0 / 1024.0
        ),
        handle.stats().progress_percent_human_readable().to_string(),
      ]))
    });

    let titles_row = ["filename", "size", "progress"];
    let mut rows = vec![Row::new(titles_row.map(Span::raw))];
    rows.extend(items);

    let table = Table::new(
      rows,
      [Constraint::Fill(1), Constraint::Min(3), Constraint::Min(3)],
    )
    .block(
      Block::new()
        .borders(Borders::LEFT | Borders::RIGHT)
        .border_type(BorderType::Thick),
    )
    .row_highlight_style(Style::default().cyan());
    frame.render_stateful_widget(table, area, &mut self.table);
  }
  fn dispatch_event(
    &mut self,
    event: Arc<event::Event>,
    tx: mpsc::UnboundedSender<Action>,
    cx: &AppCtxt,
  ) -> anyhow::Result<()> {
    if self.visible
      && let Mode::Downloads(prev_mode) = cx.mode.clone()
      && let event::Event::Key(event::KeyEvent {
        code,
        modifiers: _,
        kind: event::KeyEventKind::Press,
        state: _,
      }) = *event
    {
      match code {
        event::KeyCode::Char('k') | event::KeyCode::Up => {
          let selected =
            self.table.selected().map(|s| s.saturating_sub(1).max(1));

          self.table.select(selected);
        }
        event::KeyCode::Char('j') | event::KeyCode::Down => {
          let max = cx.torrent_handles.len();
          let selected =
            self.table.selected().map(|s| s.saturating_add(1).min(max));
          self.table.select(selected);
        }
        event::KeyCode::Char('p')
        | event::KeyCode::Char(' ')
        | event::KeyCode::Enter => {
          if let Some(item_idx) = self.table.selected()
            && let Some(handle) =
              cx.torrent_handles.get(item_idx.saturating_sub(1))
          {
            let url = format!(
              "http://127.0.0.1:3030/torrents/{}/stream/{}",
              handle.id(),
              handle
                .only_files()
                .and_then(|files| files.first().copied())
                .unwrap_or(0)
            );

            tx.send(Action::Enter(Mode::Player))?;
            tx.send(Action::Play(url.into()))?;
          };
        }
        event::KeyCode::Esc => {
          self.visible = false;
          tx.send(Action::Enter(*prev_mode))?;
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
    if let Action::Enter(Mode::Downloads(..)) = action {
      self.visible = true;
      self.table.select(Some(1));
    };
    Ok(())
  }
}
