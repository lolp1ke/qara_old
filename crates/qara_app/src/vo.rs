use core::fmt;

#[derive(Debug)]
#[derive(Clone, Copy)]
pub(crate) enum Vo {
  Ascii,
  Mpv,
}
impl fmt::Display for Vo {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Ascii => f.write_str("ascii"),
      Self::Mpv => f.write_str("mpv"),
    }
  }
}
