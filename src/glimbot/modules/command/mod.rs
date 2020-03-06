use regex::{Regex, RegexBuilder};
use serenity::prelude::*;
use serenity::model::prelude::*;
use thiserror::Error as ThisErr;
use std::error::Error as StdErr;
use std::result::Result as StdRes;
use once_cell::sync::Lazy;
use std::str::FromStr;
use crate::glimbot::guilds::GuildContext;
use std::collections::HashSet;
use serenity::model::permissions::Permissions;
use std::fmt::Debug;


pub mod parser;

static ARG_RE: Lazy<Regex> = Lazy::new(
    || RegexBuilder::new(r#"
    ^. # command prefix, single character
    (?P<cmd>\w+) # command name
    (
    \s+
        (
            (\w+) # non-escaped stuff
            | ("((\\") | [^"])*") # escaped string
        )
    )*
    "#)
        .ignore_whitespace(true)
        .build()
        .unwrap()
);

#[derive(Debug)]
pub enum Arg {
    UInt(u64),
    Int(i64),
    Str(String),
}

impl std::fmt::Display for Arg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Arg::UInt(u) => write!(f, "{}", u),
            Arg::Int(i) => write!(f, "{}", i),
            Arg::Str(s) => write!(f, "{}", s),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ArgType {
    UInt,
    Int,
    Str,
}

impl std::fmt::Display for ArgType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}",
               match self {
                   ArgType::UInt => "u64",
                   ArgType::Int => "i64",
                   ArgType::Str => "str",
               }
        )
    }
}

#[derive(ThisErr, Debug)]
pub enum CommanderError {
    #[error("Glimmy doesn't have sufficient permissions to perform this action.")]
    InsufficientBotPerms,
    #[error("Glimmy ran into an issue with Discord.\n{0:?}\nT̵i̵m̵e̵ ̵t̵o̵ ̵b̵a̵n̵i̵s̵h̵.")]
    DiscordError(#[from] serenity::Error),
    #[error("Command parse failure: {0}")]
    BadCommandParse(String),
    #[error("Invalid parameter at index {0}: expected {1}")]
    BadParameter(usize, ArgType),
    #[error("Incorrect number of parameters. Got {0}")]
    IncorrectNumParams(usize),
    #[error("Could not parse arguments from {0}")]
    BadArgString(String),
    #[error("Glimmy's backend is having issues.")]
    Other,
}

pub type ActionFn = fn(&Commander, &GuildContext, &Context, &Message, &[Arg]) -> Result<()>;
pub type Result<T> = StdRes<T, CommanderError>;

/// The responsibility for controlling *who* can issue commands exists outside of this module.
#[derive(Clone)]
pub struct Commander {
    name: String,
    description: Option<String>,
    arg_names: Vec<String>,
    args: Vec<ArgType>,
    optional_args: Vec<ArgType>,
    action: ActionFn,
    required_perms: Permissions
}

impl std::fmt::Display for Commander {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.help_msg())
    }
}

impl Debug for Commander {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl Commander {
    pub fn new(name: impl Into<String>,
               description: Option<impl Into<String>>,
               arg_names: Vec<impl Into<String>>,
               args: Vec<ArgType>,
               optional_args: Vec<ArgType>,
               required_perms: Permissions,
               action: ActionFn) -> Self {
        if arg_names.len() != args.len() + optional_args.len() {
            panic!("arg_names must have exactly as many elements as the combined lengths of args and optional_args.")
        }

        Commander {
            name: name.into(),
            description: description.map(Into::into),
            arg_names: arg_names.into_iter().map(Into::into).collect(),
            args,
            optional_args,
            required_perms,
            action,
        }
    }

    pub fn invoke(&self, g: &GuildContext, ctx: &Context, msg: &Message, args: impl AsRef<[String]>) -> Result<()> {
        let parsed_args = self.parse_args(args.as_ref())?;
        (self.action)(self, g, ctx, msg, &parsed_args)
    }

    pub fn parse_args(&self, args: &[String]) -> Result<Vec<Arg>> {
        let ziter =
            if args.len() > self.arg_names.len() || args.len() < self.args.len() {
                Err(CommanderError::IncorrectNumParams(args.len()))
            } else {
                Ok(self.args.iter().chain(self.optional_args.iter()).zip(args.iter()))
            }?;

        let res: Vec<_> = ziter.map(|(t, r)| Self::parse_arg(r, *t))
            .collect();

        if let Some((i, _)) = res.iter().enumerate().find(|(_i, x)| x.is_none()) {
            Err(CommanderError::BadParameter(i,
                                             *self.args
                                                 .iter()
                                                 .chain(self.optional_args
                                                     .iter())
                                                 .nth(i)
                                                 .unwrap()))
        } else {
            Ok(res.into_iter().map(|x| x.unwrap()).collect())
        }
    }

    fn parse_arg(raw: &str, typ: ArgType) -> Option<Arg> {
        match typ {
            ArgType::UInt => u64::from_str(raw).map(Arg::UInt).ok(),
            ArgType::Int => i64::from_str(raw).map(Arg::Int).ok(),
            ArgType::Str => Some(Arg::Str(raw.to_owned())),
        }
    }

    pub fn help_msg(&self) -> String {
        let req_args = self.arg_names
            .iter()
            .zip(self.args.iter())
            .map(|(n, t)| format!("{}:{}", n, t));

        let opt_args = self.arg_names
            .iter()
            .skip(self.args.len())
            .zip(self.optional_args.iter())
            .map(|(n, t)| format!("[{}:{}]", n, t));

        let params: Vec<String> = req_args.chain(opt_args).collect();
        let param_str = params.join(" ");

        format!("{} {}{}",
                self.name,
                param_str,
                if let Some(desc) = &self.description {
                    format!("\n{}", desc)
                } else {
                    "".to_owned()
                }
        )
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> Option<&String> {
        self.description.as_ref()
    }

    pub fn arg_names(&self) -> &[String] {
        &self.arg_names
    }

    pub fn required_args(&self) -> &[ArgType] {
        &self.args
    }

    pub fn optional_args(&self) -> &[ArgType] {
        &self.optional_args
    }

    pub fn permissions(&self) -> Permissions {
        self.required_perms
    }

    pub fn action(&self) -> ActionFn {
        self.action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regex() {
        // Six arguments, command is "ping"
        let command_str = r#"!ping with arguments 1 2 3 "\"this is an escaped string\"""#;
        assert!(ARG_RE.is_match(command_str));
        ARG_RE.captures(command_str).iter().for_each(|x| {
            x.iter()
                .filter(|x| x.is_some())
                .map(|x| x.unwrap())
                .for_each(|x| println!("{}", x.as_str()))
        })
    }
}

