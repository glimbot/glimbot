// Glimbot - A Discord anti-spam and administration bot.
// Copyright (C) 2020-2021 Nick Samson

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Contains constants describing some meta info about this binary.

/// Comma separated list of the authors.
pub const AUTHOR_NAME: &str = env!("CARGO_PKG_AUTHORS");
/// Short string with the license of the project.
pub const LICENSE: &str = env!("CARGO_PKG_LICENSE");
/// Short version of the glimbot copyright header.
pub const LICENSE_HEADER: &str = r#"Glimbot - A Discord anti-spam and administration bot.
Copyright (C) 2020 Nick Samson

This binary is subject to the terms of the Mozilla Public
License, v. 2.0. If a copy of the MPL was not distributed with this
binary, You can obtain one at http://mozilla.org/MPL/2.0/."#;
/// The version from the Cargo.toml used to compile this version of glimbot.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// The repository from which this source was compiled.
pub const REPO_URL: &str = env!("CARGO_PKG_REPOSITORY");
