# Glimbot

Glimbot is a server moderation and anti-spam bot for Discord, with plans to add anti-raid features in the future.

[![Rust Cargo Check](https://github.com/nisamson/glimbot/actions/workflows/cargocheck.yml/badge.svg?branch=main)](https://github.com/nisamson/glimbot/actions/workflows/cargocheck.yml) 
[![SL Scan](https://github.com/nisamson/glimbot/actions/workflows/shiftleft-analysis.yml/badge.svg)](https://github.com/nisamson/glimbot/actions/workflows/shiftleft-analysis.yml)
![License Info](https://img.shields.io/github/license/glimbot/glimbot)

# Supported Platforms

See [the support document](SECURITY.md) for information on what support will exist.

At the time of writing, MSRV is 1.51, targeting `stable-x86_64-unknown-linux-gnu` on Ubuntu 20.04 LTS. The database layer uses PostgreSQL 13, which can be hosted locally or remotely. **This is the only supported configuration**.

At one point during development of v0.3.0, I was able to compile it for `nightly-x86_64-unknown-linux-musl`. It may compile for other platforms.

# Installation

## From Source

Assuming you have a relevant version of Rust installed, you can download and build Glimbot from source fairly easily.

```bash
$ git clone https://github.com/glimbot/glimbot
$ cd glimbot
$ cargo build --release
```

Running the command `cargo run --release -- help` will provide information on how to get Glimbot up and running from this configuration.

## From Prebuilt Packaging

TBA
  
# Commands

This section provides brief overviews of what commands are generally available for users of the Discord bot.
Run `!info <command>` or `!<command> help` for more information on how to use a command.

## Basic

### `!info`
This command provides information on available commands, including any not documented here, and can be used to 
get more information on each command.

### `!config`
This command can be used by guild owners and moderators to configure glimbot. Descriptions of available config values are available via
`!config info <config_value>`, as well as [in this document](#configuration).

## Server Moderation

Glimbot offers the `!mod`, `!mod-role`, `!spam` and `!role` commands for server administration.

### `!mod`
The `!mod` command allows users with the role [`privileged_role`](#privileged_role) to kick/ban/warn/etc users.
Bans and mutes can be set to auto-expire. Actions performed with this command will be logged in [`mod_log_channel`](#mod_log_channel)

### `!mod-role`
This command allows users with the role [`privileged_role`](#privileged_role) to assign roles to
and unassign roles to users. It also allows roles to be set as user-joinable/leavable, allowing users to assign themselves roles.
Currently, the maximum number of roles a guild may make joinable is 128.

### `!spam`
This command allows users with the [`privileged_role`](#privileged_role) to clear messages in a channel and/or from a user, up to the last
4096 messages Glimbot saw in the guild. It also allows setting/resetting user [pressure](#anti-spam).

### `!role`
This command allows users to join and leave roles that moderators have made joinable. Currently, this is the only command
non-moderators will find useful outside of [`!info`](#info)

# Configuration

Below are the various configuration options which can be set with the `!config` command.

## Basic Configuration

### `command_prefix`
The prefix for commands. Commands are parsed from messages like `<command_prefix><command> <args...>`

By default, this is `!`, but may be set to any single character representable in a Rust `char`, i.e. any Unicode code point.

### `privileged_role`
The role which should be able to run sensitive commands, i.e. banning users, setting roles, and, critically, configuring Glimbot.

### `mod_log_channel` 
The channel where Glimbot should log moderation actions taken. This channel should be fine for Glimbot to write to frequently,
so consider making a dedicated channel for it.

### `mute_role`
A role which should be assigned to users when `!mod mute` is used or when a user triggers the anti-spam. See [this page](https://discordhelp.net/mute-user)
for more information on how to set up this role.

## Spam Configuration

See [anti-spam](#anti-spam) for more information on how the spam module works.

### `spam_ignore_role`
A role which should be ignored when determining whether or not to take action against a user for spamming.

### `spam_config`
A JSON object representing the various config values for calculating spam pressure.

An example config would be:
```json
{
  "base_pressure": 10.0,
  "image_pressure": 8.333333333333334,
  "length_pressure": 0.00625,
  "line_pressure": 0.7142857142857143,
  "max_pressure": 60.0,
  "ping_pressure": 2.5,
  "pressure_decay": 2.5,
  "silence_timeout": "10m"
}
```

When you're making changes to this, you can pass the new config value to the `!config` command like this:

```
!config set spam_config '{
  "base_pressure": 10.0,
  "image_pressure": 8.333333333333334,
  "length_pressure": 0.00625,
  "line_pressure": 0.7142857142857143,
  "max_pressure": 60.0,
  "ping_pressure": 2.5,
  "pressure_decay": 2.5,
  "silence_timeout": "10m"
}'
```

Glimbot's argument parsing will correctly interpret the JSON object and set the config appropriately.

#### Keys and their meanings

`base_pressure`: The pressure each message gets.

`image_pressure`: The pressure each image in a message generates.

`length_pressure`: The pressure added to a message for each UTF-8 code point it contains (~the number of bytes in the message.)

`line_pressure`: The pressure generated by vertical whitespace; any vertical whitespace is counted as a line.

`max_pressure`: The maximum pressure a user can have before Glimbot takes action.

`ping_pressure`: The amount of pressure generated by a single ping in a message.
Note that pings are counted by unique occurance in a message; pinging the same user over and over is only counted as a single ping.
Such a message will get dinged on message length, however.

`pressure_decay`: The amount of time, in seconds, for `base_pressure` to decay.

`silence_timeout`: The duration an automatic mute should last. Glimbot uses the [`humantime` parse function](https://docs.rs/humantime/2.1.0/humantime/fn.parse_duration.html)
to parse times. In short, you can specify durations as "10m" or "5h", etc.

# Design

## Goals

- Privacy: Glimbot does not persist any information linked directly to users.
  Message IDs (but not the messages) are stored in RAM in a cache for anti-spam purposes, but this cache is cleared regularly.
- Security: Glimbot aims to reduce opportunities for privilege escalation. Glimbot carefully checks user privileges before
  executing commands, and also avoids duplicating functionality available in the Discord client, reducing potential attack vectors.
  - While I promise not to do anything naughty with the Glimbot source code, and I've made it so that a bot owner has no more privileges in Glimbot than they would have otherwise as a member of a server, make sure you trust whoever is running your deployment of Glimbot.
    They ultimately have the power to do whatever they want with the permissions assigned to their bot in your guild.
- Performance: Glimbot is designed to be able to process many thousands of messages per second, and will scale to as many cores as are available on its host.
  This allows Glimbot to process messages at the rates a guild might see during a raid without requiring expensive hosting.

## Anti-Spam

Glimbot uses a pressure-based model ~~stolen~~ copied from [SweetieBot](https://sweetiebot.io), upon which Glimbot is loosely based.

Erik McClure, creator of SweetieBot, did a great job explaining how that system works [here](https://erikmcclure.com/blog/pressure-based-anti-spam-for-discord-bots/).
As of v0.3.1, this system is only partial implemented, with anti-raid and new user features not yet implemented.
They are in the works for Glimbot v1.0.