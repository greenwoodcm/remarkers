use std::collections::HashMap;

use super::common::*;
use crate::model::{self, content::*};

use nom::{bytes::complete::take, multi::many1};
use tracing::{info, trace, warn};

fn varuint(s: ParserInput) -> ParserResult<u64> {
    let mut result: u64 = 0;
    let mut shift = 0;
    let mut s = s;

    loop {
        let (curr_stream, byte) = take(1usize)(s)?;

        assert_eq!(
            1,
            byte.len(),
            "expected to pop one byte but popped {}",
            byte.len()
        );
        let byte = byte[0] as u64;

        result |= (byte & 0x7F) << (shift * 7);

        let is_terminal = (byte & 0x80) == 0;
        if is_terminal {
            return Ok((curr_stream, result));
        }

        shift += 1;
        s = curr_stream;
    }
}

#[derive(Debug, PartialEq)]
enum TagType {
    Id,
    Length4,
    Byte8,
    Byte4,
    Byte1,
}

impl TryFrom<u64> for TagType {
    type Error = ();

    fn try_from(value: u64) -> std::result::Result<Self, Self::Error> {
        match value {
            0xF => Ok(Self::Id),
            0xC => Ok(Self::Length4),
            0x8 => Ok(Self::Byte8),
            0x4 => Ok(Self::Byte4),
            0x1 => Ok(Self::Byte1),
            _ => Err(()),
        }
    }
}

fn stream_tag(
    expected_index: u64,
    expected_tag_type: TagType,
) -> impl Fn(ParserInput) -> ParserResult<()> {
    move |s| {
        let (s, packed_val) = varuint(s)?;
        let index = packed_val >> 4;
        let tag_type: TagType = (packed_val & 0b1111).try_into().map_err(|_| {
            nom::Err::Error(nom::error::Error::new(s, nom::error::ErrorKind::NoneOf))
        })?;

        if index != expected_index {
            return Err(nom::Err::Error(nom::error::Error::new(
                s,
                nom::error::ErrorKind::NoneOf,
            )));
        }

        if tag_type != expected_tag_type {
            return Err(nom::Err::Error(nom::error::Error::new(
                s,
                nom::error::ErrorKind::NoneOf,
            )));
        }

        Ok((s, ()))
    }
}

fn tagged_u32(expected_index: u64) -> impl Fn(ParserInput) -> ParserResult<u32> {
    move |s| {
        let (s, _) = stream_tag(expected_index, TagType::Byte4)(s)?;
        u32(s)
    }
}

fn tagged_f32(expected_index: u64) -> impl Fn(ParserInput) -> ParserResult<f32> {
    move |s| {
        let (s, _) = stream_tag(expected_index, TagType::Byte4)(s)?;
        f32(s)
    }
}

fn tagged_f64(expected_index: u64) -> impl Fn(ParserInput) -> ParserResult<f64> {
    move |s| {
        let (s, _) = stream_tag(expected_index, TagType::Byte8)(s)?;
        f64(s)
    }
}

#[derive(Debug, Eq, Hash, PartialEq)]
struct CrdtId {
    part1: u8,
    part2: u64,
}

fn tagged_id(expected_index: u64) -> impl Fn(ParserInput) -> ParserResult<CrdtId> {
    move |s| {
        let (s, _) = stream_tag(expected_index, TagType::Id)(s)?;
        let (s, part1) = u8(s)?;
        let (s, part2) = varuint(s)?;
        Ok((s, CrdtId { part1, part2 }))
    }
}

struct LineItemBlock {
    parent_id: CrdtId,
    item_id: CrdtId,
    left_id: CrdtId,
    right_id: CrdtId,
    subblock: LineItemSubblock,
}

struct LineItemSubblock {
    brush_type: BrushType,
    color: Color,
    thickness_scale: f64,
    #[allow(unused)]
    starting_length: f32,
    points: Vec<Point>,
}

fn read_block_v6(s: ParserInput) -> ParserResult<Option<LineItemBlock>> {
    let (s, block_len) = u32(s)?;
    trace!("read block length: {block_len}");

    let (s, _unknown) = u8(s)?;
    let (s, min_version) = u8(s)?;
    let (s, current_version) = u8(s)?;
    let (s, block_type) = u8(s)?;
    trace!("block meta: {min_version}, {current_version}, {block_type}");

    let (b, s) = s.split_at(block_len as _);
    let (b, block) = match block_type {
        0x05 => {
            trace!("parsing block 5");

            // read top-level block content
            let (b, parent_id) = tagged_id(1)(b)?;
            let (b, item_id) = tagged_id(2)(b)?;
            let (b, left_id) = tagged_id(3)(b)?;
            let (b, right_id) = tagged_id(4)(b)?;
            let (b, deleted_len) = tagged_u32(5)(b)?;
            trace!("parsed block level meta: parent {parent_id:?}, item {item_id:?}, left {left_id:?}, right {right_id:?}, deleted len {deleted_len}");

            // read subblock content only if it leads with a tagged length
            let (b, subblock) = match stream_tag(6, TagType::Length4)(b) {
                Ok((_forward_slice, _)) => {
                    trace!("found subchunk");

                    let (b, _) = stream_tag(6, TagType::Length4)(b)?;
                    let (b, subblock_len) = u32(b)?;
                    trace!("subblock len {subblock_len}");

                    let (sb, b) = b.split_at(subblock_len as _);
                    let (sb, item_type) = u8(sb)?;
                    trace!("item type: {item_type}");

                    let (sb, subblock) = match item_type {
                        0x03 => {
                            trace!("reading line item");

                            // read tag values
                            let (sb, brush_type_id) = tagged_u32(1)(sb)?;
                            let brush_type: BrushType = brush_type_id.try_into().map_err(|_| {
                                nom::Err::Error(nom::error::Error::new(
                                    s,
                                    nom::error::ErrorKind::NoneOf,
                                ))
                            })?;
                            let (sb, color_id) = tagged_u32(2)(sb)?;
                            let color: Color = color_id.try_into().map_err(|_| {
                                nom::Err::Error(nom::error::Error::new(
                                    s,
                                    nom::error::ErrorKind::NoneOf,
                                ))
                            })?;
                            let (sb, thickness_scale) = tagged_f64(3)(sb)?;
                            let (sb, starting_length) = tagged_f32(4)(sb)?;

                            trace!("brush type: {brush_type:?}, color id: {color_id}, thickness scale: {thickness_scale}, starting len: {starting_length}");

                            // read another subblock for the point vector
                            let (sb, _) = stream_tag(5, TagType::Length4)(sb)?;
                            let (sb, subsubblock_len) = u32(sb)?;

                            trace!("subsubblock length: {}", subsubblock_len);

                            let point_size = match current_version {
                                1 => 0x18,
                                2 => 0x0E,
                                other => panic!("unrecognized version {other}"),
                            };

                            if subsubblock_len % point_size != 0 {
                                warn!("subsubblock is not evenly divisible into points");
                            }

                            let (ssb, sb) = sb.split_at(subsubblock_len as _);

                            let mut loop_ssb = ssb;
                            let mut points = Vec::new();
                            let point_count = subsubblock_len / point_size;
                            trace!("point count: {point_count}");
                            for _ in 0..point_count {
                                let (inner_ssb, x) = f32(loop_ssb)?;
                                let x = x + (model::WIDTH_PIXELS / 2) as f32;

                                let (inner_ssb, y) = f32(inner_ssb)?;
                                let (inner_ssb, speed, direction, width, pressure) =
                                    match current_version {
                                        1 => {
                                            let (inner_ssb, speed) = f32(inner_ssb)?;
                                            let (inner_ssb, width) = f32(inner_ssb)?;
                                            let (inner_ssb, direction) = f32(inner_ssb)?;
                                            let (inner_ssb, pressure) = f32(inner_ssb)?;
                                            (
                                                inner_ssb,
                                                speed * 4.0,
                                                255.0 * width / (std::f32::consts::PI * 2.0),
                                                direction * 4.0,
                                                pressure * 255.0,
                                            )
                                        }
                                        2 => {
                                            let (inner_ssb, speed) = u16(inner_ssb)?;
                                            let (inner_ssb, width) = u16(inner_ssb)?;
                                            let (inner_ssb, direction) = u8(inner_ssb)?;
                                            let (inner_ssb, pressure) = u8(inner_ssb)?;
                                            (
                                                inner_ssb,
                                                speed as f32,
                                                width as f32,
                                                direction as f32,
                                                pressure as f32,
                                            )
                                        }
                                        other => panic!("unrecognized version {other}"),
                                    };

                                // adjust width
                                let width = width * 2.0 / 255.0;
                                trace!(
                                    "point: {x}, {y}, {speed}, {direction}, {width}, {pressure}"
                                );
                                loop_ssb = inner_ssb;
                                points.push(Point {
                                    x,
                                    y,
                                    speed,
                                    direction,
                                    width,
                                    pressure,
                                });
                            }

                            (
                                sb,
                                Some(LineItemSubblock {
                                    brush_type,
                                    color,
                                    thickness_scale,
                                    starting_length,
                                    points,
                                }),
                            )
                        }
                        other => {
                            warn!("unrecognized item type: {other}");
                            (sb, None)
                        }
                    };

                    warn!("remaining subblock len: {}", sb.len());

                    (b, subblock)
                }
                Err(_) => {
                    info!("didn't find subchunk");
                    (b, None)
                }
            };

            let block = subblock.map(|subblock| LineItemBlock {
                parent_id,
                item_id,
                left_id,
                right_id,
                subblock,
            });
            (b, block)
        }
        0x09 => {
            trace!("parsing author IDs block");
            let (b, num_subblocks) = varuint(b)?;
            trace!("found {num_subblocks} subblocks");

            let mut outer_b = b;
            for _ in 0..num_subblocks {
                let (inner_b, _) = stream_tag(0, TagType::Length4)(outer_b)?;
                let (inner_b, subblock_len) = u32(inner_b)?;
                trace!("subblock len {subblock_len}");

                let (inner_sb, inner_b) = inner_b.split_at(subblock_len as _);
                let (inner_sb, _uuid_len) = varuint(inner_sb)?;

                let (inner_sb, uuid_bytes) = take(16usize)(inner_sb)?;
                let uuid = uuid::Uuid::from_slice_le(uuid_bytes).unwrap();
                trace!("uuid: {uuid}");

                let (inner_sb, author_id) = u16(inner_sb)?;
                trace!("author id: {author_id}");

                if !inner_sb.is_empty() {
                    warn!(
                        "subblock not empty, expected subblock length {}, remaining length {}",
                        subblock_len,
                        inner_sb.len(),
                    )
                }
                outer_b = inner_b;
            }
            (outer_b, None)
        }
        0x0A => {
            trace!("parsing page info block");
            let (b, loads_count) = tagged_u32(1)(b)?;
            let (b, merges_count) = tagged_u32(2)(b)?;
            let (b, text_chars_count) = tagged_u32(3)(b)?;
            let (b, text_lines_count) = tagged_u32(4)(b)?;
            trace!("loads: {loads_count}, merges: {merges_count}, text chars: {text_chars_count}, text lines: {text_lines_count}");

            let b = if b.is_empty() {
                b
            } else {
                let (inner_b, _) = tagged_u32(5)(b)?;
                inner_b
            };

            (b, None)
        }
        other => {
            warn!("unknown block type: {other}");
            (b, None)
        }
    };

    if b.is_empty() {
        info!("drained all block content");
    } else {
        warn!(
            "undrained block content, block length {block_len}, unread length {}",
            b.len()
        );
    }

    Ok((s, block))
}

pub fn read_page_v6(s: ParserInput) -> ParserResult<Page> {
    let (s, blocks) = many1(read_block_v6)(s)?;
    info!("blocks length: {}", blocks.len());
    info!("remaining buffer len: {}", s.len());

    let mut all_ids = HashMap::new();
    let mut lines = Vec::new();
    for b in blocks {
        if let Some(b) = b {
            for id in [b.parent_id, b.item_id, b.left_id, b.right_id] {
                if all_ids.contains_key(&id) {
                    let count = all_ids.get_mut(&id).unwrap();
                    *count += 1;
                } else {
                    all_ids.insert(id, 1u64);
                }
            }

            lines.push(Line {
                brush_type: b.subblock.brush_type,
                color: b.subblock.color,
                brush_size: b.subblock.thickness_scale as f32,
                points: b.subblock.points,
            });
        }
    }

    info!("found {} IDs: {all_ids:?}", all_ids.len());

    Ok((
        s,
        Page {
            layers: vec![Layer { lines }],
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(&[0x00], 0)]
    #[case(&[0x01], 1)]
    #[case(&[0x12], 18)]
    #[case(&[0x80, 0x01], 128)]
    #[case(&[0x81, 0x01], 129)]
    #[case(&[0xF0, 0x48], 9328)]
    #[case(&[0xFF, 0x55], 11007)]
    #[case(&[0x80, 0x80, 0x01], 16384)]
    #[case(&[0x80, 0xA6, 0x01], 21248)]
    #[case(&[0xC7, 0x96, 0x4D], 1264455)]
    fn test_varuint(#[case] bytes: &[u8], #[case] expected: u64) {
        let (_s, parsed) = varuint(bytes).unwrap();
        assert_eq!(parsed, expected);
    }
}
