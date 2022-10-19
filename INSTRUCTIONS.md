# Instructions

For every chat/group the bot is used in, it keeps a separate list of expenses. The participants of
such expenses may or may not be chat/group members.

The bot supports the following list of commands:

- `/expense` or `/e`: register a new expense
- `/balance`: show the current balance
- `/reset`: cancel all outstanding debts
- `/list`: show list of recent expenses
- `/delete`: delete an expense by ID

Some commands accept no arguments, other require a string. See below for a detailed description.

## Expense

Register a new expense.

### Syntax

The high-level syntax is

```
/expense participant [participant...] amount [participant...] [- message]
```

#### Participant

A participant to an expense: either a **creditor**, if to the left of the amount, or a **debtor**,
if to the right.

Every participant can be both a creditor and a debtor. By default, a creditor is always counted as a
debtor too. To prevent this (i.e., to register expenses where someone paid without participating)
see the examples below.

The participant is composed of a **name** and an **optional amount**:

```
name[/amount]
```

##### Name

The name is an alphanumeric sequence of ASCII characters, optionally preceded by `@`. Examples:

- `abc`
- `a123`
- `@abc` (same as `abc`)
- `@a123` (same as `a123`)

##### Participant amount

The amount is the amount of money paid, if the participant is a creditor, or owed, if the
participant is a debtor. It must be a floating point number which follows the same rules of the
[expense amount](#amount).

All creditors without a custom amount paid an equal share of the total amount, after subtracting any
creditor custom amount (see examples below).

Similarly, all debtors without a custom amount owe an equal share of the total amount, after
subtracting any debtor custom amount (see examples below).

#### Amount

The amount is the total amount paid by all creditors. It must be a floating point number with at
most two decimal digits (if there are more, it is truncated).

Both `,` and `.` are valid decimal separators. No other separators are allowed.

Examples:

- `12`
- `12.1`
- `12.14`
- `12,14` (same as `12.14`)

#### Message

The message is an optional string of free-text that describes the expense. If present, it must be
preceded by a dash and a space (`- `).

### Examples

- `/expense p1 12 p2 p3`: `p1`, `p2` and `p3` paid something 12 euros; `p1` paid for everybody, so
  `p2` and `p3` owe him/her 4 euros each
- `/expense @p1 12 p2 p3 - breakfast`: same as above, but with a note associated to the expense, to
  remember that the 12 euros were paid for a breakfast
- `/expense @p1 12 p2/2 p3`: same as above, but `p2` only owes 2 euros to `p1`; that means that `p1`
  and `p3` spent 5 each
- `/expense p1 p2/3 12 p2/2 p3`: the expense was paid by `p2` who put 3 euros and `p1` who put the
  rest; however, `p2` only spent 2 euros, so `p3` owes 1 euro to `p2` and 4 euros to `p1`
- `/expense p1 12 p1/0 p2 p3`: `p1` paid everything but did not spend anything, so `p2` and `p3`
  must give 6 euros back each

## E

This is just a shortcut for `/expense`.

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
