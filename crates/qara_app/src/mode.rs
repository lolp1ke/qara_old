use core::fmt;

#[derive(Debug)]
#[derive(Default)]
#[derive(Clone)]
pub enum Mode {
  #[default]
  Normal,
  Search,
  Searching,
  Results,
  Selected,
  Downloads(Box<Self>),
  Popup(Box<Self>),
  Player,
}
impl fmt::Display for Mode {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Normal => f.write_str("normal"),
      Self::Search => f.write_str("search"),
      Self::Searching => f.write_str("searching your query..."),
      Self::Results => f.write_str("items list"),
      Self::Selected => f.write_str("selected item info"),
      Self::Downloads(prev_mode) => {
        f.write_str(&format!("{} -> downloads", prev_mode))
      }
      Self::Popup(prev_mode) => f.write_str(&format!("{} -> popup", prev_mode)),
      Self::Player => f.write_str("player"),
    }
  }
}
