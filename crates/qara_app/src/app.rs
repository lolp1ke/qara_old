use alloc::sync::Arc;
use core::sync::{atomic, atomic::AtomicBool};
use std::{
  env::home_dir,
  io::{Stdout, stdin},
  process::Stdio,
  sync::OnceLock,
};

use anyhow::Context as _;
use crossterm::event::{self, EventStream};
use futures_util::StreamExt as _;
use librqbit::{
  AddTorrent, AddTorrentOptions, Api, ManagedTorrent, Session, SessionOptions,
  SessionPersistenceConfig,
};
use ratatui::{
  Terminal,
  layout::{Constraint, Layout},
  prelude::CrosstermBackend,
};
use rss::{Channel, Item};
use tokio::{
  runtime::Handle,
  sync::{mpsc, oneshot},
};

use qara_video::{Player, Vo};

use crate::{Action, Component, Components, Mode, NyaaChannel, PopupComponent};

static TICK_RATE: OnceLock<f32> = OnceLock::new();
fn tick_rate() -> f32 {
  *TICK_RATE.get_or_init(|| 24.0)
}
static FRAME_RATE: OnceLock<f32> = OnceLock::new();
fn frame_rate() -> f32 {
  *FRAME_RATE.get_or_init(|| 60.0)
}

#[derive(derive_more::Debug)]
pub(crate) struct AppCtxt {
  // pub(crate) video: Player,
  pub(crate) vo: Vo,
  pub(crate) width: u16,
  pub(crate) height: u16,
  pub(crate) mode: Mode,
  #[debug(skip)]
  pub(crate) channel: Option<Arc<NyaaChannel>>,
  #[debug(skip)]
  pub(crate) items: Vec<Item>,
  pub(crate) selected_item: Option<Arc<Item>>,
  #[debug(skip)]
  pub(crate) torrent_handles: Vec<Arc<ManagedTorrent>>,
}
impl Default for AppCtxt {
  fn default() -> Self {
    let size = crossterm::terminal::size().unwrap();

    Self {
      // video: Player::default(),
      vo: Vo::Mpv,
      width: size.0,
      height: size.1,
      mode: Mode::default(),
      channel: None,
      items: Vec::new(),
      selected_item: None,
      torrent_handles: Vec::new(),
    }
  }
}

#[derive(derive_more::Debug)]
pub struct App {
  quit: AtomicBool,
  executor: Arc<Handle>,

  pub(crate) tx: mpsc::UnboundedSender<Action>,
  rx: mpsc::UnboundedReceiver<Action>,

  player_stop: Option<oneshot::Sender<()>>,

  #[debug(skip)]
  term: Terminal<CrosstermBackend<Stdout>>,
  #[debug(skip)]
  components: Components,

  #[debug(skip)]
  session: Option<Arc<Session>>,
}
impl App {
  pub fn new(executor: Arc<Handle>, frame_rate: f32, tick_rate: f32) -> Self {
    let (tx, rx) = mpsc::unbounded_channel();
    TICK_RATE.get_or_init(|| tick_rate);
    FRAME_RATE.get_or_init(|| frame_rate);

    Self {
      quit: AtomicBool::new(false),
      executor,
      tx,
      rx,
      player_stop: None,
      term: ratatui::init(),
      components: Components::new(),
      session: None,
    }
  }

  async fn init(&mut self, cx: &mut AppCtxt) -> anyhow::Result<()> {
    let has_mpv = tokio::process::Command::new("which")
      .stdin(Stdio::null())
      .stdout(Stdio::null())
      .stderr(Stdio::null())
      .arg("mpv")
      .status()
      .await?;
    if !has_mpv.success() {
      cx.vo = Vo::Ansi;

      let has_ffmpeg = tokio::process::Command::new("which")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .arg("ffmpeg")
        .status()
        .await?;
      if !has_ffmpeg.success() {
        self.cleanup()?;
        eprintln!("Either mpv or ffmpeg must be installed.");
        std::process::exit(1);
      };
    };

    // TODO: macos friendly rn
    let downloads_dir = home_dir()
      .context("no home bro?")?
      .join("Downloads")
      .join("qara");
    std::fs::create_dir_all(&downloads_dir)?;

    self.session = Some(
      Session::new_with_opts(
        downloads_dir.clone(),
        SessionOptions {
          disable_dht: false,
          persistence: Some(SessionPersistenceConfig::Json {
            folder: Some(downloads_dir),
          }),
          disable_upload: true,
          ..Default::default()
        },
      )
      .await?,
    );

    if let Some(session) = self.session.clone() {
      session.with_torrents(|torrents| {
        for (_, t) in torrents {
          self.tx.send(Action::AppendHandle(t.clone()))?;
        }
        anyhow::Ok(())
      })?;

      let api = Api::new(session, None, None);
      let app = librqbit::http_api::HttpApi::new(api, None);
      let listener = tokio::net::TcpListener::bind("127.0.0.1:3030").await?;
      self.executor.spawn(async move {
        app.make_http_api_and_run(listener, None).await?;
        anyhow::Ok(())
      });
    };

    Ok(())
  }
  pub fn cleanup(&self) -> anyhow::Result<()> {
    ratatui::restore();
    Ok(())
  }

  fn execute_action(
    &mut self,
    action: Action,
    cx: &mut AppCtxt,
  ) -> anyhow::Result<()> {
    match action.clone() {
      Action::Quit => {
        self.quit.store(true, atomic::Ordering::Relaxed);
      }
      Action::Enter(mode) => {
        cx.mode = mode;
      }

      Action::Search(url) => {
        self.tx.send(Action::Enter(Mode::Searching))?;
        self.executor.spawn({
          let tx = self.tx.clone();
          async move {
            let rss_bytes = reqwest::get(&*url).await?.bytes().await?;
            let channel = Channel::read_from(&*rss_bytes)?;
            tx.send(Action::SetChannel(Arc::new(NyaaChannel::new(channel))))?;
            tx.send(Action::Enter(Mode::Results))?;

            anyhow::Ok(())
          }
        });
      }
      Action::SetChannel(channel) => {
        cx.items = channel.items.clone();
        cx.channel = Some(channel);
      }
      Action::SelectItem(idx) => {
        if let Some(channel) = &cx.channel
          && let Some(item) = channel.items.get(idx)
        {
          cx.selected_item = Some(item.clone().into());
        };
      }

      Action::Download(link) => {
        if let Some(session) = self.session.clone() {
          self.executor.spawn({
            let tx = self.tx.clone();
            async move {
              if let Err(e) = {
                let tx = tx.clone();
                async move {
                  let torrent_bytes =
                    reqwest::get(&*link).await?.bytes().await?;
                  let handle = session
                    .add_torrent(
                      AddTorrent::from_bytes(torrent_bytes),
                      Some(AddTorrentOptions {
                        overwrite: true,
                        ..Default::default()
                      }),
                    )
                    .await?
                    .into_handle()
                    .context("no handle")?;

                  tx.send(Action::AppendHandle(handle))?;
                  anyhow::Ok(())
                }
              }
              .await
              {
                tx.send(Action::Popup(
                  "Error".into(),
                  format!("{}", e).into(),
                  Arc::new(["kk".into(), "quit".into()]),
                ))?;
              };

              anyhow::Ok(())
            }
          });
        };
      }

      Action::AppendHandle(handle) => {
        cx.torrent_handles.push(handle);
      }

      Action::Popup(title, description, buttons) => {
        cx.mode = Mode::Popup(Box::new(cx.mode.clone()));
        self
          .components
          .popup
          .as_any()
          .downcast_mut::<PopupComponent>()
          .context("context")?
          .update(|p| {
            p.title = title;
            p.description = description;
            p.buttons = buttons;
            p.visible = true;
          });
      }

      Action::Play(url) => {
        let (stop_tx, stop_rx) = oneshot::channel();
        if let Some(stop_tx) = self.player_stop.take() {
          stop_tx.send(()).unwrap();
        };
        self.player_stop = Some(stop_tx);

        let player = Player::new(
          cx.vo.clone(),
          url,
          self.executor.clone(),
          cx.width,
          cx.height,
          stop_rx,
        );
        self.executor.spawn(async move {
          player.play().await?;

          anyhow::Ok(())
        });
      }
      Action::NextVo => {
        cx.vo = cx.vo.next();
      }
      Action::PrevVo => {
        cx.vo = cx.vo.prev();
      }
    };
    self.components.dispatch_action(action, cx)?;

    Ok(())
  }
  fn handle_key_event(
    &mut self,
    key: event::KeyEvent,
    cx: &mut AppCtxt,
  ) -> anyhow::Result<()> {
    if !matches!(key.kind, event::KeyEventKind::Press) {
      return Ok(());
    };

    match cx.mode {
      Mode::Normal => {
        match key.code {
          event::KeyCode::Char('q') => {
            self.tx.send(Action::Quit)?;
          }

          event::KeyCode::Char('i') => {
            self.tx.send(Action::Enter(Mode::Search))?;
          }
          event::KeyCode::Char('s') => {
            if cx.channel.is_some() {
              self.tx.send(Action::Enter(Mode::Results))?;
            };
          }
          event::KeyCode::Char('l') => {
            self.tx.send(Action::Enter(Mode::Downloads(Box::new(
              cx.mode.clone(),
            ))))?;
          }
          event::KeyCode::Char(' ') => {
            if cx.selected_item.is_some() {
              self.tx.send(Action::Enter(Mode::Selected))?;
            };
          }

          event::KeyCode::Char(',') => {
            self.tx.send(Action::PrevVo)?;
          }
          event::KeyCode::Char('.') => {
            self.tx.send(Action::NextVo)?;
          }

          // test only
          event::KeyCode::Char('p') => {
            let (stop_tx, stop_rx) = oneshot::channel();
            if let Some(stop_tx) = self.player_stop.take() {
              stop_tx.send(()).unwrap();
            };
            self.player_stop = Some(stop_tx);

            let player = Player::new(
              Vo::Ansi,
              "http://127.0.0.1:3030/torrents/0/stream/0".into(),
              self.executor.clone(),
              cx.width,
              cx.height,
              stop_rx,
            );

            self.tx.send(Action::Enter(Mode::Player))?;
            self.executor.spawn(async move {
              player.play().await?;

              anyhow::Ok(())
            });
          }

          _ => {}
        }
      }

      Mode::Player => {
        if let event::KeyEvent {
          code: event::KeyCode::Char('q'),
          modifiers: _,
          kind: event::KeyEventKind::Press,
          state: _,
        } = key
        {
          if let Some(player_stop) = self.player_stop.take() {
            player_stop.send(()).unwrap();
          };
          self.tx.send(Action::Enter(Mode::Normal))?;
        };
      }
      _ => {}
    };

    Ok(())
  }
  fn handle_event(
    &mut self,
    event: Arc<event::Event>,
    cx: &mut AppCtxt,
  ) -> anyhow::Result<()> {
    match *event {
      event::Event::Key(key) => {
        self.handle_key_event(key, cx)?;
      }
      event::Event::Mouse(..) => {}
      _ => {}
    };
    Ok(())
  }
  pub async fn run(&mut self) -> anyhow::Result<()> {
    // panic!("{:?}", crossterm::terminal::size());
    let mut cx = AppCtxt::default();
    self.init(&mut cx).await?;

    let mut tick = tokio::time::interval(tokio::time::Duration::from_secs_f32(
      1.0 / tick_rate(),
    ));
    let mut render_tick = tokio::time::interval(
      tokio::time::Duration::from_secs_f32(1.0 / frame_rate()),
    );
    let mut event_reader = EventStream::new();

    while !self.quit.load(atomic::Ordering::Relaxed) {
      tokio::select! {
        Some(Ok(event)) = event_reader.next() => {
          let event = Arc::new(event);
          self.handle_event(event.clone(), &mut cx)?;
          self.components.dispatch_event(event, self.tx.clone(), &cx)?;
        }
        Some(action) = self.rx.recv() => {
          self.execute_action(action.clone(), &mut cx)?;
          self.components.dispatch_action(action, &cx)?;
        }

        _ = tick.tick() => {
          self.components.tick()?;
        }
        _ = render_tick.tick() => {
          if !matches!(cx.mode, Mode::Player) {
            self.render(&mut cx)?;
          };
        }
      }
    }

    self.cleanup()?;
    dbg!(&cx);
    Ok(())
  }
  fn render(&mut self, cx: &mut AppCtxt) -> anyhow::Result<()> {
    let layout = Layout::vertical([Constraint::Length(3), Constraint::Fill(1)]);
    let components = &mut self.components;

    self.term.draw(|frame| {
      let layout = layout.split(frame.area());
      components.search.draw(frame, layout[0], cx);
      components.downloaded_items.draw(frame, layout[1], cx);

      let layout = Layout::horizontal([
        Constraint::Fill(3),
        if cx.selected_item.is_some() {
          Constraint::Fill(1)
        } else {
          Constraint::Max(0)
        },
      ])
      .split(layout[1]);
      components.items.draw(frame, layout[0], cx);
      if cx.selected_item.is_some() {
        components.selected_item.draw(frame, layout[1], cx);
      };

      components.popup.draw(frame, frame.area(), cx);
    })?;
    Ok(())
  }
}
impl Default for App {
  fn default() -> Self {
    Self::new(Arc::new(Handle::current()), frame_rate(), tick_rate())
  }
}

pub fn retsore_term() {
  ratatui::restore();
}
