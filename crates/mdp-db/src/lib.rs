use std::path::{Path, PathBuf};

use mdp_core::NoteDocument;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct Database {
    path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteSummary {
    pub id: String,
    pub title: String,
    pub note_type: String,
    pub updated_at: String,
    pub path: String,
}

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("property value could not be serialized: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, DatabaseError>;

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let database = Self {
            path: path.as_ref().to_path_buf(),
        };
        database.migrate()?;
        Ok(database)
    }

    pub fn upsert_note(&self, document: &NoteDocument, path: &Path) -> Result<()> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        let id = document.frontmatter.id.to_string();
        let path = path.to_string_lossy().to_string();
        let body_hash = simple_hash(&document.body);
        let frontmatter_hash = simple_hash(&serde_json::to_string(&document.frontmatter)?);

        transaction.execute(
            r#"
            insert into notes (
              id, path, title, note_type, body_hash, frontmatter_hash,
              created_at, updated_at, indexed_at, body
            ) values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'), ?9)
            on conflict(id) do update set
              path = excluded.path,
              title = excluded.title,
              note_type = excluded.note_type,
              body_hash = excluded.body_hash,
              frontmatter_hash = excluded.frontmatter_hash,
              created_at = excluded.created_at,
              updated_at = excluded.updated_at,
              indexed_at = datetime('now'),
              body = excluded.body
            "#,
            params![
                id,
                path,
                document.frontmatter.title,
                document.frontmatter.note_type,
                body_hash,
                frontmatter_hash,
                document.frontmatter.created_at.to_rfc3339(),
                document.frontmatter.updated_at.to_rfc3339(),
                document.body,
            ],
        )?;

        transaction.execute("delete from properties where note_id = ?1", params![id])?;

        insert_property(
            &transaction,
            &id,
            "title",
            &serde_json::Value::String(document.frontmatter.title.clone()),
        )?;
        insert_property(
            &transaction,
            &id,
            "type",
            &serde_json::Value::String(document.frontmatter.note_type.clone()),
        )?;
        insert_property(
            &transaction,
            &id,
            "tags",
            &serde_json::to_value(&document.frontmatter.tags)?,
        )?;
        insert_property(
            &transaction,
            &id,
            "aliases",
            &serde_json::to_value(&document.frontmatter.aliases)?,
        )?;

        for (key, value) in &document.frontmatter.extra {
            insert_property(&transaction, &id, key, value)?;
        }

        transaction.commit()?;
        Ok(())
    }

    pub fn list_notes(&self) -> Result<Vec<NoteSummary>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            r#"
            select id, title, note_type, updated_at, path
            from notes
            order by updated_at desc, title asc
            "#,
        )?;

        let rows = statement.query_map([], |row| {
            Ok(NoteSummary {
                id: row.get(0)?,
                title: row.get(1)?,
                note_type: row.get(2)?,
                updated_at: row.get(3)?,
                path: row.get(4)?,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(DatabaseError::from)
    }

    pub fn note_path(&self, id: &str) -> Result<Option<PathBuf>> {
        let connection = self.connection()?;
        let path = connection
            .query_row("select path from notes where id = ?1", params![id], |row| {
                row.get::<_, String>(0)
            })
            .optional()?;

        Ok(path.map(PathBuf::from))
    }

    pub fn delete_note(&self, id: &str) -> Result<()> {
        let connection = self.connection()?;
        connection.execute("delete from properties where note_id = ?1", params![id])?;
        connection.execute(
            "delete from links where source_id = ?1 or target_id = ?1",
            params![id],
        )?;
        connection.execute("delete from notes where id = ?1", params![id])?;
        Ok(())
    }

    fn migrate(&self) -> Result<()> {
        let connection = self.connection()?;
        connection.execute_batch(
            r#"
            pragma journal_mode = wal;
            pragma foreign_keys = on;

            create table if not exists notes(
              id text primary key,
              path text not null unique,
              title text not null,
              note_type text not null,
              body_hash text not null,
              frontmatter_hash text not null,
              created_at text not null,
              updated_at text not null,
              indexed_at text not null,
              body text not null
            );

            create table if not exists properties(
              note_id text not null,
              key text not null,
              type text not null,
              value_text text,
              value_number real,
              value_bool integer,
              value_datetime text,
              value_json text,
              primary key(note_id, key),
              foreign key(note_id) references notes(id) on delete cascade
            );

            create table if not exists links(
              source_id text not null,
              target_id text not null,
              alias text,
              position_json text
            );
            "#,
        )?;

        Ok(())
    }

    fn connection(&self) -> Result<Connection> {
        Ok(Connection::open(&self.path)?)
    }
}

fn insert_property(
    transaction: &rusqlite::Transaction<'_>,
    note_id: &str,
    key: &str,
    value: &serde_json::Value,
) -> Result<()> {
    let property_type = property_type(value);
    let value_text = value.as_str().map(ToOwned::to_owned);
    let value_number = value.as_f64();
    let value_bool = value.as_bool().map(i64::from);
    let value_json = serde_json::to_string(value)?;

    transaction.execute(
        r#"
        insert into properties (
          note_id, key, type, value_text, value_number, value_bool, value_json
        ) values (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
        params![
            note_id,
            key,
            property_type,
            value_text,
            value_number,
            value_bool,
            value_json
        ],
    )?;

    Ok(())
}

fn property_type(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::String(_) => "string",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "json",
        serde_json::Value::Null => "null",
    }
}

fn simple_hash(input: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}
