# Discarded plans

Here we keep track of changes that were attempted but then discarded for some reason.

## Full list

- Return a table when calling the `/balance` command
- Add a `/total` command to show how much a specific participant has spent

## Return a table when calling the `/balance` command

It looked more readable to return a formatted table. Telegram does not format tables but you can
just send it as a code block as explained in this
[answer](https://stackoverflow.com/questions/49345960/how-do-i-send-tables-with-telegram-bot-api).

The easiest way of doing this with Rust is using the
[prettytable](https://github.com/phsym/prettytable-rs) library:

```rust
let table = table!(["Debtor", "Amount", "Creditor"],
                   ["aa", "12.2", "bb"],
                   ["asdfgfff", "123", "bbfegergrgergr"]);

// Note here we use HTML, so the parsemode set when sending must be changed accordingly.
teloxide::utils::html::code_block(&table.to_string())
```

### Why discarded?

Because tables take a lot of space and they become unreadable when a line needs to be wrapped, which
will happen even with relatively short participant names.

## Add a `/total` command to show how much participants have spent

This would be a nice-to-have feature to answer questions like "How much have we spent during this
holiday?".

### Why discarded?

At first look it appears simple to implement: just go through the list of expenses and for each
participant sum their debts.

However, this does not take into account that in practice some expenses are in fact money transfers
(think of something like `\e p1 100 p2 p1/0`, which is the usual way to mark that `p1` has given 100
to `p2`). Money transfers are not real expenses and they would mess up with this command.

It could be possible in principle to add a `/transfer` command for this specific case, which would
have the added benefit that you don't need to write `p1/0` since it is implied, but this looks a
little overkill and would make it more difficult to explain how to use the bot.

Also, the `/transfer` command does not solve all problems, cause there may be expenses which are not
registered in the bot, which still makes the `/total` command a best-effort attempt at answering the
above question.

If, on the other hand, the `/transfer` command is added, then the amount should be optional: not
specifying it would mean that the two listed participants have settled their debt.
