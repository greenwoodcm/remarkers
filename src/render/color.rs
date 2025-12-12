use crate::model::content::Color as ModelColor;
use anyhow::{anyhow, Result};
use printpdf::{Color as PdfColor, Rgb};

pub const PDF_BLACK: PdfColor = to_pdf_color(ModelColor::Black);

// const fn to_pdf_color_unchecked(color: ModelColor) -> PdfColor {
//     match to_pdf_color(color) {
//         Ok(color) => color,
//         Err(e) => panic!(),
//     }
// }

pub const fn to_pdf_color(color: ModelColor) -> PdfColor {
    let (r, g, b) = match color {
        ModelColor::Black => (0.0, 0.0, 0.0),
        ModelColor::Grey => (0.5, 0.5, 0.5),
        ModelColor::White => panic!("unimplemented color White"),
        ModelColor::Yellow => (1.0, 1.0, 0.0),
        ModelColor::Green => (0.0, 1.0, 0.0),
        ModelColor::Pink => panic!("unimplemented color Pink"),
        ModelColor::Blue => (0.0, 0.0, 1.0),
        ModelColor::Red => (1.0, 0.0, 0.0),
        ModelColor::GreyOverlap => panic!("unimplemented color GreyOverlap"),
    };

    PdfColor::Rgb(Rgb {
        r,
        g,
        b,
        icc_profile: None,
    })
}
