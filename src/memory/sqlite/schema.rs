const CREATE_EXPENSE_TABLE: &'static str = "CREATE TABLE IF NOT EXISTS expense (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  chat_id INTEGER NOT NULL,
  amount INTEGER NOT NULL,
  message TEXT,
  message_ts DATETIME NOT NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  settled_at DATETIME,
  deleted_at DATETIME
)";

const CREATE_PARTICIPANT_TABLE: &'static str = "CREATE TABLE IF NOT EXISTS participant (
  name TEXT NOT NULL,
  is_creditor BOOL NOT NULL,
  expense_id INTEGER NOT NULL,
  amount INTEGER
)";

pub fn create_all_tables(connection: &rusqlite::Connection) -> anyhow::Result<()> {
    connection.execute(CREATE_EXPENSE_TABLE, ())?;
    connection.execute(CREATE_PARTICIPANT_TABLE, ())?;
    Ok(())
}
