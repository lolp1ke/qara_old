use qara_app::App;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
  let default_hook = std::panic::take_hook();
  std::panic::set_hook(Box::new(move |info| {
    qara_app::retsore_term();

    default_hook(info)
  }));
  ctrlc::set_handler(|| {
    qara_app::retsore_term();
  })
  .expect("failed to set interrupt hook");

  let mut app = App::default();
  app.run().await?;
  dbg!(&app);

  Ok(())
}
