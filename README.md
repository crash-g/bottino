# Treasurer bot a.k.a. *bottino*

This Telegram bot can be used to track expenses in a group of people.

For instructions on how to use it, see [here](INSTRUCTIONS.md).

## Build and run

Just use `cargo run`. The bot expects the `TELOXIDE_TOKEN` environment variable to be defined, with
the bot token to use as value.

The log level can be customized using the `RUST_LOG` environment variable (e.g., `export
RUST_LOG=info`).

Please note that upon start the bot will create a Sqlite database named `treasurer.db` in the folder
where it is started.

## TODO list

- add commands to add and remove aliases
- accept UTF-8 input
- turn foreign keys on with `PRAGMA foreign_keys = ON;` (?)
- add interactive mode to delete expense (?)
- add interactive mode to list expenses (?)
- add interactive mode to register expense (?)
- add other interactive modes (?)

## Useful links

- [Telegram bot library](https://crates.io/crates/teloxide)
- [SQLite instructions](https://rust-lang-nursery.github.io/rust-cookbook/database/sqlite.html)
- [SQLite NULL handling](https://www.sqlite.org/nulls.html)
- [nom combinators](https://github.com/Geal/nom/blob/main/doc/choosing_a_combinator.md)
- [format! options](https://doc.rust-lang.org/std/fmt/)

## Future plans

Commands are not interactive and we have no aliases.

Also, we may add the possibility to get the participant list from the list of group members.

### Interactive commands

The command syntax is easy but some commands have long and difficult-to-spell names.

It would be nice to be able to click on the command list, but if you do it now the command will
immediately be sent without arguments, which in general is not what you want.

To resolve this we would need an interactive version of commands for all commands that accept an
input.

### Add user aliases

Groups are great to prevent the boilerplate of writing many names when it's always the same people
that take part.

However, groups do not cover these use cases:

1. participants are registered with long formal names, that will appear in the balance, but users
   want a shortcut to refer to them in expenses (of course, groups could be *abused* to obtain this,
   but there is still the limitation on custom amounts)
2. (different) people what to address participants with different names (e.g. a complete and formal
   name and a short nickname)

These can be solved by aliases.

### Get list of participants

**Less easy than it looks**, the bot API [cannot be used to retrieve a list of group
members](https://stackoverflow.com/questions/33844290/how-to-get-telegram-channel-users-list-with-telegram-bot-api)
(but it can tell you the number of participants in a group).

We can use the [Telegram API](https://core.telegram.org/#telegram-api) instead, which is linked to a
specific account but can be used by bots too. This API is based on a custom RPC protocol, not
something you want to mess with manually...

In Rust the best implementation is given by `grammers`. An example:
https://github.com/Lonami/grammers/blob/master/lib/grammers-client/examples/echo.rs. For this
purpose is probably sufficient.
