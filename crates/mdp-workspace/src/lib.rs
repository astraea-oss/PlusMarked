use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Utc};
use mdp_core::{NoteDocument, new_note, parse_document, serialize_document, update_note};
use mdp_db::Database;
pub use mdp_db::NoteSummary;
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
pub struct CreateBaseInput {
    pub title: Option<String>,
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
pub struct NoteSource {
    pub id: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveNoteSourceInput {
    pub id: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveResult {
    pub note: NoteSummary,
}

impl WorkspaceHandle {
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = normalize_workspace_root(root.as_ref())?;
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
        let path = self.note_path_for_document(&document, None)?;
        write_document_atomic(&path, &document)?;
        self.database.upsert_note(&document, &path)?;
        self.find_summary(&document.frontmatter.id.to_string())
    }

    pub fn create_base(&self, input: CreateBaseInput) -> Result<NoteSummary> {
        let title = input.title.unwrap_or_else(|| "Untitled base".to_string());
        let path = self.base_path_for_title(&title, None)?;
        let source = default_base_source(&title);
        write_source_atomic(&path, &source)?;
        self.base_summary_for_path(&path)
    }

    pub fn rename_base(&self, id: &str, title: &str) -> Result<NoteSummary> {
        let path = self
            .base_path_from_id(id)?
            .ok_or_else(|| anyhow!("base not found: {id}"))?;
        if !path.exists() {
            return Err(anyhow!("base not found: {id}"));
        }

        let next_path = self.base_path_for_title(title, Some(&path))?;
        if !paths_equal(&next_path, Some(&path)) {
            fs::rename(&path, &next_path).with_context(|| {
                format!(
                    "renaming base {} to {}",
                    path.to_string_lossy(),
                    next_path.to_string_lossy()
                )
            })?;
        }

        self.base_summary_for_path(&next_path)
    }

    pub fn list_notes(&self) -> Result<Vec<NoteSummary>> {
        let mut documents = self.database.list_notes()?;
        documents.extend(self.list_bases()?);
        documents.sort_by(|left, right| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then_with(|| left.title.cmp(&right.title))
        });
        Ok(documents)
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

    pub fn get_note_source(&self, id: &str) -> Result<NoteSource> {
        if let Some(path) = self.base_path_from_id(id)? {
            let source = fs::read_to_string(&path)
                .with_context(|| format!("reading base {}", path.to_string_lossy()))?;

            return Ok(NoteSource {
                id: id.to_string(),
                source,
            });
        }

        let path = self
            .database
            .note_path(id)?
            .unwrap_or_else(|| self.note_path(id));
        let source = fs::read_to_string(&path)
            .with_context(|| format!("reading note {}", path.to_string_lossy()))?;

        Ok(NoteSource {
            id: id.to_string(),
            source,
        })
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
        let previous_path = self.database.note_path(&document.frontmatter.id.to_string())?;
        let path = self.note_path_for_document(&document, previous_path.as_deref())?;
        write_document_atomic(&path, &document)?;
        remove_replaced_note_file(previous_path.as_deref(), &path)?;
        self.database.upsert_note(&document, &path)?;

        Ok(SaveResult {
            note: self.find_summary(&document.frontmatter.id.to_string())?,
        })
    }

    pub fn save_note_source(&self, input: SaveNoteSourceInput) -> Result<SaveResult> {
        if let Some(path) = self.base_path_from_id(&input.id)? {
            write_source_atomic(&path, &input.source)?;
            return Ok(SaveResult {
                note: self.base_summary_for_path(&path)?,
            });
        }

        let document = parse_document(&input.source).context("parsing MarkdownPlus source")?;
        let document_id = document.frontmatter.id.to_string();
        if document_id != input.id {
            return Err(anyhow!(
                "note id cannot be changed from {} to {}",
                input.id,
                document_id
            ));
        }

        let previous_path = self.database.note_path(&document_id)?;
        let path = self.note_path_for_document(&document, previous_path.as_deref())?;
        write_source_atomic(&path, &input.source)?;
        remove_replaced_note_file(previous_path.as_deref(), &path)?;
        self.database.upsert_note(&document, &path)?;

        Ok(SaveResult {
            note: self.find_summary(&document_id)?,
        })
    }

    pub fn delete_note(&self, id: &str) -> Result<()> {
        if let Some(path) = self.base_path_from_id(id)? {
            if !path.exists() {
                return Err(anyhow!("base not found: {id}"));
            }

            fs::remove_file(&path)
                .with_context(|| format!("deleting base {}", path.to_string_lossy()))?;
            return Ok(());
        }

        let path = self
            .database
            .note_path(id)?
            .unwrap_or_else(|| self.note_path(id));

        if !path.exists() {
            return Err(anyhow!("note not found: {id}"));
        }

        fs::remove_file(&path)
            .with_context(|| format!("deleting note {}", path.to_string_lossy()))?;
        self.database.delete_note(id)?;
        Ok(())
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
            let target_path = self.note_path_for_document(&document, Some(&path))?;
            if target_path != path {
                write_source_atomic(&target_path, &input)?;
                remove_replaced_note_file(Some(&path), &target_path)?;
            }
            self.database.upsert_note(&document, &target_path)?;
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

    fn list_bases(&self) -> Result<Vec<NoteSummary>> {
        let bases_dir = self.root.join("bases");
        let mut bases = Vec::new();
        for entry in fs::read_dir(&bases_dir).context("reading bases directory")? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("base") {
                continue;
            }

            bases.push(self.base_summary_for_path(&path)?);
        }

        Ok(bases)
    }

    fn base_summary_for_path(&self, path: &Path) -> Result<NoteSummary> {
        let title = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("Untitled base")
            .to_string();
        let modified = fs::metadata(path)
            .and_then(|metadata| metadata.modified())
            .ok()
            .map(DateTime::<Utc>::from)
            .unwrap_or_else(Utc::now);

        Ok(NoteSummary {
            id: self.base_id_for_path(path)?,
            title,
            note_type: "base".to_string(),
            updated_at: modified.to_rfc3339(),
            path: path.to_string_lossy().to_string(),
        })
    }

    fn base_id_for_path(&self, path: &Path) -> Result<String> {
        let relative = path
            .strip_prefix(self.root.join("bases"))
            .with_context(|| format!("base path is outside bases directory: {}", path.to_string_lossy()))?;
        Ok(format!("base:{}", relative.to_string_lossy().replace('\\', "/")))
    }

    fn base_path_from_id(&self, id: &str) -> Result<Option<PathBuf>> {
        let Some(relative) = id.strip_prefix("base:") else {
            return Ok(None);
        };

        if relative.contains("..") || relative.starts_with('/') || relative.starts_with('\\') {
            return Err(anyhow!("invalid base id: {id}"));
        }

        let path = self.root.join("bases").join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
        if path.extension().and_then(|extension| extension.to_str()) != Some("base") {
            return Err(anyhow!("invalid base id: {id}"));
        }

        Ok(Some(path))
    }

    fn base_path_for_title(&self, title: &str, current_path: Option<&Path>) -> Result<PathBuf> {
        let bases_dir = self.root.join("bases");
        let title_slug = filename_stem_from_title(title);
        let preferred = bases_dir.join(format!("{title_slug}.base"));
        if self.can_use_base_path(&preferred, current_path) {
            return Ok(preferred);
        }

        for index in 2.. {
            let candidate = bases_dir.join(format!("{title_slug} {index}.base"));
            if self.can_use_base_path(&candidate, current_path) {
                return Ok(candidate);
            }
        }

        unreachable!("unbounded base filename suffix loop should always return");
    }

    fn can_use_base_path(&self, candidate: &Path, current_path: Option<&Path>) -> bool {
        paths_equal(candidate, current_path) || !candidate.exists()
    }

    fn note_path_for_document(
        &self,
        document: &NoteDocument,
        current_path: Option<&Path>,
    ) -> Result<PathBuf> {
        let id = document.frontmatter.id.to_string();
        let title_slug = filename_stem_from_title(&document.frontmatter.title);
        let notes_dir = self.root.join("notes");
        let short_id = id.chars().take(8).collect::<String>();

        let preferred = notes_dir.join(format!("{title_slug}.mdp"));
        if self.can_use_note_path(&preferred, &id, current_path)? {
            return Ok(preferred);
        }

        let fallback = notes_dir.join(format!("{title_slug} - {short_id}.mdp"));
        if self.can_use_note_path(&fallback, &id, current_path)? {
            return Ok(fallback);
        }

        for index in 2.. {
            let candidate = notes_dir.join(format!("{title_slug} - {short_id} {index}.mdp"));
            if self.can_use_note_path(&candidate, &id, current_path)? {
                return Ok(candidate);
            }
        }

        unreachable!("unbounded filename suffix loop should always return");
    }

    fn can_use_note_path(
        &self,
        candidate: &Path,
        id: &str,
        current_path: Option<&Path>,
    ) -> Result<bool> {
        if paths_equal(candidate, current_path) {
            return Ok(true);
        }

        if !candidate.exists() {
            return Ok(true);
        }

        let input = fs::read_to_string(candidate)
            .with_context(|| format!("reading note {}", candidate.to_string_lossy()))?;
        let existing = parse_document(&input)
            .with_context(|| format!("parsing note {}", candidate.to_string_lossy()))?;
        Ok(existing.frontmatter.id.to_string() == id)
    }
}

fn default_base_source(title: &str) -> String {
    format!(
        r#"filters: {{}}
formulas: {{}}
properties:
  title:
    displayName: Title
  type:
    displayName: Type
  tags:
    displayName: Tags
  aliases:
    displayName: Aliases
  created_at:
    displayName: Created
  updated_at:
    displayName: Modified
views:
  - type: table
    name: "{title}"
    limit: 100
    order:
      - title
      - type
      - tags
      - aliases
      - created_at
      - updated_at
"#,
        title = title.replace('"', "\\\"")
    )
}

fn filename_stem_from_title(title: &str) -> String {
    let mut sanitized = String::with_capacity(title.len());
    let mut previous_was_space = false;

    for character in title.trim().chars() {
        let replacement = if character.is_control() || matches!(character, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
            ' '
        } else {
            character
        };

        if replacement.is_whitespace() {
            if !previous_was_space {
                sanitized.push(' ');
                previous_was_space = true;
            }
        } else {
            sanitized.push(replacement);
            previous_was_space = false;
        }
    }

    let sanitized = sanitized.trim_matches([' ', '.']).chars().take(96).collect::<String>();
    let sanitized = sanitized.trim_matches([' ', '.']).to_string();
    let fallback = if sanitized.is_empty() {
        "Untitled".to_string()
    } else {
        sanitized
    };

    if is_reserved_windows_filename(&fallback) {
        format!("{fallback} note")
    } else {
        fallback
    }
}

fn is_reserved_windows_filename(stem: &str) -> bool {
    let uppercase = stem.to_ascii_uppercase();
    matches!(uppercase.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (uppercase.len() == 4
            && (uppercase.starts_with("COM") || uppercase.starts_with("LPT"))
            && uppercase[3..].chars().all(|character| matches!(character, '1'..='9')))
}

fn paths_equal(left: &Path, right: Option<&Path>) -> bool {
    right.is_some_and(|right| {
        left == right
            || (left.exists()
                && right.exists()
                && dunce::canonicalize(left).ok() == dunce::canonicalize(right).ok())
    })
}

fn remove_replaced_note_file(previous_path: Option<&Path>, next_path: &Path) -> Result<()> {
    let Some(previous_path) = previous_path else {
        return Ok(());
    };

    if !paths_equal(next_path, Some(previous_path)) && previous_path.exists() {
        fs::remove_file(previous_path)
            .with_context(|| format!("removing replaced note {}", previous_path.to_string_lossy()))?;
    }

    Ok(())
}

fn write_document_atomic(path: &Path, document: &NoteDocument) -> Result<()> {
    let content = serialize_document(document)?;
    write_source_atomic(path, &content)
}

fn write_source_atomic(path: &Path, content: &str) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("note path has no parent: {}", path.to_string_lossy()))?;
    fs::create_dir_all(parent)
        .with_context(|| format!("creating note directory {}", parent.to_string_lossy()))?;

    let mut temporary_file = tempfile::Builder::new()
        .prefix(".mdp-write-")
        .suffix(".tmp")
        .tempfile_in(parent)
        .with_context(|| format!("creating temporary note in {}", parent.to_string_lossy()))?;

    temporary_file
        .write_all(content.as_bytes())
        .with_context(|| format!("writing temporary note {}", path.to_string_lossy()))?;
    temporary_file
        .flush()
        .with_context(|| format!("flushing temporary note {}", path.to_string_lossy()))?;
    temporary_file
        .persist(path)
        .map_err(|error| error.error)
        .with_context(|| format!("moving note into place {}", path.to_string_lossy()))?;
    Ok(())
}

fn normalize_workspace_root(root: &Path) -> Result<PathBuf> {
    fs::create_dir_all(root)
        .with_context(|| format!("creating workspace root {}", root.to_string_lossy()))?;
    dunce::canonicalize(root)
        .with_context(|| format!("canonicalizing workspace root {}", root.to_string_lossy()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_reindexes_workspace_with_spaces_and_unicode() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let root = temp
            .path()
            .join("Workspace With Spaces")
            .join("Notes Ünicode");
        let expected_root = dunce::canonicalize({
            fs::create_dir_all(&root)?;
            &root
        })?;

        let workspace = WorkspaceHandle::open(&root)?;
        let summary = workspace.summary()?;

        assert_eq!(summary.root, expected_root.to_string_lossy().to_string());
        assert!(expected_root.join("notes").is_dir());
        assert!(expected_root.join("bases").is_dir());
        assert!(expected_root.join(".local").join("index.sqlite").is_file());

        let note = workspace.create_note(CreateNoteInput {
            title: Some("Portable Path Test".to_string()),
            note_type: None,
        })?;
        assert_eq!(workspace.list_notes()?.len(), 1);
        drop(workspace);

        let reopened = WorkspaceHandle::open(&root)?;
        let notes = reopened.list_notes()?;

        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].id, note.id);
        assert!(notes[0].path.ends_with("Portable Path Test.mdp"));

        Ok(())
    }

    #[test]
    fn note_files_use_title_and_rename_when_title_changes() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let workspace = WorkspaceHandle::open(temp.path().join("Readable Names"))?;
        let note = workspace.create_note(CreateNoteInput {
            title: Some("Testing Note".to_string()),
            note_type: None,
        })?;
        let original_path = workspace.database.note_path(&note.id)?.unwrap();

        assert_eq!(original_path.file_name().and_then(|name| name.to_str()), Some("Testing Note.mdp"));

        let source = workspace
            .get_note_source(&note.id)?
            .source
            .replace("title: Testing Note", "title: Human Title");
        let saved = workspace.save_note_source(SaveNoteSourceInput {
            id: note.id.clone(),
            source,
        })?;
        let renamed_path = PathBuf::from(saved.note.path);

        assert_eq!(renamed_path.file_name().and_then(|name| name.to_str()), Some("Human Title.mdp"));
        assert!(renamed_path.exists());
        assert!(!original_path.exists());

        Ok(())
    }

    #[test]
    fn duplicate_titles_get_readable_unique_suffixes() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let workspace = WorkspaceHandle::open(temp.path().join("Duplicate Names"))?;
        let first = workspace.create_note(CreateNoteInput {
            title: Some("Same Title".to_string()),
            note_type: None,
        })?;
        let second = workspace.create_note(CreateNoteInput {
            title: Some("Same Title".to_string()),
            note_type: None,
        })?;

        let first_path = workspace.database.note_path(&first.id)?.unwrap();
        let second_path = workspace.database.note_path(&second.id)?.unwrap();

        assert_eq!(first_path.file_name().and_then(|name| name.to_str()), Some("Same Title.mdp"));
        assert_ne!(first_path, second_path);
        assert!(second_path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("Same Title - ") && name.ends_with(".mdp")));

        Ok(())
    }

    #[test]
    fn save_note_source_replaces_existing_file_and_cleans_temporary_files() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let workspace = WorkspaceHandle::open(temp.path().join("Replace Existing"))?;
        let note = workspace.create_note(CreateNoteInput {
            title: Some("Replace Test".to_string()),
            note_type: None,
        })?;

        let original = workspace.get_note_source(&note.id)?;
        let first_edit = format!(
            "{}\n# First Heading\nBody one\n",
            original.source.trim_end()
        );
        workspace.save_note_source(SaveNoteSourceInput {
            id: note.id.clone(),
            source: first_edit,
        })?;

        let saved = workspace.get_note_source(&note.id)?;
        assert!(saved.source.contains("# First Heading\nBody one"));

        let second_edit = saved.source.replace("Body one", "Body two");
        workspace.save_note_source(SaveNoteSourceInput {
            id: note.id.clone(),
            source: second_edit,
        })?;

        let saved_again = workspace.get_note_source(&note.id)?;
        assert!(saved_again.source.contains("Body two"));
        assert_no_write_temporary_files(&workspace.root.join("notes"))?;

        Ok(())
    }

    #[test]
    fn delete_note_removes_file_and_index_entry() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let workspace = WorkspaceHandle::open(temp.path())?;
        let note = workspace.create_note(CreateNoteInput {
            title: Some("Delete Me".to_string()),
            note_type: None,
        })?;
        let path = PathBuf::from(&note.path);

        workspace.delete_note(&note.id)?;

        assert!(!path.exists());
        assert!(workspace.list_notes()?.is_empty());
        Ok(())
    }

    #[test]
    fn delete_note_removes_base_file() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let workspace = WorkspaceHandle::open(temp.path())?;
        let base = workspace.create_base(CreateBaseInput {
            title: Some("Delete Base".to_string()),
        })?;
        let path = PathBuf::from(&base.path);

        workspace.delete_note(&base.id)?;

        assert!(!path.exists());
        assert!(workspace.list_notes()?.is_empty());
        Ok(())
    }

    #[test]
    fn rename_base_updates_path_and_summary_id() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let workspace = WorkspaceHandle::open(temp.path())?;
        let base = workspace.create_base(CreateBaseInput {
            title: Some("Original Base".to_string()),
        })?;
        let original_path = PathBuf::from(&base.path);

        let renamed = workspace.rename_base(&base.id, "Renamed Base")?;

        assert_eq!(renamed.title, "Renamed Base");
        assert_ne!(renamed.id, base.id);
        assert!(!original_path.exists());
        assert_eq!(
            PathBuf::from(&renamed.path).file_name().and_then(|name| name.to_str()),
            Some("Renamed Base.base")
        );
        Ok(())
    }

    fn assert_no_write_temporary_files(notes_dir: &Path) -> Result<()> {
        for entry in fs::read_dir(notes_dir)? {
            let path = entry?.path();
            assert_ne!(
                path.extension().and_then(|extension| extension.to_str()),
                Some("tmp"),
                "temporary file was left behind: {}",
                path.to_string_lossy()
            );
        }

        Ok(())
    }
}
