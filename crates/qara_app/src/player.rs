use std::process::Stdio;

use anyhow::Context;
use tokio::{
  io::{AsyncReadExt, BufReader},
  process,
};

use crate::Vo;

pub(crate) async fn play(
  vo: Vo,
  url: &str,
  width: u16,
  height: u16,
) -> anyhow::Result<()> {
  match vo {
    Vo::Ascii => play_ascii(url, width, height).await?,
    Vo::Mpv => {
      process::Command::new("mpv")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .args(["--fullscreen", url])
        .spawn()?;
    }
  };

  Ok(())
}

async fn play_ascii(url: &str, width: u16, height: u16) -> anyhow::Result<()> {
  let mut current_ts = 0;

  loop {
    let mut ffmpeg = process::Command::new("ffmpeg")
      .args([
        "-y",
        "-hwaccel",
        "videotoolbox",
        "-ss",
        &current_ts.to_string(),
        "-i",
        url,
        "-f",
        "rawvideo",
        "-pix_fmt",
        "gray",
        "pipe:1",
      ])
      .stdout(Stdio::piped())
      .stderr(Stdio::inherit())
      .spawn()?;
    let stdout = ffmpeg.stdout.take().context("stdout?")?;

    let mut reader = BufReader::new(stdout);
    let mut buf = vec![0u8; width as usize * height as usize];
    let start = tokio::time::Instant::now();

    loop {
      tokio::select! {
        result = reader.read_exact(&mut buf) => match result {
          Ok(_) => {}
          Err(_) => break,
        }
      }
    }
  }
}
