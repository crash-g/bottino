# Instructions

For every chat/group the bot is used in, it keeps a separate list of expenses, participants, aliases
and groups. The participants of such expenses may or may not be chat/group members.

The bot supports the following commands:

**commands to manage expenses**:

- `/expense` or `/e`: register a new expense
- `/balance` or `/b`: show the current balance
- `/reset`: cancel all outstanding debts
- `/list` or `/l`: show list of recent expenses
- `/delete`: delete an expense by ID

**commands to manage participants**:

- `/addparticipants` or `/ap`: add participants that can be used as creditors or debtors in expenses
- `/removeparticipants` or `/rp`: remove participants that should not appear in expenses anymore
- `/listparticipants` or `/lp`: return the list of all registered participants

**commands to manage aliases**:

- `/addparticipantaliases` or `/apa`: add aliases for a participant
- `/removeparticipantaliases` or `/rpa`: remove aliases for a participant
- `/listparticipantaliases` or `/lpa`: return the list of all aliases for a participant

**commands to manage groups**:

- `/addgroup` or `/ag`: create a group of participants
- `/removegroup` or `/rg`: remove a group of participants
- `/addgroupmembers` or `/agm`: add members to a group
- `/removegroupmembers` or `/rgm`: remove members from a group
- `/listgroups` or `/lg`: return the list of all existing groups
- `/listgroupmembers` or `/lgm`: return the list of all members of a group

All commands have shortcuts, except for `/reset` and `/delete` which are dangerous commands and are
therefore intentionally left without a shortcut.

Some commands accept no arguments, other require a string. In general, the bot does not answer to
commands unless required or an error has occurred.

See below for a detailed description.

## Expense

Register a new expense.

### Syntax

The high-level syntax is

```
/expense participant_or_group [participant_or_group...] amount [participant_or_group...] [- message]
```

#### Participant or Group

A single participant or a group of participants to an expense: either **creditors**, if to the left
of the amount, or **debtors**, if to the right.

Every participant/group can be both a creditor and a debtor. By default, a creditor is always
counted as a debtor too. To prevent this (i.e., to register expenses where someone paid without
participating) see the examples below.

There are only minor differences from a participant and a group of participants. In short:

- a group name always starts with `#`
- with groups it is not possible to specify custom amounts

**Participants and groups must be registered before being available in expenses** (see below).

##### Participant

A participant is composed of a **name** and an **optional amount**:

```
name[/amount]
```

###### Name

The name is an alphanumeric sequence that must start with a letter, optionally preceded by
`@`. Examples:

- `abc`
- `a123`
- `@abc` (same as `abc`)
- `@a123` (same as `a123`)
- `@ABC` (same as `abc`, since names are **case-insensitive**)

###### Participant amount

The amount is the amount of money paid, if the participant is a creditor, or owed, if the
participant is a debtor.

It must be a floating point number which follows the same rules of the [expense amount](#amount).

All creditors without a custom amount paid an equal share of the total amount, after subtracting any
creditor custom amount (see examples below).

Similarly, all debtors without a custom amount owe an equal share of the total amount, after
subtracting any debtor custom amount (see examples below).

###### Participant alias

For each participant you can register an arbitrary number of aliases. Aliases can be used to refer
to a participant in expenses: for all intents and purposes, using an alias is exactly like using the
participant name.

##### Group

Groups are basically just a shortcut to specify more than one participant. A group is only composed
of a name, since it's not possible to specify a custom amount (see examples below).

###### Name

The group name follows the same rules of the participant name, with the exception that it must be
prepended with `#`.

#### Amount

The amount is the total amount paid by all creditors. It must be a floating point number with at
most two decimal digits (if there are more, it is truncated).

Both `,` and `.` are valid decimal separators. No other separators are allowed.

Examples:

- `12`
- `12.1`
- `12.14`
- `12,14` (same as `12.14`)
- `12000.1`

#### Message

The message is an optional string of free-text that describes the expense. If present, it must be
preceded by a dash and a space (`- `).

### Examples

#### `/expense p1 12 p2 p3`

`p1`, `p2` and `p3` paid 12 euros; `p1` paid for everybody, so `p2` and `p3` owe him/her 4 euros
each.

#### `/expense @p1 12 p2 p3 - breakfast`

Same as above, but with a note associated to the expense, to remember that the 12 euros were paid
for a breakfast.

#### `/expense @p1 12 p2/2 p3`

Same as above, but `p2` only owes 2 euros to `p1`; that means that `p1` and `p3` spent 5 each.

#### `/expense p1 p2/3 12 p2/2 p3`

The expense was paid by `p2` who put 3 euros and `p1` who put the rest; however, `p2` only spent 2
euros, so `p3` owes 1 euro to `p2` and 4 euros to `p1`.

#### `/expense p1 12 p1/0 p2 p3`

`p1` paid everything but did not spend anything, so `p2` and `p3` must give 6 euros back each.

#### `/expense p1 12 #all`

`p1` paid 12 euros for something where "all" participated. `all` is a group that must be defined
before usage (see below).

#### `/expense p1 12 #all p3/1`

Same as above, but this example shows how it's possible to specify a custom amount for someone that
is part of the `all` group.

#### `/expense #g1 12 #g2 #g3`

The participants in group `g1` paid 12 euros and the participants in group `g2` and `g3` are
debtors: even if there are participants who are both in `g2` and `g3` the expense is still valid.

In fact, a participant can appear many times and will only count once. However, **a participant can
only appear once with a custom amount**.

## Balance

Show the current balance. No argument accepted. The bot prints a series of money exchange which can
be performed to reduce all debts to zero.

The proposed solution is only one of the many possible and, in general, may not be optimal (i.e., it
may require more money exchanges than strictly necessary): since finding the optimal solution is
NP-complete the bot uses a simplified algorithm, which still yields an optimal solution in most real
cases.

## Reset

Cancel all outstanding debts. No argument accepted. This command should be used when all debts have
been repaid and you want to register new expenses.

Note that the bot has no command to register partially paid debts, but you can always register a new
expense that does this.

For instance, assuming `p2` owed 12 euros to `p1` and gave them back, you can add an expense such
as:

```
/e p2 12 p1 p2/0 - reimbursement
```

## List

Show list of recent expenses. If used without arguments, it shows only the latest expense.
Otherwise, the argument must be an integer number which represents the number of expenses to show.

Expenses are shown from newest to oldest. Every expense starts with a number, which is the ID of the
expense.

Examples:

- `/list`: show latest expense
- `/list 3`: show the latest three expenses

## Delete

Delete an expense by ID. The ID can be found using the `/list` command.

Examples:

- `/delete 12`: delete the expense with ID 12

## Add participants

Before using a participant in an expense their name must be registered with this command.

The command accepts a list of space-separated participant names: there must be at least one
participant, or an error message is returned. If one or more participants already exist, they are
silently ignored.

Examples:

- `/addparticipants p1 p2`

## Remove participants

Participants that are no longer needed can be removed with this command.

The syntax is the same as the syntax of the command to add participants (there must be at least one
participant). If one or more participants do not exist, an error message is returned.

If the participant is part of outstanding expenses, it is not removed from them. If needed, these
expenses must be manually deleted.

Examples:

- `/removeparticipants p1 p2`

## List participants

This command is used to get the list of all registered participants. No argument accepted.

## Add participant aliases

Aliases are a way of referring to the same participant with different names. An alias could be, for
instance, a shorter version of a name.

They can be added with this command. If the participant does not exist or if one of the aliases is
already in use (either as the name of a participant or as an alias of another participant), an error
message is returned.

Examples:

```
/addparticipantaliases participant a1 a2
```

## Remove participant aliases

Aliases can be removed from a participant with this command.

If the participant does not exist or one of the aliases is not an alias of the given participant, an
error message is returned.

Examples:

```
/removeparticipantaliases participant a1 a2
```

## List participant aliases

This command returns the list of aliases of a participant.

If the participant does not exist, an error message is returned.

Examples:

- `listparticipantaliases participant`

## Add group

As for participants, groups must be registered before being used in an expense.

If the group already exists, nothing happens.

Examples:

```
/addgroup group_name
```


## Remove group

A group which is no longer needed can be removed with this command. All expenses that previously used
this group are not affected.

If the group does not exist, an error message is returned.

Examples:

- `/removegroup group_name`

## Add group members

Members can be added to an existing group with this command. All expenses that previously used
this group are not affected.

If the group or a participant does not exist, an error message is returned.

Examples:

```
/addgroupmembers group_name p1 p2
```

## Remove group members

Members can be removed from an existing group with this command. All expenses that previously used
this group are not affected.

If the group or a participant does not exist, an error message is returned.

Examples:

```
/removegroupmembers group_name p1 p2
```

## List groups

This command returns the list of all existing groups. No argument accepted.

## List group members

This command returns the list of participants of an existing group.

If the group does not exist, an error message is returned.

Examples:

- `listgroupmembers group_name`
