use alloc::sync::Arc;
use core::fmt;
use std::{io::Write, os::fd::FromRawFd, process::Stdio};
use wide::u8x32;

use rayon::{
  iter::{
    IndexedParallelIterator, IntoParallelRefMutIterator, ParallelIterator,
  },
  slice::ParallelSliceMut,
};
use tokio::{
  io::{AsyncReadExt as _, BufReader},
  process,
  runtime::Handle,
  sync::{mpsc, oneshot},
};

const ASCII_LUT: [u8; 256] = [
  32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32,
  32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 46, 46, 46, 46, 46, 46, 46, 46, 46,
  46, 46, 46, 46, 46, 46, 46, 46, 46, 46, 46, 46, 46, 46, 46, 46, 46, 46, 46,
  58, 58, 58, 58, 58, 58, 58, 58, 58, 58, 58, 58, 58, 58, 58, 58, 58, 58, 58,
  58, 58, 58, 58, 58, 58, 58, 58, 58, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45,
  45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45,
  61, 61, 61, 61, 61, 61, 61, 61, 61, 61, 61, 61, 61, 61, 61, 61, 61, 61, 61,
  61, 61, 61, 61, 61, 61, 61, 61, 61, 43, 43, 43, 43, 43, 43, 43, 43, 43, 43,
  43, 43, 43, 43, 43, 43, 43, 43, 43, 43, 43, 43, 43, 43, 43, 43, 43, 43, 42,
  42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42,
  42, 42, 42, 42, 42, 42, 42, 42, 42, 35, 35, 35, 35, 35, 35, 35, 35, 35, 35,
  35, 35, 35, 35, 35, 35, 35, 35, 35, 35, 35, 35, 35, 35, 35, 35, 35, 35, 37,
  37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37,
  37, 37, 37, 37, 37, 37, 37, 37, 64,
];

#[derive(Debug)]
pub struct Player {
  vo: Vo,
  url: Arc<str>,

  executor: Arc<Handle>,

  position: i64,
  width: u16,
  height: u16,

  mpv: Option<tokio::process::Child>,
  stop_rx: Option<oneshot::Receiver<()>>,
}
impl Player {
  pub fn new(
    vo: Vo,
    url: Arc<str>,
    executor: Arc<Handle>,
    width: u16,
    height: u16,
    stop_rx: oneshot::Receiver<()>,
  ) -> Self {
    Self {
      vo,
      url,
      executor,
      position: 0,
      width,
      height,
      mpv: None,
      stop_rx: Some(stop_rx),
    }
  }

  async fn get_fps(&self) -> anyhow::Result<f64> {
    let child = process::Command::new("ffprobe")
      .args([
        "-v",
        "error",
        "-select_streams",
        "v:0",
        "-show_entries",
        "stream=r_frame_rate",
        "-of",
        "default=noprint_wrappers=1:nokey=1",
        &*self.url,
      ])
      .output()
      .await?;

    let fps_ratio = String::from_utf8(child.stdout)?.trim().to_string();
    let mut parts = fps_ratio.split('/');
    let num = parts.next().unwrap().parse::<f64>()?;
    let den = parts.next().unwrap().parse::<f64>()?;

    Ok(num / den)
  }

  async fn start_mpv_audio(mut self) -> anyhow::Result<Self> {
    if let Some(mut mpv) = self.mpv.take() {
      mpv.kill().await?;
    }

    let child = process::Command::new("mpv")
      .args([
        "--no-video",
        "--no-terminal",
        &format!("--start={}", self.position),
        "--input-ipc-server=/tmp/qara-mpv-socket",
        &*self.url,
      ])
      .stdout(Stdio::null())
      .stderr(Stdio::null())
      .spawn()?;

    self.mpv = Some(child);
    Ok(self)
  }
  pub async fn play(self) -> anyhow::Result<()> {
    match self.vo {
      Vo::Ascii => {}
      Vo::Ansi => self.start_ansi().await?,
      Vo::Kitty => {}
      Vo::Mpv => self.start_mpv().await?,
    };

    Ok(())
  }
  async fn start_mpv(self) -> anyhow::Result<()> {
    let mpv_installed = process::Command::new("which")
      .arg("mpv")
      .stdin(Stdio::null())
      .stdout(Stdio::null())
      .stderr(Stdio::null())
      .status()
      .await?;
    if !mpv_installed.success() {
      return self.start_ansi().await;
    };

    process::Command::new("mpv")
      .stdout(std::process::Stdio::null())
      .stderr(std::process::Stdio::null())
      .args(["--fullscreen", &*self.url])
      .spawn()?;
    Ok(())
  }

  async fn start_ansi(mut self) -> anyhow::Result<()> {
    let term_width = self.width as usize;
    let term_height = self.height as usize;

    let ffmpeg_height = term_height * 2;
    let raw_frame_size = term_width * ffmpeg_height * 3;
    let worst_row = row_size(term_width);
    let url = self.url.clone();

    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(4);

    let stop_rx = self.stop_rx.take();
    let executor = self.executor.clone();

    let fps = self.get_fps().await?;

    let decode = executor.spawn(async move {
      let mut ffmpeg = process::Command::new("ffmpeg")
        .args([
          "-y",
          "-hwaccel",
          "videotoolbox",
          "-i",
          &*url,
          "-vf",
          &format!("scale={}:{},fps={}", term_width, ffmpeg_height, fps),
          "-f",
          "rawvideo",
          "-pix_fmt",
          "rgb24",
          "pipe:1",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;

      let ffmpeg_stdout = ffmpeg.stdout.take().unwrap();
      let mut reader =
        BufReader::with_capacity(raw_frame_size * 2, ffmpeg_stdout);
      let mut frame = vec![0u8; raw_frame_size];

      loop {
        match reader.read_exact(&mut frame).await {
          Ok(n) => {
            if n == 0 {
              break;
            };

            if tx.send(frame.clone()).await.is_err() {
              break;
            }
          }
          Err(_) => break,
        }
      }

      ffmpeg.kill().await?;
      anyhow::Ok(())
    });

    let render = executor.spawn(async move {
      #[allow(unsafe_code)]
      let stdout = unsafe { std::fs::File::from_raw_fd(1) };
      let mut stdout =
        std::io::BufWriter::with_capacity(4 * raw_frame_size, stdout);

      stdout.write_all(b"\x1b[?25l\x1b[2J")?;
      stdout.flush()?;

      let mut flat = vec![0u8; worst_row * term_height];
      let mut row_lens = vec![0usize; term_height];
      let mut out_buf = Vec::<u8>::with_capacity(worst_row * term_height);

      let mut prev_frame = vec![255u8; raw_frame_size];

      let t_first_frame = tokio::time::Instant::now();
      let mut frame = match rx.recv().await {
        Some(f) => f,
        None => return anyhow::Ok(()),
      };
      let te_first_frame = t_first_frame.elapsed();
      tracing::info!("frist frame {:>4}ms", te_first_frame.as_millis());

      let mut next_frame = tokio::time::Instant::now();

      loop {
        let t_render = tokio::time::Instant::now();
        flat
          .par_chunks_exact_mut(worst_row)
          .zip(row_lens.par_iter_mut())
          .enumerate()
          .for_each(|(y, (row_buf, row_len))| {
            let top_start = (y * 2) * term_width * 3;
            let bottom_start = (y * 2 + 1) * term_width * 3;
            let top = &frame[top_start..top_start + term_width * 3];
            let bottom = &frame[bottom_start..bottom_start + term_width * 3];
            let prev_top = &prev_frame[top_start..top_start + term_width * 3];
            let prev_bottom =
              &prev_frame[bottom_start..bottom_start + term_width * 3];

            if !row_has_changes(top, prev_top)
              && !row_has_changes(bottom, prev_bottom)
            {
              *row_len = 0;
              return;
            };

            let mut out = 0;

            row_buf[out..out + 2].copy_from_slice(b"\x1b[");
            out += 2;
            out += write_usize_decimal(&mut row_buf[out..], y + 1);
            row_buf[out] = b';';
            out += 1;
            row_buf[out] = b'1';
            out += 1;
            row_buf[out] = b'H';
            out += 1;

            let mut tmp = [0u8; PIXEL_SIZE];
            let mut skip_count = 0;
            let mut x = 0;
            while x < term_width {
              let i = x * 3;

              if x + 8 <= term_width {
                let mask =
                  changed_mask_8(top, prev_top, bottom, prev_bottom, x);
                if mask == 0 {
                  skip_count += 8;
                  x += 8;
                  continue;
                };

                for bit in 0..8 {
                  let px = x + bit as usize;
                  let i = px * 3;

                  if mask & (1 << bit) == 0 {
                    skip_count += 1;
                  } else {
                    if skip_count > 0 {
                      row_buf[out..out + 2].copy_from_slice(b"\x1b[");
                      out += 2;
                      out +=
                        write_usize_decimal(&mut row_buf[out..], skip_count);
                      row_buf[out] = b'C';
                      out += 1;
                      skip_count = 0;
                    }
                    let n = write_half_block(
                      &mut tmp,
                      top[i],
                      top[i + 1],
                      top[i + 2],
                      bottom[i],
                      bottom[i + 1],
                      bottom[i + 2],
                    );
                    row_buf[out..out + n].copy_from_slice(&tmp[..n]);
                    out += n;
                  }
                }

                x += 8;
                continue;
              };

              if top[i..i + 3] == prev_top[i..i + 3]
                && bottom[i..i + 3] == prev_bottom[i..i + 3]
              {
                skip_count += 1;
              } else {
                if skip_count > 0 {
                  row_buf[out..out + 2].copy_from_slice(b"\x1b[");
                  out += 2;
                  out += write_usize_decimal(&mut row_buf[out..], skip_count);
                  row_buf[out] = b'C';
                  out += 1;
                  skip_count = 0;
                };

                let n = write_half_block(
                  &mut tmp,
                  top[i],
                  top[i + 1],
                  top[i + 2],
                  bottom[i],
                  bottom[i + 1],
                  bottom[i + 2],
                );
                row_buf[out..out + n].copy_from_slice(&tmp[..n]);
                out += n;
              };
              x += 1;
            }

            row_buf[out..out + 4].copy_from_slice(RESET);
            out += 4;

            *row_len = out;
          });
        let te_render = t_render.elapsed();

        let t_build = tokio::time::Instant::now();
        out_buf.clear();
        for (chunk, &len) in flat.chunks_exact(worst_row).zip(row_lens.iter()) {
          if len > 0 {
            out_buf.extend_from_slice(&chunk[..len]);
          };
        }
        let te_build = t_build.elapsed();

        let t_write = tokio::time::Instant::now();
        stdout.write_all(&out_buf)?;
        let te_write = t_write.elapsed();

        core::mem::swap(&mut prev_frame, &mut frame);

        let t_0 = tokio::time::Instant::now();
        frame = match rx.recv().await {
          Some(f) => f,
          None => break,
        };
        let te_recv = t_0.elapsed();

        tracing::info!(
          "recv={:>4}μs render={:>4}μs build={:>2}μs write={:>4}ms buf={}kb",
          te_recv.as_micros(),
          te_render.as_micros(),
          te_build.as_micros(),
          te_write.as_millis(),
          out_buf.len() / 1024,
        );

        next_frame += tokio::time::Duration::from_secs_f64(1.0 / fps);
        let now = tokio::time::Instant::now();
        if next_frame > now {
          tokio::time::sleep(next_frame - now).await;
        } else {
          next_frame = now;
        };
      }

      stdout.write_all(b"\x1b[?25h\x1b[0m\x1b[2J\x1b[H")?;
      stdout.flush()?;

      anyhow::Ok(())
    });

    executor
      .spawn(async move {
        let this = self.start_mpv_audio().await?;

        stop_rx.unwrap().await?;
        if let Some(mut mpv) = this.mpv {
          mpv.kill().await?;
          decode.abort();
          render.abort();
        };

        anyhow::Ok(())
      })
      .await??;

    Ok(())
  }
}

#[derive(Debug)]
#[derive(Clone)]
#[derive(Default)]
pub enum Vo {
  Ascii,
  Ansi,
  Kitty,
  #[default]
  Mpv,
}
impl Vo {
  pub fn next(&self) -> Self {
    match self {
      Self::Ascii => Self::Ansi,
      Self::Ansi => Self::Kitty,
      Self::Kitty => Self::Mpv,
      Self::Mpv => Self::Ascii,
    }
  }
  pub fn prev(&self) -> Self {
    match self {
      Self::Ascii => Self::Mpv,
      Self::Ansi => Self::Ascii,
      Self::Kitty => Self::Ansi,
      Self::Mpv => Self::Kitty,
    }
  }
}
impl fmt::Display for Vo {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Ascii => f.write_str("ascii"),
      Self::Ansi => f.write_str("ansi"),
      Self::Kitty => f.write_str("kitty"),
      Self::Mpv => f.write_str("mpv"),
    }
  }
}

const HALF_BLOCK: &[u8] = "▀".as_bytes();
const PIXEL_SIZE: usize = 45;
const SKIP_SIZE: usize = 8;
const RESET: &[u8] = b"\x1b[0m";
const CURSOR_SIZE: usize = 10;
const fn row_size(width: usize) -> usize {
  CURSOR_SIZE + width * (PIXEL_SIZE + SKIP_SIZE) + RESET.len() + 1
}

#[inline(always)]
const fn write_usize_decimal(buf: &mut [u8], mut v: usize) -> usize {
  if v == 0 {
    buf[0] = b'0';
    return 1;
  }
  let mut tmp = [0u8; 8];
  let mut len = 0;
  while v > 0 {
    tmp[len] = b'0' + (v % 10) as u8;
    v /= 10;
    len += 1;
  }
  tmp[..len].reverse();
  buf[..len].copy_from_slice(&tmp[..len]);
  len
}
static DEC_LUT: &[([u8; 3], u8); 256] = &{
  let mut t = [([0u8; 3], 0u8); 256];
  let mut i = 0usize;
  while i < 256 {
    let v = i as u8;
    if v >= 100 {
      t[i] = ([b'0' + v / 100, b'0' + (v / 10) % 10, b'0' + v % 10], 3);
    } else if v >= 10 {
      t[i] = ([b'0' + v / 10, b'0' + v % 10, 0], 2);
    } else {
      t[i] = ([b'0' + v, 0, 0], 1);
    }
    i += 1;
  }
  t
};

#[inline(always)]
const fn write_u8_decimal(buf: &mut [u8], v: u8) -> usize {
  let (digits, len) = DEC_LUT[v as usize];
  let len = len as usize;
  buf[..len].copy_from_slice(&digits[..len]);
  len
}
#[inline(always)]
const fn write_half_block(
  buf: &mut [u8],
  r_top: u8,
  g_top: u8,
  b_top: u8,
  r_bot: u8,
  g_bot: u8,
  b_bot: u8,
) -> usize {
  let mut out = 0;

  buf[out..out + 7].copy_from_slice(b"\x1b[38;2;");
  out += 7;
  out += write_u8_decimal(&mut buf[out..], r_top);
  buf[out] = b';';
  out += 1;
  out += write_u8_decimal(&mut buf[out..], g_top);
  buf[out] = b';';
  out += 1;
  out += write_u8_decimal(&mut buf[out..], b_top);
  buf[out] = b'm';
  out += 1;

  buf[out..out + 7].copy_from_slice(b"\x1b[48;2;");
  out += 7;
  out += write_u8_decimal(&mut buf[out..], r_bot);
  buf[out] = b';';
  out += 1;
  out += write_u8_decimal(&mut buf[out..], g_bot);
  buf[out] = b';';
  out += 1;
  out += write_u8_decimal(&mut buf[out..], b_bot);
  buf[out] = b'm';
  out += 1;

  buf[out..out + 3].copy_from_slice(HALF_BLOCK);
  out += 3;

  buf[out..out + 4].copy_from_slice(RESET);
  out += 4;

  out
}

#[inline(always)]
fn row_has_changes(cur: &[u8], prev: &[u8]) -> bool {
  debug_assert_eq!(cur.len(), prev.len());
  let mut i = 0;
  let len = cur.len();

  while i + 32 <= len {
    let a = u8x32::from(&cur[i..i + 32]);
    let b = u8x32::from(&prev[i..i + 32]);
    if a != b {
      return true;
    };
    i += 32;
  }
  cur[i..] != prev[i..]
}

#[inline(always)]
fn changed_mask_8(
  top: &[u8],
  prev_top: &[u8],
  bottom: &[u8],
  prev_bottom: &[u8],
  x: usize,
) -> u8 {
  let i = x * 3;

  let mut top_cur = [0u8; 32];
  let mut top_prv = [0u8; 32];
  let mut bottom_cur = [0u8; 32];
  let mut bottom_prv = [0u8; 32];

  top_cur[..24].copy_from_slice(&top[i..i + 24]);
  top_prv[..24].copy_from_slice(&prev_top[i..i + 24]);
  bottom_cur[..24].copy_from_slice(&bottom[i..i + 24]);
  bottom_prv[..24].copy_from_slice(&prev_bottom[i..i + 24]);

  let top_diff: [u8; 32] = (u8x32::from(top_cur) ^ u8x32::from(top_prv)).into();
  let bot_diff: [u8; 32] =
    (u8x32::from(bottom_cur) ^ u8x32::from(bottom_prv)).into();

  let mut mask = 0u8;
  for px in 0..8 {
    let b = top_diff[px * 3]
      | top_diff[px * 3 + 1]
      | top_diff[px * 3 + 2]
      | bot_diff[px * 3]
      | bot_diff[px * 3 + 1]
      | bot_diff[px * 3 + 2];
    if b != 0 {
      mask |= 1 << px;
    }
  }
  mask
}
