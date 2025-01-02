use printpdf::{Color as PdfColor, Rgb};
use crate::model::content::Color as ModelColor;

pub const PDF_BLACK: PdfColor = to_pdf_color(ModelColor::Black);

pub const fn to_pdf_color(color: ModelColor) -> PdfColor {
    let (r, g, b) = match color {
        ModelColor::Black => (0.0, 0.0, 0.0),
        ModelColor::Grey => todo!(),
        ModelColor::White => todo!(),
        ModelColor::Yellow => todo!(),
        ModelColor::Green => (0.0, 1.0, 0.0),
        ModelColor::Pink => todo!(),
        ModelColor::Blue => (0.0, 0.0, 1.0),
        ModelColor::Red => (1.0, 0.0, 0.0),
        ModelColor::GreyOverlap => todo!(),
    };
    PdfColor::Rgb(Rgb { r, g, b, icc_profile: None })
}