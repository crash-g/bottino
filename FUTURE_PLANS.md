# Future plans

Here we keep track of possible improvements or new features that need to be fleshed out.

## Full list

- Commands are not interactive
- We may add the possibility to get the participant list from the list of group members

## Interactive commands

The command syntax is easy but some expect arguments and Telegram has this unintuitive behavior
where a command is immediately sent (without arguments) if you click on it.

To improve on this we would need an interactive version of commands for all commands that accept an
input: instead of returning an error, these commands would ask for more input interactively.

**Usefulness**: not really necessary for expert users, but it would make the bot more user-friendly
for beginners.

### Detailed list of interactive modes

- add interactive mode to delete expense
- add interactive mode to list expenses
- add interactive mode to register expense
- add all other interactive modes

## Get list of participants

**Less easy than it looks**, the bot API [cannot be used to retrieve a list of group
members](https://stackoverflow.com/questions/33844290/how-to-get-telegram-channel-users-list-with-telegram-bot-api)
(but it can tell you the number of participants in a group).

We can use the [Telegram API](https://core.telegram.org/#telegram-api) instead, which is linked to a
specific account but can be used by bots too. This API is based on a custom RPC protocol, not
something you want to mess with manually...

In Rust the best implementation is given by `grammers`. An example:
https://github.com/Lonami/grammers/blob/master/lib/grammers-client/examples/echo.rs. For this
purpose is probably sufficient.

**Usefulness**: I think it could be mainly useful for large groups; for small groups, participants
can be quickly registered at the beginning and you don't need to worry about this again. Also, the
auto register mode greatly reduces the need for this feature.
