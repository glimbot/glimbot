use nom;
use super::{RollComponent, Roll};
use nom::IResult;
use nom::character::complete::*;
use nom::character::{is_digit, is_space};
use nom::error::{VerboseErrorKind, VerboseError, ParseError, make_error, context, ErrorKind};
use nom::character::complete::digit1;
use nom::combinator::*;
use std::num::ParseIntError;
use num::Integer;
use nom::bytes::complete::{take_while, tag};
use nom::Err::Failure;
use nom::branch::alt;
use nom::sequence::{delimited, preceded, tuple, separated_pair};
use nom::multi::{many0, fold_many1c, fold_many1, fold_many0};

type PResult<'a, 'b, T> = IResult<&'a str, T, VerboseError<&'b str>>;

fn sp(input: &str) -> PResult<&str> {
    take_while(move |c| c == ' ')(input)
}

fn number<T: Integer + std::str::FromStr>(input: &str) -> PResult<T> {
    map_res(
        digit1,
        |i| T::from_str(i),
    )(input)
}

fn parse_u8(input: &str) -> PResult<u8> {
    cut(context("u8 integer", number::<u8>))(input)
}

fn parse_u32(input: &str) -> PResult<u32> {
    cut(context("u32 integer", number::<u32>))(input)
}

pub fn die(input: &str) -> PResult<RollComponent> {
    map(
    separated_pair(parse_u32, tag("d"), parse_u32),
        |(n, d)| RollComponent::die(n, d))(input)
}

fn constant(input: &str) -> PResult<RollComponent> {
    cut(map(
        number,
        RollComponent::constant,
    ))(input)
}

fn atom(input: &str) -> PResult<Roll> {
    cut(map(
        alt((die, constant)),
        Roll::from,
    ))(input)
}

fn paren_roll(input: &str) -> PResult<Roll> {
    delimited(
        preceded(sp, tag("(")),
        roll,
        preceded(sp, tag(")")),
    )(input)
}

fn roll_front(input: &str) -> PResult<Roll> {
    cut(alt((paren_roll, atom)))(input)
}

fn op(input: &str) -> PResult<char> {
    context("op",
        alt((value('+', tag("+")),
         value('-', tag("-")))))(input)
}

fn roll_tail(input: &str) -> PResult<(char, Roll)> {
    tuple((preceded(sp, op), preceded(sp, roll_front)))(input)
}

pub fn roll(input: &str) -> PResult<Roll> {
    let (i, first) = preceded(sp, roll_front)(input)?;
    fold_many0(delimited(sp, roll_tail, sp), first,
        |l, (op, r)| match op {
            '+' => Roll::add(l, r),
            '-' => Roll::sub(l, r),
            _ => unreachable!()
        }
    )(i)
}

pub fn top_level_roll(input: &str) -> PResult<Roll> {
    all_consuming(roll)(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nom::error::convert_error;

    #[test]
    fn simple_roll() {
        let inp = "10000d20";
        let (_, r) = top_level_roll(inp).unwrap();
        assert_eq!(r.num_dice(), 10000)
    }
}