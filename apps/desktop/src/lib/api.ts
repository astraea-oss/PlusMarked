import { invoke } from '@tauri-apps/api/core';
import type { NoteDocument, NoteSummary, SaveNoteInput, WorkspaceSummary } from './types';

export function openWorkspace(path: string): Promise<WorkspaceSummary> {
  return invoke('open_workspace', { path });
}

export function createNote(title?: string): Promise<NoteSummary> {
  return invoke('create_note', { input: { title: title || 'Untitled', note_type: 'note' } });
}

export function listNotes(): Promise<NoteSummary[]> {
  return invoke('list_notes');
}

export function getNote(id: string): Promise<NoteDocument> {
  return invoke('get_note', { id });
}

export function saveNote(input: SaveNoteInput): Promise<{ note: NoteSummary }> {
  return invoke('save_note', { input });
}

