# 0.2.0

## Added

- The `listall` command, that shows also settled expenses

## Changed

- The page size for `/list` and `/listall` has been lowered from 20 to 15
- The list of exchanges returned by `/balance` is now sorted (first by debtor and then by creditor)

# 0.1.1

## Fixed

- Added date to list of expenses
- Improved algorithm that computes balance (use floating point as long as possible)

# 0.1.0

Initial version of the bot.
