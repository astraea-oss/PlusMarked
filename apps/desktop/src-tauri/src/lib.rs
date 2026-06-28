use std::sync::Mutex;

use mdp_core::NoteDocument;
use mdp_workspace::{
    CreateNoteInput, NoteSummary, SaveNoteInput, SaveResult, WorkspaceHandle, WorkspaceSummary,
};
use tauri::State;

#[derive(Default)]
struct AppState {
    workspace: Mutex<Option<WorkspaceHandle>>,
}

#[tauri::command]
fn open_workspace(path: String, state: State<'_, AppState>) -> Result<WorkspaceSummary, String> {
    let workspace = WorkspaceHandle::open(path).map_err(to_command_error)?;
    let summary = workspace.summary().map_err(to_command_error)?;
    *state.workspace.lock().map_err(lock_error)? = Some(workspace);
    Ok(summary)
}

#[tauri::command]
fn create_note(
    input: CreateNoteInput,
    state: State<'_, AppState>,
) -> Result<NoteSummary, String> {
    let guard = state.workspace.lock().map_err(lock_error)?;
    let workspace = guard.as_ref().ok_or("open a workspace first")?;
    workspace.create_note(input).map_err(to_command_error)
}

#[tauri::command]
fn list_notes(state: State<'_, AppState>) -> Result<Vec<NoteSummary>, String> {
    let guard = state.workspace.lock().map_err(lock_error)?;
    let workspace = guard.as_ref().ok_or("open a workspace first")?;
    workspace.list_notes().map_err(to_command_error)
}

#[tauri::command]
fn get_note(id: String, state: State<'_, AppState>) -> Result<NoteDocument, String> {
    let guard = state.workspace.lock().map_err(lock_error)?;
    let workspace = guard.as_ref().ok_or("open a workspace first")?;
    workspace.get_note(&id).map_err(to_command_error)
}

#[tauri::command]
fn save_note(input: SaveNoteInput, state: State<'_, AppState>) -> Result<SaveResult, String> {
    let guard = state.workspace.lock().map_err(lock_error)?;
    let workspace = guard.as_ref().ok_or("open a workspace first")?;
    workspace.save_note(input).map_err(to_command_error)
}

pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            open_workspace,
            create_note,
            list_notes,
            get_note,
            save_note
        ])
        .run(tauri::generate_context!())
        .expect("error while running MarkdownPlus");
}

fn to_command_error(error: anyhow::Error) -> String {
    error.to_string()
}

fn lock_error<T>(error: std::sync::PoisonError<T>) -> String {
    format!("application state lock failed: {error}")
}
