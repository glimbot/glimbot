use pest::Parser;
use pest_derive::Parser;

use crate::glimbot::modules::command::CommanderError;

#[derive(Debug, Clone)]
pub struct RawCmd {
    pub prefix: String,
    pub command: String,
    pub args: Vec<String>,
}

#[derive(Parser)]
#[grammar = "../resources/command.pest"]
pub struct CommandParser;

pub fn parse_command(s: impl AsRef<str>) -> super::Result<RawCmd> {
    let cmd = CommandParser::parse(Rule::command, s.as_ref())
        .map_err(|e| CommanderError::BadCommandParse(e.to_string()))?
        .next().unwrap();

    let mut prefix = "";
    let mut command = "";
    let mut args = Vec::new();

    for component in cmd.into_inner() { // We're in Rule::command
        match component.as_rule() {
            Rule::prefixed_command => {
                let mut inner_rules = component.into_inner();
                prefix = inner_rules.next().unwrap().as_str();
                command = inner_rules.next().unwrap().as_str();
            }
            Rule::arg => {
                args.push(
                    unescape(component.as_str())
                );
            }
            _ => (),
        };
    };

    Ok(RawCmd { prefix: prefix.to_string(), command: command.to_string(), args })
}

fn unescape(s: impl AsRef<str>) -> String {
    let s = s.as_ref();
    if s.starts_with("\"") {
        let mut out = s.replace(r#"\""#, "\"");
        out.remove(0);
        out.pop();
        out
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_parse() {
        let cmd = CommandParser::parse(Rule::command, "!ping")
            .map_err(|e| CommanderError::BadCommandParse(e.to_string())).unwrap();

        println!("{:?}", cmd)
    }

    #[test]
    fn parse_with_args() {
        let command_str = r#"~pong with arguments 1 2 3 "\"this is an escaped string\"""#;
        let cmd = CommandParser::parse(Rule::command, command_str)
            .map_err(|e| CommanderError::BadCommandParse(e.to_string())).unwrap();

        println!("{:?}", cmd);

        let command = parse_command(command_str).unwrap();
        println!("{:?}", command);
        assert_eq!(command.prefix, "~");
        assert_eq!(command.command, "pong");
        assert_eq!(command.args.len(), 6);
    }

    #[test]
    fn test_unescape() {
        let s = r#""\"this is an escaped string\"""#;
        let unescaped = unescape(s);
        assert_eq!(unescaped, "\"this is an escaped string\"");
    }
}