# Discarded plans

Here we keep track of changes that were attempted but then discarded for some reason.

## Full list

- Return a table when calling the `/balance` command

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
