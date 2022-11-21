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

- update INSTRUCTIONS.md and README.md
- add aliases (?)
- add interactive mode to delete expense (?)
- add interactive mode to list expenses (?)
- add interactive mode to register expense (?)

### Group commands

The group name must be preceded by `#`.

- /addgroup group_name p1 p2 p3
- /deletegroup group_name
- /addgroupmembers group_name p1 p2 p3
- /removegroupmembers group_name p1 p2 p3
- /listgroups
- /listgroupmembers group_name

## Useful links

- [Telegram bot library](https://crates.io/crates/teloxide)
- [SQLite
  instructions](https://rust-lang-nursery.github.io/rust-cookbook/database/sqlite.html)
- [nom
  combinators](https://github.com/Geal/nom/blob/main/doc/choosing_a_combinator.md)
- [format! options](https://doc.rust-lang.org/std/fmt/)

## Future plans

With this version it's a little painful to write names, everybody has to use the same ones and any
typo will break the balance.

This can be improved.

### Add an alias for `self` [DEPRECATED]

So that you don't have to write your own name. This is easy to implement, but not sure how useful.

### Add user aliases [DEPRECATED]

Assume the bot has the list of participants (more on that later). Every time it does not recognize a
name, it can send a custom keyboard with all the names and ask the user who the new name corresponds
to. **This will prevent typos**. Afterwards the new name will be available as an alias.

Of course, **if different people use the same alias for different persons that can be a problem**.

#### Prerequisites

- get list of participants (see below)
- bot must be upgraded to use
  [dispatching](https://docs.rs/teloxide/latest/teloxide/dispatching/index.html) because now there
  is some state in the conversation (may be beneficial in itself, so that we don't open and close
  Sqlite connections)

### Get list of participants

Less easy than it looks, the bot API [cannot be used to retrieve a list of group
members](https://stackoverflow.com/questions/33844290/how-to-get-telegram-channel-users-list-with-telegram-bot-api)
(but it can tell you the number of participants in a group).

We can use the [Telegram API](https://core.telegram.org/#telegram-api) instead, which is linked to a
specific account but can be used by bots too. This API is based on a custom RPC protocol, not
something you want to mess with manually...

In Rust the best implementation is given by `grammers`. An example:
https://github.com/Lonami/grammers/blob/master/lib/grammers-client/examples/echo.rs. For this
purpose is probably sufficient.

**Alternative:** just ask for a list of participants when entering a new group, or when prompted
with a command. The downside is that it is more manual, but way easier to implement.

### Add command to define user aliases

This is a variant of the `Add user aliases` plan. It is much easier to implement and provides a
similar user experience, so it is preferable.

In short, we should add a `\alias alias_name p1 p2 p3` command to define aliases. It will not change
how an expense is saved in the database, but it will change how the balance is computed: every alias
is mapped to the list of participants it represents before computing the balance.

#### Use cases

1. Do not type each time many names when most expenses are shared between the same people: a common
  alias will be `all`
2. assume `p1` and `p2` are used as participants in different expenses, but they are in fact the
  same participant; you can alias them to fix all expenses, without manually deleting and
  re-inserting all the ones that use `p2` instead of `p1`

Both use cases are interesting, but (1) is more important than (2).

Supporting (2) means (among other things) that:

- we cannot use a special character for aliases (e.g., prepend them with `$`)
- we should accept lazy initialization, so an alias can be defined after it is used

#### Amounts for aliases

We may be tempted to prevent using amounts for aliases, but this defeats use case (2).

So it's probably better to use this rule: **if an amount is used with an alias, its value applies to
all participants**.

#### Recursive aliases

Can an alias be defined on top of another alias, as in `/alias alias_name another_alias p2`?

Probably a bad idea, but we can only check that it does not happen when computing a balance, since
aliases can be defined lazily.

In order to make the usage cleaner then we may accept it, though the implementation will be a little
more difficult and we must be careful to detect circular dependencies.

#### Change an alias definition

I don't think we should encourage this action. It will be enough to give the possibility to delete
aliases.

This works because alias definition is lazy. Otherwise, we should check that an alias is never used
in order to delete it. I don't think this provides a nice user experience.

#### Participant repetition

In the current version we are very strict, every participant can appear at most once as creditor and
at most once as debtor. With the usage of aliases we should relax this, otherwise one cannot write
something like

    /e p1 12 all p3/2

to say that `p1` paid for all but `p3` owes a custom amount. However, we have to make sure that:

1. if a participant has a custom amount, it takes precedence over all other appearances
2. a participant cannot appear twice with custom amounts (see next section too)

Again, since alias definition is lazy, we cannot check (2) until it's time to compute the balance.

##### Can a participant appear more than once with custom amounts?

Let's consider this convoluted example, where there are two aliases and `p3` is part of both.

    /e p1 12 a1/2 a2/3

Here we are saying that the participants in `a1` should pay 2 each and the participants in `a2`
should pay 3 each. How much should `p3` pay?

We may solve this by forcing the user to explicitly set a value for `p3`. So the previous example
should cause an error (again, only when computing the balance), while the following will work:

    /e p1 12 a1/2 a2/3 p3/1

#### Checks that will have to be deferred until balance computation

To sum up, these are the checks:

1. the total amount paid does not correspond to the expense: either it is greater or is fixed for
   all participants and is lower
2. the total amount owed does not correspond to the expense: either it is lower or is fixed for all
   participants and is greater
3. a participant appears as part of aliases with different custom amounts and does not appear
   explicitly with a custom amount
4. an alias is not defined
5. there are circular dependencies in the alias definitions

#### Should we really allow custom amounts for aliases?

To support use case (2), it is only necessary to allow custom amounts for aliases that represent a
single participant and not a group. If we limit ourselves to this, we can execute checks (1) and (2)
directly when an expense is created. (3) instead can only occur if a participant appears under
different aliases, but this can only be checked when computing the balance.

What's more, we could have two different concepts:

- `\alias` to say that two participants are in fact the same
- `\group` to define a name that represents a list of participants

Some things we could do:

1. groups may use a special character and we may ask for them to be defined in advance (but we still
   have to take care about the possibility to delete/re-define them)
2. custom amounts cannot be used with groups
3. groups can (and should) be resolved when used; this is so you can add new people to the `all`
   group without affecting past expenses
4. **if we decide to force the registration of participants**, we can raise an error if a name in an
   expense is not present either as a participant or as an alias: this can be useful in bigger
   groups, where the balance may be a long list and it may be difficult to verify if an alias is
   undefined
