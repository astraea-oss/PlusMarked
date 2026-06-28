use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use mdp_core::{new_note, parse_document, serialize_document, update_note, NoteDocument};
pub use mdp_db::NoteSummary;
use mdp_db::Database;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct WorkspaceHandle {
    root: PathBuf,
    database: Database,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSummary {
    pub root: String,
    pub note_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateNoteInput {
    pub title: Option<String>,
    pub note_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveNoteInput {
    pub id: String,
    pub title: String,
    pub note_type: String,
    pub tags: Vec<String>,
    pub aliases: Vec<String>,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveResult {
    pub note: NoteSummary,
}

impl WorkspaceHandle {
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(root.join("notes")).context("creating notes directory")?;
        fs::create_dir_all(root.join("bases")).context("creating bases directory")?;
        fs::create_dir_all(root.join(".local")).context("creating local metadata directory")?;

        let workspace_config = root.join("workspace.toml");
        if !workspace_config.exists() {
            fs::write(
                &workspace_config,
                "schema_version = 1\napp = \"MarkdownPlus\"\n",
            )
            .context("writing workspace.toml")?;
        }

        let database = Database::open(root.join(".local").join("index.sqlite"))
            .context("opening workspace database")?;

        let handle = Self { root, database };
        handle.reindex_notes()?;
        Ok(handle)
    }

    pub fn summary(&self) -> Result<WorkspaceSummary> {
        Ok(WorkspaceSummary {
            root: self.root.to_string_lossy().to_string(),
            note_count: self.database.list_notes()?.len(),
        })
    }

    pub fn create_note(&self, input: CreateNoteInput) -> Result<NoteSummary> {
        let title = input.title.unwrap_or_else(|| "Untitled".to_string());
        let document = new_note(title, input.note_type);
        let path = self.note_path(&document.frontmatter.id.to_string());
        write_document_atomic(&path, &document)?;
        self.database.upsert_note(&document, &path)?;
        self.find_summary(&document.frontmatter.id.to_string())
    }

    pub fn list_notes(&self) -> Result<Vec<NoteSummary>> {
        Ok(self.database.list_notes()?)
    }

    pub fn get_note(&self, id: &str) -> Result<NoteDocument> {
        let path = self
            .database
            .note_path(id)?
            .unwrap_or_else(|| self.note_path(id));
        let input = fs::read_to_string(&path)
            .with_context(|| format!("reading note {}", path.to_string_lossy()))?;
        parse_document(&input).map_err(Into::into)
    }

    pub fn save_note(&self, input: SaveNoteInput) -> Result<SaveResult> {
        let existing = self.get_note(&input.id)?;
        let document = update_note(
            existing,
            input.title,
            input.note_type,
            input.tags,
            input.aliases,
            input.body,
        );
        let path = self.note_path(&document.frontmatter.id.to_string());
        write_document_atomic(&path, &document)?;
        self.database.upsert_note(&document, &path)?;

        Ok(SaveResult {
            note: self.find_summary(&document.frontmatter.id.to_string())?,
        })
    }

    fn reindex_notes(&self) -> Result<()> {
        for entry in fs::read_dir(self.root.join("notes")).context("reading notes directory")? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("mdp") {
                continue;
            }

            let input = fs::read_to_string(&path)
                .with_context(|| format!("reading note {}", path.to_string_lossy()))?;
            let document = parse_document(&input)
                .with_context(|| format!("parsing note {}", path.to_string_lossy()))?;
            self.database.upsert_note(&document, &path)?;
        }

        Ok(())
    }

    fn find_summary(&self, id: &str) -> Result<NoteSummary> {
        self.database
            .list_notes()?
            .into_iter()
            .find(|note| note.id == id)
            .ok_or_else(|| anyhow!("note {id} was saved but not found in the index"))
    }

    fn note_path(&self, id: &str) -> PathBuf {
        self.root.join("notes").join(format!("{id}.mdp"))
    }
}

fn write_document_atomic(path: &Path, document: &NoteDocument) -> Result<()> {
    let content = serialize_document(document)?;
    let temporary_path = path.with_extension("mdp.tmp");
    fs::write(&temporary_path, content)
        .with_context(|| format!("writing temporary note {}", temporary_path.to_string_lossy()))?;
    fs::rename(&temporary_path, path)
        .with_context(|| format!("moving note into place {}", path.to_string_lossy()))?;
    Ok(())
}

