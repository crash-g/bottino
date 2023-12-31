const CREATE_PARTICIPANT_TABLE: &str = "CREATE TABLE IF NOT EXISTS participant (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  chat_id INTEGER NOT NULL,
  name TEXT NOT NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  deleted_at DATETIME,
  UNIQUE(chat_id, name, deleted_at)
)";

const CREATE_ALIAS_TABLE: &str = "CREATE TABLE IF NOT EXISTS alias (
  chat_id INTEGER NOT NULL,
  name TEXT NOT NULL,
  participant_id INTEGER NOT NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  deleted_at DATETIME,
  UNIQUE(chat_id, name, deleted_at)
)";

const CREATE_EXPENSE_TABLE: &str = "CREATE TABLE IF NOT EXISTS expense (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  chat_id INTEGER NOT NULL,
  amount INTEGER NOT NULL,
  message TEXT,
  message_ts DATETIME NOT NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  settled_at DATETIME,
  deleted_at DATETIME
)";

const CREATE_EXPENSE_PARTICIPANT_TABLE: &str = "CREATE TABLE IF NOT EXISTS expense_participant (
  expense_id INTEGER NOT NULL,
  participant_id INTEGER NOT NULL,
  is_creditor BOOL NOT NULL,
  amount INTEGER,
  UNIQUE(expense_id, participant_id, is_creditor)
)";

const CREATE_GROUP_TABLE: &str = "CREATE TABLE IF NOT EXISTS participant_group (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  chat_id INTEGER NOT NULL,
  name TEXT NOT NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  deleted_at DATETIME,
  UNIQUE(chat_id, name, deleted_at)
)";

const CREATE_GROUP_MEMBER_TABLE: &str = "CREATE TABLE IF NOT EXISTS group_member (
  group_id INTEGER NOT NULL,
  participant_id INTEGER NOT NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  deleted_at DATETIME,
  UNIQUE(group_id, participant_id, deleted_at)
)";

const CREATE_AUTO_REGISTER_FLAG_TABLE: &str = "CREATE TABLE IF NOT EXISTS chat_flag (
  chat_id INTEGER NOT NULL,
  auto_register BOOL NOT NULL DEFAULT FALSE,
  UNIQUE(chat_id)
)";

pub fn create_all_tables(connection: &rusqlite::Connection) -> anyhow::Result<()> {
    connection.execute(CREATE_PARTICIPANT_TABLE, ())?;
    connection.execute(CREATE_ALIAS_TABLE, ())?;
    connection.execute(CREATE_EXPENSE_TABLE, ())?;
    connection.execute(CREATE_EXPENSE_PARTICIPANT_TABLE, ())?;
    connection.execute(CREATE_GROUP_TABLE, ())?;
    connection.execute(CREATE_GROUP_MEMBER_TABLE, ())?;
    connection.execute(CREATE_AUTO_REGISTER_FLAG_TABLE, ())?;
    Ok(())
}
