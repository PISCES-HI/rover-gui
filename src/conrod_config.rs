use conrod;
use piston_window;

pub type Backend = (<piston_window::G2d<'static> as conrod::Graphics>::Texture, piston_window::Glyphs);
pub type Ui = conrod::Ui<Backend>;
pub type UiCell<'a> = conrod::UiCell<'a, Backend>;
