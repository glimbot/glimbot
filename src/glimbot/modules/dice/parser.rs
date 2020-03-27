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

fn die(i: impl AsRef<str>) -> CmdRes<RollComponent> {
    let i = i.as_ref();
    let caps = DIE_RE.captures(i).ok_or_else(
        || CommanderError::RuntimeError(format!("{} is not a valid die", i)))?;

    let num_dice: u32 = caps.get(1).unwrap().as_str().parse().map_err(CommanderError::from_error)?;
    let die_type: u32 = caps.get(2).unwrap().as_str().parse().map_err(CommanderError::from_error)?;

    Ok(RollComponent::die(num_dice, die_type))
}

pub fn parse_roll(input: impl AsRef<str>) -> CmdRes<Roll> {
    let roll = RollParser::parse(Rule::roll, input.as_ref())
        .map_err(|e| CommanderError::BadCommandParse(e.to_string()))?
        .next().unwrap();


    parse_expr(roll.into_inner().next().unwrap())
}

fn parse_expr(input: Pair<Rule>) -> CmdRes<Roll> {
    match input.as_rule() {
        // This is actually very subtly wrong. Builds operations right-deep 1 + 2 + 3 -> 1 + (2 + 3)
        // Doesn't actually affect anything here, but would if this is ever changed in the future.
        Rule::expr => {
            let mut inner = input.into_inner();
            let head = parse_head(inner.next().unwrap())?;
            inner.try_fold(head, |acc, tail| {
                let (op, rhs) = parse_tail(tail)?;
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
    match input.as_rule() {
        Rule::expr => parse_expr(input),
        Rule::atom => {
            let s = input.as_str();
            if s.contains("d") {
                die(s).map(Roll::from)
            } else {
                let i = s.parse::<u32>();
                i.map_err(CommanderError::from_error).map(RollComponent::constant).map(Roll::from)
            }
        },
        _ => unreachable!()
    }
}

fn parse_tail(input: Pair<Rule>) -> CmdRes<(char, Roll)> {
    let mut inner = input.into_inner();
    let op = inner.next().unwrap();
    let op = op.as_str().chars().next().unwrap();
    let roll = parse_expr(inner.next().unwrap())?;
    Ok((op, roll))
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_roll() {
        let inp = "(10000d20) + 20 - 5d4";
        let r = parse_roll(inp).unwrap();
        println!("{:?}", &r);
        assert_eq!(r.num_dice(), 10005);
        println!("{}", r.eval())
    }

    #[test]
    fn test_fail() {
        let inp = "100 + 5d";
        let r = parse_roll(inp).unwrap_err();
        println!("{}", &r);
    }
}