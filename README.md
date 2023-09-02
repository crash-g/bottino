# Treasurer bot a.k.a. *bottino*

This Telegram bot can be used to track expenses in a group of people.

For a quick introduction see [here](QUICKSTART.md).

For detailed instructions on how to use it, see [here](INSTRUCTIONS.md).

## Build and run

Just use `cargo run`. The bot expects the `TELOXIDE_TOKEN` environment variable to be defined, with
the bot token to use as value.

The log level can be customized using the `RUST_LOG` environment variable (e.g., `export
RUST_LOG=info`).

Please note that upon start the bot will create a Sqlite database named `treasurer.db` in the folder
where it is started.

## TODO list

- turn foreign keys on with `PRAGMA foreign_keys = ON;` (?)

## Future plans

See [here](FUTURE_PLANS.md) for discussions about possible improvements and new features.

## Discarded plans

See [here](DISCARDED_PLANS.md) for an overview of changes that we decided not to implement.

## Useful links

- [Telegram bot library](https://crates.io/crates/teloxide)
- [SQLite instructions](https://rust-lang-nursery.github.io/rust-cookbook/database/sqlite.html)
- [SQLite NULL handling](https://www.sqlite.org/nulls.html)
- [nom combinators](https://github.com/Geal/nom/blob/main/doc/choosing_a_combinator.md)
- [format! options](https://doc.rust-lang.org/std/fmt/)
