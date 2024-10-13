# 0.2.1

## Fixed

- The log library now implements log rotation (max file size is 10 MB and only the last two files
  are kept)
- The bot spawns a background thread that logs its state every three hours (this is an attempt at
  preventing the OS killing the bot after long periods of inactivity)

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
