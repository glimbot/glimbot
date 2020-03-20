use super::{RollComponent, Roll};
use pest::Parser;
use pest_derive::Parser;
use crate::glimbot::modules::command::{Result as CmdRes, CommanderError};
use pest::iterators::Pair;
use regex::Regex;
use once_cell::sync::Lazy;
use crate::glimbot::util::FromError;

#[derive(Parser)]
#[grammar = "../resources/dice.pest"]
pub struct RollParser;

static DIE_RE: Lazy<Regex> = Lazy::new(
    || Regex::new(r#"(\d+)d(\d+)"#).unwrap()
);

pub fn die(i: impl AsRef<str>) -> CmdRes<RollComponent> {
    let i = i.as_ref();
    let caps = DIE_RE.captures(i).ok_or_else(
        || CommanderError::RuntimeError(format!("{} is not a valid die", i)))?;

    let num_dice: u32 = caps.get(1).unwrap().parse().map_err(CommanderError::from_error)?;
    let die_type: u32 = caps.get(2).unwrap().parse().map_err(CommanderError::from_error)?;

    Ok(RollComponent::die(num_dice, die_type))
}

pub fn parse_roll(input: impl AsRef<str>) -> CmdRes<Roll> {
    let roll = RollParser::parse(Rule::roll, input)
        .map_err(|e| CommanderError::BadCommandParse(e.to_string()))?
        .next().unwrap();


    parse_expr(roll.into_inner().next().unwrap().into_inner().next().unwrap())
}

fn parse_expr(input: Pair<Rule>) -> CmdRes<Roll> {
    match input.as_rule() {
        Rule::expr => {
            let mut inner = input.into_inner();
            let head = parse_head(inner.next().unwrap())?;
            inner.try_fold(head, |acc, tail| {
                let = parse_tail(tail)?;
                Ok(match op {
                    '-' => Roll::sub(acc, rhs),
                    '+' => Roll::add(acc, rhs),
                    _ => unreachable!()
                })
            })
        },
        _ => unreachable!()
    }
}

fn parse_head(input: Pair<Rule>) -> CmdRes<Roll> {

}

fn parse_tail(input: Pair<Rule>) -> CmdRes<(char, Roll)> {

}


#[cfg(test)]
mod tests {
    use super::*;
    use nom::error::convert_error;

    #[test]
    fn simple_roll() {
        let inp = "10000d20";
        let r = parse_roll(inp).unwrap();
        assert_eq!(r.num_dice(), 10000)
    }
}