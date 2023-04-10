use std::collections::HashMap;

use super::common::*;
use crate::model::{self, content::*};

use nom::{
    bytes::complete::take,
    error::ErrorKind,
    multi::{count, many1},
};
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
    type Error = u64;

    fn try_from(value: u64) -> std::result::Result<Self, Self::Error> {
        match value {
            0xF => Ok(Self::Id),
            0xC => Ok(Self::Length4),
            0x8 => Ok(Self::Byte8),
            0x4 => Ok(Self::Byte4),
            0x1 => Ok(Self::Byte1),
            other => Err(other),
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
        let tag_type: TagType = (packed_val & 0b1111)
            .try_into()
            .map_err(|_| error(s, ErrorKind::NoneOf))?;

        trace!("comparing {expected_index}, {expected_tag_type:?} == {index}, {tag_type:?}");
        if index != expected_index {
            return Err(error(s, ErrorKind::NoneOf));
        }

        if tag_type != expected_tag_type {
            return Err(error(s, ErrorKind::NoneOf));
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

/// This combinator parses a segment of the stream that we know the length of.  In this case we want to align parsing
/// of the next segment correctly, even if the `parser` passed in fails to drain all of the length of this segment.
/// So in implementing this combinator we split off the known length of the segment, parse it, log if the parser failed
/// to drain the segment, and return the tail so that future parsing is correctly aligned.
fn fixed_length_segment<T>(
    len: u32,
    mut parser: impl FnMut(ParserInput) -> ParserResult<T>,
) -> impl FnMut(ParserInput) -> ParserResult<T> {
    move |s| {
        let (head, tail) = s.split_at(len as _);
        let (head, parsed) = parser(head)?;

        if head.is_empty() {
            trace!("succeeded draining fixed length segment of length {len}");
        } else {
            warn!(
                "failed to drain fixed length segment of length {len}, remaining length {}",
                head.len()
            );
        }

        Ok((tail, parsed))
    }
}

#[derive(Debug, PartialEq)]
enum BlockType {
    SceneItem,
    AuthorInfo,
    PageInfo,
}

impl TryFrom<u8> for BlockType {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x05 => Ok(BlockType::SceneItem),
            0x09 => Ok(BlockType::AuthorInfo),
            0x0A => Ok(BlockType::PageInfo),
            _ => Err(()),
        }
    }
}

#[derive(Debug, PartialEq)]
enum ItemType {
    Line,
}

impl TryFrom<u8> for ItemType {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x03 => Ok(ItemType::Line),
            _ => Err(()),
        }
    }
}

struct LineItemSubblock {
    brush_type: BrushType,
    color: Color,
    thickness_scale: f64,
    #[allow(unused)]
    starting_length: f32,
    points: Vec<Point>,
}

struct Block<T> {
    parent_id: CrdtId,
    item_id: CrdtId,
    left_id: CrdtId,
    right_id: CrdtId,
    subblock: T,
}

type LineItemBlock = Block<LineItemSubblock>;

fn point(version: u8) -> impl Fn(ParserInput) -> ParserResult<Point> {
    move |s| {
        let (s, x) = f32(s)?;
        let x = x + (model::WIDTH_PIXELS / 2) as f32;

        let (s, y) = f32(s)?;
        let (s, speed, direction, width, pressure) = match version {
            1 => {
                let (s, speed) = f32(s)?;
                let (s, width) = f32(s)?;
                let (s, direction) = f32(s)?;
                let (s, pressure) = f32(s)?;
                (
                    s,
                    speed * 4.0,
                    255.0 * width / (std::f32::consts::PI * 2.0),
                    direction * 4.0,
                    pressure * 255.0,
                )
            }
            2 => {
                let (s, speed) = u16(s)?;
                let (s, width) = u16(s)?;
                let (s, direction) = u8(s)?;
                let (s, pressure) = u8(s)?;
                (
                    s,
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
        trace!("point: {x}, {y}, {speed}, {direction}, {width}, {pressure}");
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
}

fn line_item_subblock(version: u8) -> impl Fn(ParserInput) -> ParserResult<LineItemSubblock> {
    move |s| {
        // read tag values
        let (s, brush_type_id) = tagged_u32(1)(s)?;
        let brush_type: BrushType = brush_type_id
            .try_into()
            .map_err(|_| error(s, ErrorKind::NoneOf))?;
        let (s, color_id) = tagged_u32(2)(s)?;
        let color: Color = color_id
            .try_into()
            .map_err(|_| error(s, ErrorKind::NoneOf))?;
        let (s, thickness_scale) = tagged_f64(3)(s)?;
        let (s, starting_length) = tagged_f32(4)(s)?;

        trace!("brush type: {brush_type:?}, color id: {color_id}, thickness scale: {thickness_scale}, starting len: {starting_length}");

        // read another subblock for the point vector
        let (s, _) = stream_tag(5, TagType::Length4)(s)?;
        let (s, subsubblock_len) = u32(s)?;

        trace!("subsubblock length: {}", subsubblock_len);

        let point_size = match version {
            1 => 0x18,
            2 => 0x0E,
            other => panic!("unrecognized version {other}"),
        };

        if subsubblock_len % point_size != 0 {
            warn!("subsubblock is not evenly divisible into points");
        }

        let point_count = subsubblock_len / point_size;
        trace!("point count: {point_count}");

        let (s, points) = fixed_length_segment(subsubblock_len, |s| {
            count(point(version), point_count as _)(s)
        })(s)?;

        Ok((
            s,
            LineItemSubblock {
                brush_type,
                color,
                thickness_scale,
                starting_length,
                points,
            },
        ))
    }
}

fn scene_item_block<T, S>(
    expected_item_type: ItemType,
    subblock_parser: T,
) -> impl Fn(ParserInput) -> ParserResult<Option<Block<S>>>
where
    T: Fn(ParserInput) -> ParserResult<S>,
{
    move |s| {
        let (s, parent_id) = tagged_id(1)(s)?;
        let (s, item_id) = tagged_id(2)(s)?;
        let (s, left_id) = tagged_id(3)(s)?;
        let (s, right_id) = tagged_id(4)(s)?;
        let (s, deleted_len) = tagged_u32(5)(s)?;
        trace!("parsed block level meta: parent {parent_id:?}, item {item_id:?}, left {left_id:?}, right {right_id:?}, deleted len {deleted_len}");

        // in some cases the block header is not followed by a corresponding subblock tag and length.
        // in these cases we just proceed and ignore the block.
        let (s, _) = match stream_tag(6, TagType::Length4)(s) {
            Ok(val) => val,
            Err(_) => return Ok((s, None)),
        };

        let (s, subblock_len) = u32(s)?;
        trace!("subblock len {subblock_len}");

        let (s, subblock) = fixed_length_segment(subblock_len, |s| {
            let (s, item_type) = u8(s)?;
            let item_type: Result<ItemType, _> = item_type.try_into();
            trace!("item type: {item_type:?}");

            if item_type.is_err() || item_type.unwrap() != expected_item_type {
                return Err(error(s, ErrorKind::NoneOf));
            }

            subblock_parser(s)
        })(s)?;

        Ok((
            s,
            Some(Block {
                parent_id,
                item_id,
                left_id,
                right_id,
                subblock,
            }),
        ))
    }
}

fn author_id_block(s: ParserInput) -> ParserResult<()> {
    let (s, _) = stream_tag(0, TagType::Length4)(s)?;
    let (s, subblock_len) = u32(s)?;
    trace!("subblock len {subblock_len}");

    let (s, _) = fixed_length_segment(subblock_len, |s| {
        let (s, _uuid_len) = varuint(s)?;

        let (s, uuid_bytes) = take(16usize)(s)?;
        let uuid = uuid::Uuid::from_slice_le(uuid_bytes).unwrap();
        trace!("uuid: {uuid}");

        let (s, author_id) = u16(s)?;
        trace!("author id: {author_id}");

        Ok((s, ()))
    })(s)?;

    Ok((s, ()))
}

fn author_ids_block(s: ParserInput) -> ParserResult<()> {
    trace!("parsing author IDs block");
    let (s, num_subblocks) = varuint(s)?;
    trace!("found {num_subblocks} subblocks");

    let (s, _author_ids) = count(author_id_block, num_subblocks as _)(s)?;
    Ok((s, ()))
}

fn page_info_block(s: ParserInput) -> ParserResult<()> {
    trace!("parsing page info block");
    let (s, loads_count) = tagged_u32(1)(s)?;
    let (s, merges_count) = tagged_u32(2)(s)?;
    let (s, text_chars_count) = tagged_u32(3)(s)?;
    let (s, text_lines_count) = tagged_u32(4)(s)?;
    trace!("loads: {loads_count}, merges: {merges_count}, text chars: {text_chars_count}, text lines: {text_lines_count}");

    let s = if s.is_empty() {
        s
    } else {
        let (s, _) = tagged_u32(5)(s)?;
        s
    };

    Ok((s, ()))
}

fn read_block_v6(s: ParserInput) -> ParserResult<Option<LineItemBlock>> {
    let (s, block_len) = u32(s)?;
    trace!("read block length: {block_len}");

    let (s, _unknown) = u8(s)?;
    let (s, min_version) = u8(s)?;
    let (s, current_version) = u8(s)?;
    let (s, block_type) = u8(s)?;
    let block_type: Result<BlockType, _> = block_type.try_into();
    trace!("block meta: {min_version}, {current_version}, {block_type:?}");

    let (s, block) = fixed_length_segment(block_len, |b| match block_type {
        Ok(BlockType::SceneItem) => {
            trace!("reading line item");
            let item_parser = line_item_subblock(current_version);
            let block_parser = scene_item_block(ItemType::Line, item_parser);

            let (sb, subblock) = block_parser(b)?;
            warn!("remaining subblock len: {}", sb.len());

            Ok((b, subblock))
        }
        Ok(BlockType::AuthorInfo) => {
            let (b, _) = author_ids_block(b)?;
            Ok((b, None))
        }
        Ok(BlockType::PageInfo) => {
            let (b, _) = page_info_block(b)?;
            Ok((b, None))
        }
        Err(_) => {
            warn!("unknown block type");
            Ok((b, None))
        }
    })(s)?;

    Ok((s, block))
}

pub fn read_page_v6(s: ParserInput) -> ParserResult<Vec<Layer>> {
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

    Ok((s, vec![Layer { lines }]))
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
