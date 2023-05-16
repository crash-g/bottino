# Quickstart

This bot keeps track of who owns money to whom. The main commands you will use are:

- `/expense` (or `/e` in short) to add a new expense
- `/balance` (or `/b` in short) to get the current balance

When you create a new group and you add the bot, first thing you should register participants:

```
/ap Anna Marco Sara
```

Then you can start to register expenses. Let's say that *Sara* paid for lunch, 55 euros in total:

```
/e Anna 55 Marco Sara
```

Note that creditors will appear to the left of the amount, while debtors will appear to the right.

Then assume that *Marco* paid 30 euros for the cinema and that *Sara* bought pop-corn for *Marco*
and herself:

```
/e Marco 30 Anna Sara
/e Sara 10 Marco
```

You can get the current balance with

```
/b
```

which will say that *Sara* owes 23.33 euros to *Anna* and *Marco* owes 3.33 euros to *Anna*.

When all money have been given back, you can erase all outstanding debts in one go with

```
/reset
```

## Aliases and groups

Aliases give the possibility to call participants with different names; they are especially useful
if you want to use shortcuts (e.g., *a* instead of *anna*).

Groups give the possibility to mention multiple participants at the same time, without writing all
their names. For instance, let's assume that a group named **all** exists and that it contains
*Anna*, *Marco* and *Sara*. Then the first two expenses above could be rewritten as:

```
/e Anna 55 #all
/e Marco 30 #all
```

## Custom amounts

Sometimes an expense cannot be split exactly, because someone paid more than someone else. Custom
amounts can be added with:

```
/e Anna 55 Marco Sara/25
```

This means that *Sara* owes 25 euros to *Anna*, while *Marco* and *Anna* share the rest of the
expense (so *Marco* owes 15 euros to *Anna*).

Custom amounts can also be used for creditors, in which case they mean that a creditor contributed
for exactly that much.

## Where to go from here

The examples above are enough for basic usage, but you should refer to the [full
instructions](INSTRUCTIONS.md) for the complete list of available commands.
