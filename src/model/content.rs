#[derive(Debug, PartialEq)]
pub enum Version {
    V3,
    V5,
    V6,
}

impl TryFrom<char> for Version {
    type Error = ();

    fn try_from(value: char) -> Result<Self, Self::Error> {
        match value {
            '3' => Ok(Version::V3),
            '5' => Ok(Version::V5),
            '6' => Ok(Version::V6),
            _ => Err(()),
        }
    }
}

#[derive(Debug)]
pub struct Notebook {
    pub id: String,
    pub pages: Vec<Page>,
}

#[derive(Debug)]
pub struct Page {
    pub id: String,
    pub version: Version,
    pub layers: Vec<Layer>,
}

#[derive(Debug)]
pub struct Layer {
    pub lines: Vec<Line>,
}

#[derive(Debug)]
pub enum Color {
    Black,
    Grey,
    White,
    Yellow,
    Green,
    Pink,
    Blue,
    Red,
    GreyOverlap,
}

impl TryFrom<u32> for Color {
    type Error = ();

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Color::Black),
            1 => Ok(Color::Grey),
            2 => Ok(Color::White),
            3 => Ok(Color::Yellow),
            4 => Ok(Color::Green),
            5 => Ok(Color::Pink),
            6 => Ok(Color::Blue),
            7 => Ok(Color::Red),
            8 => Ok(Color::GreyOverlap),
            _ => Err(()),
        }
    }
}

#[derive(Debug)]
pub enum BrushType {
    Eraser,
    EraserArea,
    Marker,
    Fineliner,
    Paintbrush,
    MechanicalPencil,
    Pencil,
    Ballpoint,
    Highlighter,
    Calligraphy,
}

impl TryFrom<u32> for BrushType {
    type Error = ();

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0x06 => Ok(Self::Eraser),
            0x08 => Ok(Self::EraserArea),
            0x03 | 0x10 => Ok(Self::Marker),
            0x04 | 0x11 => Ok(Self::Fineliner),
            0x00 | 0x0C => Ok(Self::Paintbrush),
            0x07 | 0x0D => Ok(Self::MechanicalPencil),
            0x01 | 0x0E => Ok(Self::Pencil),
            0x02 | 0x0F => Ok(Self::Ballpoint),
            0x05 | 0x12 => Ok(Self::Highlighter),
            0x15 => Ok(Self::Calligraphy),
            _ => Err(()),
        }
    }
}

#[derive(Debug)]
pub struct Line {
    pub brush_type: BrushType,
    pub color: Color,
    #[allow(unused)]
    pub brush_size: f32,
    pub points: Vec<Point>,
}

#[derive(Debug)]
pub struct Point {
    pub x: f32,
    pub y: f32,
    #[allow(unused)]
    pub speed: f32,
    #[allow(unused)]
    pub direction: f32,
    pub width: f32,
    #[allow(unused)]
    pub pressure: f32,
}
