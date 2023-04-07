use nom::multi::count;

use super::common::*;
use crate::model::content::*;

fn num_layers(s: ParserInput) -> ParserResult<u32> {
    u32(s)
}

fn layer(s: ParserInput) -> ParserResult<Layer> {
    let (s, num_lines) = u32(s)?;
    let (rem, lines) = count(line, num_lines as _)(s)?;
    Ok((rem, Layer { lines }))
}

fn line(s: ParserInput) -> ParserResult<Line> {
    let (s, brush_type) = u32(s)?;
    let brush_type: BrushType = brush_type
        .try_into()
        .map_err(|_| nom::Err::Error(nom::error::Error::new(s, nom::error::ErrorKind::NoneOf)))?;

    let (s, color) = u32(s)?;
    let color: Color = color
        .try_into()
        .map_err(|_| nom::Err::Error(nom::error::Error::new(s, nom::error::ErrorKind::NoneOf)))?;

    let (s, _padding) = u32(s)?;
    let (s, brush_size) = f32(s)?;
    // second padding only included for v5
    let (s, _padding) = u32(s)?;
    let (s, num_points) = u32(s)?;
    let (rem, points) = count(point, num_points as _)(s)?;

    Ok((
        rem,
        Line {
            brush_type,
            color,
            brush_size,
            points,
        },
    ))
}

fn point(s: ParserInput) -> ParserResult<Point> {
    let (s, x) = f32(s)?;
    let (s, y) = f32(s)?;
    let (s, speed) = f32(s)?;
    let (s, direction) = f32(s)?;
    let (s, width) = f32(s)?;
    let (s, pressure) = f32(s)?;

    Ok((
        s,
        Point {
            x,
            y,
            speed,
            direction,
            width,
            pressure,
        },
    ))
}

pub fn read_page_v5(s: ParserInput) -> ParserResult<Vec<Layer>> {
    let (s, num_layers) = num_layers(s)?;
    let (s, layers) = count(layer, num_layers as _)(s)?;
    Ok((s, layers))
}
