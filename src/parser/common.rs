use crate::model::content::Version;
use nom::{
    bytes::complete::tag,
    character::complete::anychar,
    error::{ErrorKind, ParseError, VerboseError},
    sequence::tuple,
    Err, IResult,
};

pub type ParserAtom<'a> = &'a [u8];
pub type ParserInput<'a> = ParserAtom<'a>;
pub type ParserError<'a> = VerboseError<ParserAtom<'a>>;
pub type ParserResult<'a, T> = IResult<ParserAtom<'a>, T, ParserError<'a>>;

pub fn error(s: ParserAtom, k: ErrorKind) -> Err<ParserError> {
    Err::Error(VerboseError::from_error_kind(s, k))
}

pub fn u8(s: ParserInput) -> ParserResult<u8> {
    nom::number::complete::u8(s)
}

pub fn u16(s: ParserInput) -> ParserResult<u16> {
    nom::number::complete::u16(nom::number::Endianness::Little)(s)
}

pub fn u32(s: ParserInput) -> ParserResult<u32> {
    nom::number::complete::u32(nom::number::Endianness::Little)(s)
}

pub fn f32(s: ParserInput) -> ParserResult<f32> {
    nom::number::complete::f32(nom::number::Endianness::Little)(s)
}

pub fn f64(s: ParserInput) -> ParserResult<f64> {
    nom::number::complete::f64(nom::number::Endianness::Little)(s)
}

fn header_prelude(s: ParserInput) -> ParserResult<()> {
    tag("reMarkable .lines file, version=")(s).map(|(rem, _)| (rem, ()))
}

fn header_version(s: ParserInput) -> ParserResult<Version> {
    let (remainder, version) = anychar(s)?;
    let version: Version = version
        .try_into()
        .map_err(|_| error(s, nom::error::ErrorKind::NoneOf))?;

    Ok((remainder, version))
}

fn header_padding(s: ParserInput) -> ParserResult<()> {
    tag("          ")(s).map(|(rem, _)| (rem, ()))
}

pub fn header(s: ParserInput) -> ParserResult<Version> {
    let (remainder, (_prelude, version, _padding)) =
        tuple((header_prelude, header_version, header_padding))(s)?;
    Ok((remainder, version))
}
