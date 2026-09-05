// https://en.wikipedia.org/wiki/ANSI_escape_code#8-bit

pub enum Colour {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
}

impl Colour {
    // Returns the ANSI foreground (text) color code number
    pub const fn fg_code(self) -> &'static str {
        match self {
            Colour::Black => "30",
            Colour::Red => "31",
            Colour::Green => "32",
            Colour::Yellow => "33",
            Colour::Blue => "34",
            Colour::Magenta => "35",
            Colour::Cyan => "36",
            Colour::White => "37",
        }
    }

    pub const fn bg_code(self) -> &'static str {
        match self {
            Colour::Black => "40",
            Colour::Red => "41",
            Colour::Green => "42",
            Colour::Yellow => "43",
            Colour::Blue => "44",
            Colour::Magenta => "45",
            Colour::Cyan => "46",
            Colour::White => "47",
        }
    }
}
