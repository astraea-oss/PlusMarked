<script lang="ts">
  import { createNote, getNote, listNotes, openWorkspace, saveNote } from '$lib/api';
  import type { NoteDocument, NoteSummary, WorkspaceSummary } from '$lib/types';

  let workspacePath = '';
  let workspace: WorkspaceSummary | null = null;
  let notes: NoteSummary[] = [];
  let selectedNote: NoteDocument | null = null;
  let status = 'Choose or type a workspace path to begin.';
  let saving = false;

  $: selectedId = selectedNote?.frontmatter.id;
  $: tagText = selectedNote?.frontmatter.tags?.join(', ') ?? '';
  $: aliasText = selectedNote?.frontmatter.aliases?.join(', ') ?? '';

  async function openCurrentWorkspace() {
    if (!workspacePath.trim()) {
      status = 'Enter a local workspace path.';
      return;
    }

    workspace = await openWorkspace(workspacePath.trim());
    notes = await listNotes();
    selectedNote = null;
    status = `Opened ${workspace.root}`;
  }

  async function createNewNote() {
    const note = await createNote('Untitled');
    notes = await listNotes();
    await selectNote(note.id);
    status = 'Created note.';
  }

  async function selectNote(id: string) {
    selectedNote = await getNote(id);
    status = 'Note loaded.';
  }

  async function saveSelectedNote() {
    if (!selectedNote) return;

    saving = true;
    const result = await saveNote({
      id: selectedNote.frontmatter.id,
      title: selectedNote.frontmatter.title,
      note_type: selectedNote.frontmatter.type || 'note',
      tags: splitList(tagText),
      aliases: splitList(aliasText),
      body: selectedNote.body
    });
    notes = await listNotes();
    await selectNote(result.note.id);
    status = 'Saved.';
    saving = false;
  }

  function splitList(input: string): string[] {
    return input
      .split(',')
      .map((item) => item.trim())
      .filter(Boolean);
  }
</script>

<svelte:head>
  <title>MarkdownPlus</title>
</svelte:head>

<main class="app-shell">
  <aside class="sidebar">
    <div class="brand">
      <div>
        <h1>MarkdownPlus</h1>
        <p>Local `.mdp` workspace</p>
      </div>
    </div>

    <section class="workspace-panel">
      <label for="workspace-path">Workspace path</label>
      <div class="workspace-row">
        <input
          id="workspace-path"
          bind:value={workspacePath}
          placeholder="/home/lua/MarkdownPlus Workspace"
        />
        <button class="primary" on:click={openCurrentWorkspace}>Open</button>
      </div>
    </section>

    {#if workspace}
      <section class="notes-header">
        <div>
          <span class="count">{notes.length}</span>
          <span>notes</span>
        </div>
        <button on:click={createNewNote}>New</button>
      </section>

      <nav class="notes-list" aria-label="Notes">
        {#each notes as note}
          <button
            class:active={note.id === selectedId}
            class="note-row"
            on:click={() => selectNote(note.id)}
          >
            <span class="note-title">{note.title}</span>
            <span class="note-meta">{note.note_type} · {new Date(note.updated_at).toLocaleString()}</span>
          </button>
        {/each}
      </nav>
    {/if}
  </aside>

  <section class="editor">
    {#if selectedNote}
      <header class="editor-header">
        <div>
          <input
            class="title-input"
            bind:value={selectedNote.frontmatter.title}
            aria-label="Note title"
          />
          <p>{selectedNote.frontmatter.id}</p>
        </div>
        <button class="primary" disabled={saving} on:click={saveSelectedNote}>
          {saving ? 'Saving' : 'Save'}
        </button>
      </header>

      <div class="properties">
        <label>
          Type
          <input bind:value={selectedNote.frontmatter.type} />
        </label>
        <label>
          Tags
          <input bind:value={tagText} placeholder="comma, separated" />
        </label>
        <label>
          Aliases
          <input bind:value={aliasText} placeholder="comma, separated" />
        </label>
      </div>

      <textarea class="body-editor" bind:value={selectedNote.body} aria-label="Markdown body" />
    {:else}
      <div class="empty-state">
        <h2>Open a workspace</h2>
        <p>{status}</p>
      </div>
    {/if}

    <footer>{status}</footer>
  </section>
</main>

<style>
  .app-shell {
    display: grid;
    grid-template-columns: minmax(280px, 340px) minmax(0, 1fr);
    min-height: 100vh;
  }

  .sidebar {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    border-right: 1px solid #d8e0e6;
    background: #edf2f3;
    padding: 1rem;
  }

  .brand h1 {
    margin: 0;
    font-size: 1.25rem;
  }

  .brand p,
  .editor-header p,
  .empty-state p,
  footer {
    margin: 0.25rem 0 0;
    color: #5c6670;
    font-size: 0.86rem;
  }

  .workspace-panel {
    display: grid;
    gap: 0.45rem;
  }

  .workspace-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 0.5rem;
  }

  .notes-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
  }

  .count {
    font-weight: 700;
  }

  .notes-list {
    display: grid;
    align-content: start;
    gap: 0.35rem;
    overflow: auto;
  }

  .note-row {
    display: grid;
    gap: 0.2rem;
    width: 100%;
    text-align: left;
    background: transparent;
  }

  .note-row.active {
    background: #ffffff;
    border-color: #285e61;
  }

  .note-title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 650;
  }

  .note-meta {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: #66727f;
    font-size: 0.8rem;
  }

  .editor {
    display: grid;
    grid-template-rows: auto auto minmax(0, 1fr) auto;
    gap: 1rem;
    padding: 1rem;
    min-width: 0;
  }

  .editor-header {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: start;
    gap: 1rem;
  }

  .title-input {
    border: 0;
    border-bottom: 1px solid #c7d0d8;
    border-radius: 0;
    padding-left: 0;
    background: transparent;
    font-size: 1.55rem;
    font-weight: 700;
  }

  .properties {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 0.75rem;
  }

  .properties label {
    display: grid;
    gap: 0.35rem;
    color: #43505d;
    font-size: 0.84rem;
    font-weight: 650;
  }

  .body-editor {
    min-height: 0;
    height: 100%;
    line-height: 1.5;
    font-family:
      "SFMono-Regular", Consolas, "Liberation Mono", Menlo, ui-monospace, monospace;
  }

  .empty-state {
    display: grid;
    place-content: center;
    min-height: 100%;
    text-align: center;
  }

  .empty-state h2 {
    margin: 0 0 0.35rem;
  }

  footer {
    border-top: 1px solid #d8e0e6;
    padding-top: 0.75rem;
  }

  @media (max-width: 760px) {
    .app-shell {
      grid-template-columns: 1fr;
    }

    .sidebar {
      border-right: 0;
      border-bottom: 1px solid #d8e0e6;
    }

    .properties {
      grid-template-columns: 1fr;
    }
  }
</style>

