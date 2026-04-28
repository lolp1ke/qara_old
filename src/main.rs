use std::fs::OpenOptions;

use qara_app::App;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
  let default_hook = std::panic::take_hook();
  std::panic::set_hook(Box::new(move |info| {
    qara_app::retsore_term();

    default_hook(info);
    std::process::exit(1);
  }));
  ctrlc::set_handler(|| {
    qara_app::retsore_term();
    std::process::exit(1);
  })
  .expect("failed to set interrupt hook");
  tokio::spawn(async {
    let mut sighup =
      tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup())
        .unwrap();
    let mut sigterm =
      tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .unwrap();

    tokio::select! {
      _ = sighup.recv() => {}
      _ = sigterm.recv() => {}
      _ = tokio::signal::ctrl_c() => {}
    }

    qara_app::retsore_term();
    std::process::exit(0);
  });
  init_logging()?;

  let mut app = App::default();
  app.run().await?;
  dbg!(&app);

  Ok(())
}

fn init_logging() -> anyhow::Result<()> {
  let file = OpenOptions::new()
    .create(true)
    .write(true)
    .truncate(true)
    .open("latest.log")?;
  tracing_subscriber::fmt().with_writer(file).init();

  Ok(())
}
