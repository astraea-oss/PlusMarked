<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import {
    CalendarClock,
    CalendarDays,
    Cog,
    FilePlus,
    FolderOpen,
    Hash,
    List,
    ListTree,
    NotebookText,
    Table,
    SlidersHorizontal,
    SquareCheck,
    Tags,
    Type,
    X
  } from '@lucide/svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import appIconUrl from '../../src-tauri/icons/icon.png';
  import DOMPurify from 'dompurify';
  import { marked } from 'marked';
  import BasesView from '$lib/BasesView.svelte';
  import MarkdownPlusEditor from '$lib/MarkdownPlusEditor.svelte';
  import {
    createBase,
    createNote,
    deleteNote,
    getAppSettings,
    getNoteSource,
    listNotes,
    openWorkspace,
    saveNoteSource,
    selectWorkspaceDirectory,
    updateAppSettings
  } from '$lib/api';
  import type {
    DockSide,
    NoteSource,
    NoteSummary,
    PanelMode,
    ToolAnchor,
    WorkspaceSummary
  } from '$lib/types';

  type EditorMode = 'live' | 'source' | 'split' | 'preview';
  type RibbonToolId = 'notes' | 'new-note' | 'settings' | 'outline';
  type RibbonTool = {
    id: RibbonToolId;
    label: string;
    dock: DockSide;
    anchor: ToolAnchor;
    order: number;
  };
  type ToolDocks = Record<RibbonToolId, DockSide>;
  type ToolAnchors = Record<RibbonToolId, ToolAnchor>;
  type ToolOrders = Record<RibbonToolId, number>;
  type ToolZone = {
    anchor: ToolAnchor;
    tools: RibbonTool[];
  };
  type PointerToolDrag = {
    tool: RibbonToolId;
    pointerId: number;
    startX: number;
    startY: number;
    dragging: boolean;
  };
  type HudToolId = 'notes' | 'outline';
  type HudHeights = Record<HudToolId, number>;
  type PropertyRow = {
    key: string;
    value: string;
  };
  type PropertyDisplay = {
    text: string;
    formatted: boolean;
  };
  type PropertyType = 'checkbox' | 'date' | 'datetime' | 'list' | 'number' | 'tags' | 'text';
  type PropertyTypeOption = {
    id: PropertyType;
    label: string;
  };
  type OutlineItem = {
    level: number;
    text: string;
  };
  type NoteContextMenu = {
    note: NoteSummary;
    x: number;
    y: number;
  };

  const minLeftPanelWidth = 150;
  const maxLeftPanelWidth = 420;
  const minRightPanelWidth = 150;
  const maxRightPanelWidth = 420;
  const ribbonPanelWidth = 52;
  const minHudHeight = 64;
  const maxHudHeight = 520;
  const defaultToolDocks: ToolDocks = {
    notes: 'left',
    'new-note': 'left',
    settings: 'left',
    outline: 'right'
  };
  const defaultToolAnchors: ToolAnchors = {
    notes: 'top',
    'new-note': 'top',
    settings: 'bottom',
    outline: 'top'
  };
  const defaultToolOrders: ToolOrders = {
    notes: 10,
    'new-note': 20,
    settings: 30,
    outline: 40
  };
  const defaultHudHeights: HudHeights = {
    notes: 220,
    outline: 120
  };
  const systemPropertyLabels: Record<string, string> = {
    id: 'ID',
    title: 'Title',
    created_at: 'Created',
    updated_at: 'Modified',
    tags: 'Tags',
    aliases: 'Aliases',
    type: 'Type'
  };
  const propertyTypeOptions: PropertyTypeOption[] = [
    { id: 'checkbox', label: 'Checkbox' },
    { id: 'date', label: 'Date' },
    { id: 'datetime', label: 'Date & time' },
    { id: 'list', label: 'List' },
    { id: 'number', label: 'Number' },
    { id: 'tags', label: 'Tags' },
    { id: 'text', label: 'Text' }
  ];
  const enclosingPairs: Record<string, string> = {
    '(': ')',
    '[': ']',
    '{': '}',
    '"': '"',
    "'": "'",
    '`': '`',
    '<': '>'
  };
  const closingPairs = new Set(Object.values(enclosingPairs));

  let leftPanelWidth = 285;
  let rightPanelWidth = 255;
  let leftPanelMode: PanelMode = 'view';
  let rightPanelMode: PanelMode = 'view';
  let toolDocks: ToolDocks = { ...defaultToolDocks };
  let toolAnchors: ToolAnchors = { ...defaultToolAnchors };
  let toolOrders: ToolOrders = { ...defaultToolOrders };
  let draggingTool: RibbonToolId | null = null;
  let pointerToolDrag: PointerToolDrag | null = null;
  let suppressNextToolClick = false;
  let lastWindowBadgeClick: { time: number; x: number; y: number } | null = null;
  let tokenPropertyDrafts: Record<number, string> = {};
  let propertyTypeMenuIndex: number | null = null;
  let propertyTypeOverrides: Record<string, PropertyType> = {};
  let hudHeights: HudHeights = { ...defaultHudHeights };
  let workspacePath = '';
  let workspace: WorkspaceSummary | null = null;
  let notes: NoteSummary[] = [];
  let selectedNoteSource: NoteSource | null = null;
  let noteSource = '';
  let liveBody = '';
  let propertyRows: PropertyRow[] = [];
  let status = 'Choose or type a workspace path to begin.';
  let portableRoot = '';
  let saving = false;
  let autosaveTimer: ReturnType<typeof setTimeout> | null = null;
  let saveInFlight: Promise<boolean> | null = null;
  let lastSavedNoteId: string | null = null;
  let lastSavedSource = '';
  let browsing = false;
  let settingsOpen = false;
  let editorMode: EditorMode = 'live';
  let noteContextMenu: NoteContextMenu | null = null;
  let noteContextMenuElement: HTMLDivElement;

  $: selectedId = selectedNoteSource?.id;
  $: selectedDocument = notes.find((note) => note.id === selectedId) ?? null;
  $: selectedTitle = selectedDocument?.title ?? 'Untitled';
  $: selectedIsBase = selectedDocument?.note_type === 'base';
  $: leftPanelColumnWidth = leftPanelMode === 'ribbon' ? ribbonPanelWidth : leftPanelWidth;
  $: rightPanelColumnWidth = rightPanelMode === 'ribbon' ? ribbonPanelWidth : rightPanelWidth;
  $: ribbonTools = buildRibbonTools(toolDocks, toolAnchors, toolOrders);
  $: leftRibbonTools = ribbonTools.filter((tool) => tool.dock === 'left');
  $: rightRibbonTools = ribbonTools.filter((tool) => tool.dock === 'right');
  $: leftToolZones = buildToolZones(leftRibbonTools);
  $: rightToolZones = buildToolZones(rightRibbonTools);
  $: leftHasHudTool = hasHudTool(leftRibbonTools);
  $: rightHasHudTool = hasHudTool(rightRibbonTools);
  $: outlineItems = extractOutline(extractMarkdownBody(noteSource));
  $: markdownHtml = DOMPurify.sanitize(
    marked.parse(markdownPlusPreviewSource(extractMarkdownBody(noteSource)), {
      async: false,
      breaks: true
    }) as string
  );

  onMount(async () => {
    window.addEventListener('pointerdown', handleGlobalPointerDown);
    window.addEventListener('keydown', handleGlobalKeydown);

    const settings = await getAppSettings();
    applyPersistedSettings(settings);

    if (settings.last_workspace_path) {
      workspacePath = settings.last_workspace_path;
      try {
        await openWorkspacePath(settings.last_workspace_path);
      } catch (error) {
        settingsOpen = true;
        status = error instanceof Error ? error.message : String(error);
      }
    } else {
      settingsOpen = true;
      status = `Portable data: ${settings.portable_root}`;
    }
  });

  onDestroy(() => {
    if (autosaveTimer) {
      clearTimeout(autosaveTimer);
    }

    removeToolPointerListeners();
    window.removeEventListener('pointerdown', handleGlobalPointerDown);
    window.removeEventListener('keydown', handleGlobalKeydown);
  });

  async function openCurrentWorkspace() {
    if (!workspacePath.trim()) {
      status = 'Enter a local workspace path.';
      return;
    }

    await openWorkspacePath(workspacePath.trim());
  }

  async function browseWorkspace() {
    browsing = true;
    const selected = await selectWorkspaceDirectory(workspacePath.trim());
    browsing = false;

    if (!selected) {
      status = 'Workspace selection cancelled.';
      return;
    }

    workspacePath = selected;
    await openWorkspacePath(selected);
  }

  async function openWorkspacePath(path: string) {
    if (!(await flushPendingAutosave())) return;

    workspace = await openWorkspace(path);
    notes = await listNotes();
    clearAutosaveState();
    selectedNoteSource = null;
    noteSource = '';
    editorMode = 'live';
    settingsOpen = false;
    status = `Opened ${workspace.root}`;
  }

  async function createNewNote() {
    closeNoteContextMenu();
    if (!(await flushPendingAutosave())) return;

    const note = await createNote('Untitled');
    notes = await listNotes();
    editorMode = 'live';
    await selectNote(note.id);
    status = 'Created note.';
  }

  async function createNewBase() {
    closeNoteContextMenu();
    if (!(await flushPendingAutosave())) return;

    const base = await createBase('Untitled base');
    notes = await listNotes();
    editorMode = 'live';
    await selectNote(base.id);
    status = 'Created base.';
  }

  function openNoteContextMenu(event: MouseEvent, note: NoteSummary) {
    event.preventDefault();
    event.stopPropagation();

    const menuWidth = 190;
    const menuHeight = 194;
    noteContextMenu = {
      note,
      x: Math.min(event.clientX, Math.max(0, window.innerWidth - menuWidth - 8)),
      y: Math.min(event.clientY, Math.max(0, window.innerHeight - menuHeight - 8))
    };
  }

  function closeNoteContextMenu() {
    noteContextMenu = null;
  }

  function handleGlobalPointerDown(event: PointerEvent) {
    if (!noteContextMenu) return;
    if (event.target instanceof Node && noteContextMenuElement?.contains(event.target)) return;
    closeNoteContextMenu();
  }

  function handleGlobalKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') closeNoteContextMenu();
  }

  async function openContextMenuNote() {
    const note = noteContextMenu?.note;
    closeNoteContextMenu();
    if (!note) return;

    settingsOpen = false;
    editorMode = 'live';
    await selectNote(note.id);
  }

  async function copyContextMenuValue(value: string, label: string) {
    closeNoteContextMenu();
    try {
      await navigator.clipboard.writeText(value);
      status = `${label} copied.`;
    } catch {
      status = `Could not copy ${label.toLowerCase()}.`;
    }
  }

  async function copyContextMenuPath() {
    const note = noteContextMenu?.note;
    if (!note) return;
    await copyContextMenuValue(note.path, 'Path');
  }

  async function copyContextMenuId() {
    const note = noteContextMenu?.note;
    if (!note) return;
    await copyContextMenuValue(note.id, 'ID');
  }

  async function deleteContextMenuNote() {
    const note = noteContextMenu?.note;
    closeNoteContextMenu();
    if (!note) return;

    const documentType = note.note_type === 'base' ? 'base' : 'note';
    if (!window.confirm(`Delete ${documentType} "${note.title}"? This removes it from the workspace.`)) {
      return;
    }

    try {
      const wasSelected = selectedId === note.id;
      const deletedIndex = notes.findIndex((candidate) => candidate.id === note.id);

      if (wasSelected) {
        clearAutosaveState();
        selectedNoteSource = null;
        noteSource = '';
        liveBody = '';
        propertyRows = [];
      }

      await deleteNote(note.id);
      notes = await listNotes();

      if (wasSelected && notes.length) {
        const nextIndex = Math.min(Math.max(deletedIndex, 0), notes.length - 1);
        await selectNote(notes[nextIndex].id);
      }

      status = `Deleted ${documentType}.`;
    } catch (error) {
      status = error instanceof Error ? error.message : String(error);
    }
  }

  async function selectNote(id: string) {
    if (selectedNoteSource && selectedNoteSource.id !== id && !(await flushPendingAutosave())) {
      return;
    }

    clearAutosaveState();
    selectedNoteSource = await getNoteSource(id);
    noteSource = selectedNoteSource.source;
    lastSavedNoteId = id;
    lastSavedSource = noteSource;
    if (notes.find((note) => note.id === id)?.note_type === 'base') {
      liveBody = '';
      propertyRows = [];
      status = 'Base loaded.';
      return;
    }

    syncLiveFieldsFromSource();
    status = 'Note loaded.';
  }

  async function saveSelectedNote(force = false): Promise<boolean> {
    if (!selectedNoteSource) return true;
    if (selectedIsBase) return true;

    if (saveInFlight) {
      await saveInFlight;
      return saveSelectedNote(force);
    }

    if (editorMode === 'live') {
      updateSourceFromLiveFields(false);
    }

    const noteId = selectedNoteSource.id;
    const sourceToSave = noteSource;
    if (!force && noteId === lastSavedNoteId && sourceToSave === lastSavedSource) {
      return true;
    }

    const saveRequest = performNoteSave(noteId, sourceToSave);
    saveInFlight = saveRequest;
    try {
      return await saveRequest;
    } finally {
      if (saveInFlight === saveRequest) {
        saveInFlight = null;
      }
    }
  }

  async function performNoteSave(noteId: string, sourceToSave: string): Promise<boolean> {
    saving = true;
    status = 'Saving...';

    try {
      const result = await saveNoteSource({
        id: noteId,
        source: sourceToSave
      });

      notes = upsertNoteSummary(notes, result.note);

      if (selectedNoteSource?.id === noteId) {
        selectedNoteSource = { ...selectedNoteSource, source: sourceToSave };
        lastSavedNoteId = noteId;
        lastSavedSource = sourceToSave;

        if (noteSource === sourceToSave) {
          status = 'Saved.';
        } else {
          scheduleAutosave();
        }
      }

      return true;
    } catch (error) {
      status = error instanceof Error ? error.message : String(error);
      return false;
    } finally {
      saving = false;
    }
  }

  function upsertNoteSummary(items: NoteSummary[], note: NoteSummary): NoteSummary[] {
    if (!items.some((item) => item.id === note.id)) {
      return [note, ...items];
    }

    return items.map((item) => (item.id === note.id ? note : item));
  }

  function scheduleAutosave(delay = 700) {
    if (!selectedNoteSource) return;

    if (autosaveTimer) {
      clearTimeout(autosaveTimer);
    }

    if (selectedNoteSource.id === lastSavedNoteId && noteSource === lastSavedSource) {
      status = 'Saved.';
      autosaveTimer = null;
      return;
    }

    status = saving ? 'Saving...' : 'Unsaved changes.';
    autosaveTimer = setTimeout(() => {
      autosaveTimer = null;
      void saveSelectedNote();
    }, delay);
  }

  async function flushPendingAutosave(): Promise<boolean> {
    if (autosaveTimer) {
      clearTimeout(autosaveTimer);
      autosaveTimer = null;
    }

    return saveSelectedNote();
  }

  function clearAutosaveState() {
    if (autosaveTimer) {
      clearTimeout(autosaveTimer);
      autosaveTimer = null;
    }

    saveInFlight = null;
    lastSavedNoteId = null;
    lastSavedSource = '';
    saving = false;
  }

  function extractMarkdownBody(source: string): string {
    if (!source.startsWith('---')) {
      return source;
    }

    const delimiter = source.indexOf('\n---', 3);
    if (delimiter === -1) {
      return '';
    }

    return source.slice(delimiter + 4).replace(/^\r?\n+/, '');
  }

  function markdownPlusPreviewSource(source: string): string {
    return preserveMarkdownBlankLines(renderInlineTags(renderWikiLinks(source)))
      .replace(/^[ \t]*-{3,}[ \t]*$/gm, '\n<hr data-mdp-rule="underline">\n');
  }

  function preserveMarkdownBlankLines(source: string): string {
    const lines = source.split(/\r?\n/);
    let inFence = false;

    return lines
      .map((line) => {
        if (/^\s*(```|~~~)/.test(line)) {
          inFence = !inFence;
          return line;
        }

        if (!inFence && line.trim() === '') {
          return '<div class="mdp-blank-line" aria-hidden="true"></div>';
        }

        return line;
      })
      .join('\n');
  }

  function renderWikiLinks(source: string): string {
    return source.replace(/\[\[([^\]\n]+)\]\]/g, (_match, rawLink: string) => {
      const [rawTarget, rawLabel] = rawLink.split('|');
      const target = rawTarget.trim();
      const label = (rawLabel ?? rawTarget).trim();
      if (!target) return _match;

      return `<a href="#${encodeURIComponent(target)}" class="mdp-internal-link" data-mdp-internal-link="${escapeHtml(target)}">${escapeHtml(label)}</a>`;
    });
  }

  function escapeHtml(value: string) {
    return value
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;')
      .replace(/'/g, '&#39;');
  }

  function renderInlineTags(source: string): string {
    return source.replace(/(^|[\s([{>])#([A-Za-z0-9_/-]+)/g, (match, prefix: string, tag: string, offset: number, full: string) => {
      if (offset === 0 && prefix === '' && /^#{1,6}\s/.test(full.slice(offset))) {
        return match;
      }

      return `${prefix}<span class="mdp-inline-tag">${escapeHtml(tag)}</span>`;
    });
  }

  function extractOutline(source: string): OutlineItem[] {
    return source
      .split(/\r?\n/)
      .map((line) => line.match(/^\s*(#{1,3})\s+(.+)$/))
      .filter((match): match is RegExpMatchArray => Boolean(match))
      .map((match) => ({
        level: match[1].length,
        text: match[2].replace(/[*_`[\]]/g, '').trim()
      }))
      .filter((item) => item.text);
  }

  function startLeftPanelResize(event: PointerEvent) {
    if (leftPanelMode === 'ribbon') return;
    event.preventDefault();

    const startX = event.clientX;
    const startWidth = leftPanelWidth;
    document.body.classList.add('is-resizing-panel');

    const resizePanel = (moveEvent: PointerEvent) => {
      leftPanelWidth = clamp(startWidth + moveEvent.clientX - startX, minLeftPanelWidth, maxLeftPanelWidth);
    };

    const stopResize = () => {
      document.body.classList.remove('is-resizing-panel');
      window.removeEventListener('pointermove', resizePanel);
      window.removeEventListener('pointerup', stopResize);
      window.removeEventListener('pointercancel', stopResize);
      persistUiSettingsInBackground();
    };

    window.addEventListener('pointermove', resizePanel);
    window.addEventListener('pointerup', stopResize);
    window.addEventListener('pointercancel', stopResize);
  }

  function handleLeftPanelResizeKeydown(event: KeyboardEvent) {
    if (leftPanelMode === 'ribbon') return;

    if (event.key === 'ArrowLeft') {
      event.preventDefault();
      leftPanelWidth = clamp(leftPanelWidth - 12, minLeftPanelWidth, maxLeftPanelWidth);
      persistUiSettingsInBackground();
    }

    if (event.key === 'ArrowRight') {
      event.preventDefault();
      leftPanelWidth = clamp(leftPanelWidth + 12, minLeftPanelWidth, maxLeftPanelWidth);
      persistUiSettingsInBackground();
    }
  }

  function startRightPanelResize(event: PointerEvent) {
    if (rightPanelMode === 'ribbon') return;
    event.preventDefault();

    const startX = event.clientX;
    const startWidth = rightPanelWidth;
    document.body.classList.add('is-resizing-panel');

    const resizePanel = (moveEvent: PointerEvent) => {
      rightPanelWidth = clamp(startWidth - (moveEvent.clientX - startX), minRightPanelWidth, maxRightPanelWidth);
    };

    const stopResize = () => {
      document.body.classList.remove('is-resizing-panel');
      window.removeEventListener('pointermove', resizePanel);
      window.removeEventListener('pointerup', stopResize);
      window.removeEventListener('pointercancel', stopResize);
      persistUiSettingsInBackground();
    };

    window.addEventListener('pointermove', resizePanel);
    window.addEventListener('pointerup', stopResize);
    window.addEventListener('pointercancel', stopResize);
  }

  function handleRightPanelResizeKeydown(event: KeyboardEvent) {
    if (rightPanelMode === 'ribbon') return;

    if (event.key === 'ArrowLeft') {
      event.preventDefault();
      rightPanelWidth = clamp(rightPanelWidth + 12, minRightPanelWidth, maxRightPanelWidth);
      persistUiSettingsInBackground();
    }

    if (event.key === 'ArrowRight') {
      event.preventDefault();
      rightPanelWidth = clamp(rightPanelWidth - 12, minRightPanelWidth, maxRightPanelWidth);
      persistUiSettingsInBackground();
    }
  }

  function startHudResize(event: PointerEvent, tool: HudToolId) {
    event.preventDefault();
    event.stopPropagation();

    const startY = event.clientY;
    const startHeight = hudHeights[tool];
    document.body.classList.add('is-resizing-hud');

    const resizeHud = (moveEvent: PointerEvent) => {
      setHudHeight(tool, startHeight + moveEvent.clientY - startY);
    };

    const stopResize = () => {
      document.body.classList.remove('is-resizing-hud');
      window.removeEventListener('pointermove', resizeHud);
      window.removeEventListener('pointerup', stopResize);
      window.removeEventListener('pointercancel', stopResize);
      persistUiSettingsInBackground();
    };

    window.addEventListener('pointermove', resizeHud);
    window.addEventListener('pointerup', stopResize);
    window.addEventListener('pointercancel', stopResize);
  }

  function handleHudResizeKeydown(event: KeyboardEvent, tool: HudToolId) {
    if (event.key === 'ArrowUp') {
      event.preventDefault();
      setHudHeight(tool, hudHeights[tool] - 16);
      persistUiSettingsInBackground();
    }

    if (event.key === 'ArrowDown') {
      event.preventDefault();
      setHudHeight(tool, hudHeights[tool] + 16);
      persistUiSettingsInBackground();
    }
  }

  function setHudHeight(tool: HudToolId, height: number) {
    hudHeights = {
      ...hudHeights,
      [tool]: clamp(Math.round(height), minHudHeight, maxHudHeight)
    };
  }

  function clamp(value: number, min: number, max: number): number {
    return Math.min(max, Math.max(min, value));
  }

  function normalizePanelMode(mode: string): PanelMode {
    return mode === 'ribbon' ? 'ribbon' : 'view';
  }

  function normalizeToolAnchor(anchor: string): ToolAnchor {
    if (anchor === 'center' || anchor === 'bottom') return anchor;
    return 'top';
  }

  function normalizeDockSide(side: string): DockSide {
    return side === 'right' ? 'right' : 'left';
  }

  function applyPersistedSettings(settings: Awaited<ReturnType<typeof getAppSettings>>) {
    portableRoot = settings.portable_root;
    leftPanelWidth = clamp(settings.left_panel_width, minLeftPanelWidth, maxLeftPanelWidth);
    rightPanelWidth = clamp(settings.right_panel_width, minRightPanelWidth, maxRightPanelWidth);
    leftPanelMode = normalizePanelMode(settings.left_panel_mode);
    rightPanelMode = normalizePanelMode(settings.right_panel_mode);
    toolDocks = {
      notes: normalizeDockSide(settings.notes_dock),
      'new-note': normalizeDockSide(settings.new_note_dock),
      settings: normalizeDockSide(settings.settings_dock),
      outline: normalizeDockSide(settings.outline_dock)
    };
    toolAnchors = {
      notes: normalizeToolAnchor(settings.notes_anchor),
      'new-note': normalizeToolAnchor(settings.new_note_anchor),
      settings: normalizeToolAnchor(settings.settings_anchor),
      outline: normalizeToolAnchor(settings.outline_anchor)
    };
    toolOrders = {
      notes: settings.notes_order,
      'new-note': settings.new_note_order,
      settings: settings.settings_order,
      outline: settings.outline_order
    };
    hudHeights = {
      notes: clamp(settings.notes_hud_height, minHudHeight, maxHudHeight),
      outline: clamp(settings.outline_hud_height, minHudHeight, maxHudHeight)
    };
  }

  function buildRibbonTools(docks: ToolDocks, anchors: ToolAnchors, orders: ToolOrders): RibbonTool[] {
    return [
      { id: 'notes', label: 'Notes', dock: docks.notes, anchor: anchors.notes, order: orders.notes },
      { id: 'new-note', label: 'New note', dock: docks['new-note'], anchor: anchors['new-note'], order: orders['new-note'] },
      { id: 'outline', label: 'Outline', dock: docks.outline, anchor: anchors.outline, order: orders.outline }
    ];
  }

  function buildToolZones(tools: RibbonTool[]): ToolZone[] {
    return [
      { anchor: 'top', tools: sortTools(tools.filter((tool) => tool.anchor === 'top')) },
      { anchor: 'center', tools: sortTools(tools.filter((tool) => tool.anchor === 'center')) },
      { anchor: 'bottom', tools: sortTools(tools.filter((tool) => tool.anchor === 'bottom')) }
    ];
  }

  function sortTools(tools: RibbonTool[]): RibbonTool[] {
    return [...tools].sort((first, second) => first.order - second.order || first.label.localeCompare(second.label));
  }

  async function setPanelMode(panel: 'left' | 'right', mode: PanelMode) {
    const previousLeftPanelMode = leftPanelMode;
    const previousRightPanelMode = rightPanelMode;

    if (panel === 'left') {
      leftPanelMode = mode;
    } else {
      rightPanelMode = mode;
    }

    try {
      await persistUiSettings();
    } catch (error) {
      leftPanelMode = previousLeftPanelMode;
      rightPanelMode = previousRightPanelMode;
      status = error instanceof Error ? error.message : String(error);
    }
  }

  async function setToolDock(tool: RibbonToolId, side: DockSide) {
    await moveTool(tool, side, toolAnchors[tool], undefined, 'append', `${toolLabel(tool)} moved to ${side} panel.`);
  }

  async function setToolAnchor(tool: RibbonToolId, anchor: ToolAnchor) {
    await moveTool(tool, toolDocks[tool], anchor, undefined, 'append', `${toolLabel(tool)} anchored ${anchor}.`);
  }

  async function moveToolToOppositeDock(tool: RibbonToolId) {
    const currentDock = getToolDock(tool);
    await setToolDock(tool, currentDock === 'left' ? 'right' : 'left');
  }

  function getToolDock(tool: RibbonToolId): DockSide {
    return toolDocks[tool];
  }

  function hasHudTool(tools: RibbonTool[]): boolean {
    return tools.some((tool) => tool.id === 'notes' || tool.id === 'outline');
  }

  function toolLabel(tool: RibbonToolId): string {
    if (tool === 'new-note') return 'New note';
    return tool.charAt(0).toUpperCase() + tool.slice(1);
  }

  function startToolPointerDrag(event: PointerEvent, tool: RibbonToolId) {
    if (event.button !== 0 || !event.isPrimary) return;

    pointerToolDrag = {
      tool,
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      dragging: false
    };
    window.addEventListener('pointermove', handleToolPointerMove);
    window.addEventListener('pointerup', handleToolPointerUp);
    window.addEventListener('pointercancel', handleToolPointerCancel);
  }

  function handleToolPointerMove(event: PointerEvent) {
    const drag = pointerToolDrag;
    if (!drag || event.pointerId !== drag.pointerId) return;

    const distance = Math.hypot(event.clientX - drag.startX, event.clientY - drag.startY);
    if (!drag.dragging && distance < 4) return;

    if (!drag.dragging) {
      drag.dragging = true;
      draggingTool = drag.tool;
      document.body.classList.add('is-moving-tool');
      status = `Moving ${toolLabel(drag.tool)}.`;
    }

    event.preventDefault();
  }

  function handleToolPointerUp(event: PointerEvent) {
    const drag = pointerToolDrag;
    if (!drag || event.pointerId !== drag.pointerId) return;

    removeToolPointerListeners();
    pointerToolDrag = null;

    if (!drag.dragging) return;

    event.preventDefault();
    event.stopPropagation();
    suppressNextToolClick = true;
    setTimeout(() => {
      suppressNextToolClick = false;
    }, 0);
    void dropToolAtPoint(drag.tool, event.clientX, event.clientY);
  }

  function handleToolPointerCancel(event: PointerEvent) {
    const drag = pointerToolDrag;
    if (!drag || event.pointerId !== drag.pointerId) return;
    clearToolDragState();
  }

  function removeToolPointerListeners() {
    window.removeEventListener('pointermove', handleToolPointerMove);
    window.removeEventListener('pointerup', handleToolPointerUp);
    window.removeEventListener('pointercancel', handleToolPointerCancel);
  }

  async function dropToolAtPoint(tool: RibbonToolId, x: number, y: number) {
    const element = document.elementFromPoint(x, y);
    const dropTarget = element?.closest('[data-tool-drop-side]');

    if (!(dropTarget instanceof HTMLElement)) {
      status = `Drop ${toolLabel(tool)} on a tool area.`;
      clearToolDragState();
      return;
    }

    const side = parseDockSide(dropTarget.dataset.toolDropSide);
    if (!side) {
      status = `Drop ${toolLabel(tool)} on a tool area.`;
      clearToolDragState();
      return;
    }

    const anchor = parseToolAnchor(dropTarget.dataset.toolDropAnchor) ?? toolAnchors[tool];
    const targetTool = parseRibbonToolId(dropTarget.dataset.toolDropTarget);

    if (targetTool === tool) {
      clearToolDragState();
      return;
    }

    if (targetTool && targetTool !== tool) {
      const placement = y > dropTarget.getBoundingClientRect().top + dropTarget.getBoundingClientRect().height / 2
        ? 'after'
        : 'before';
      await moveTool(tool, side, anchor, targetTool, placement);
    } else {
      await moveTool(tool, side, anchor, undefined, 'append');
    }

    clearToolDragState();
  }

  function clearToolDragState() {
    removeToolPointerListeners();
    pointerToolDrag = null;
    draggingTool = null;
    document.body.classList.remove('is-moving-tool');
  }

  function parseDockSide(value: string | undefined): DockSide | null {
    return value === 'left' || value === 'right' ? value : null;
  }

  function parseToolAnchor(value: string | undefined): ToolAnchor | null {
    return value === 'top' || value === 'center' || value === 'bottom' ? value : null;
  }

  function parseRibbonToolId(value: string | undefined): RibbonToolId | null {
    return isRibbonToolId(value) ? value : null;
  }

  async function moveTool(
    tool: RibbonToolId,
    side: DockSide,
    anchor: ToolAnchor,
    targetTool: RibbonToolId | undefined,
    placement: 'before' | 'after' | 'append',
    successStatus = `${toolLabel(tool)} moved to ${anchor} ${side}.`
  ) {
    const previousToolDocks = toolDocks;
    const previousToolAnchors = toolAnchors;
    const previousToolOrders = toolOrders;
    const targetTools = sortTools(ribbonTools)
      .filter((candidate) => candidate.dock === side && candidate.anchor === anchor && candidate.id !== tool)
      .map((candidate) => candidate.id);

    if (targetTool && targetTool !== tool) {
      const targetIndex = targetTools.indexOf(targetTool);
      const insertAt = targetIndex === -1
        ? targetTools.length
        : targetIndex + (placement === 'after' ? 1 : 0);
      targetTools.splice(insertAt, 0, tool);
    } else {
      targetTools.push(tool);
    }

    toolDocks = { ...toolDocks, [tool]: side };
    toolAnchors = { ...toolAnchors, [tool]: anchor };
    toolOrders = { ...toolOrders, ...renumberTools(targetTools) };

    try {
      await persistUiSettings();
      status = successStatus;
    } catch (error) {
      toolDocks = previousToolDocks;
      toolAnchors = previousToolAnchors;
      toolOrders = previousToolOrders;
      status = error instanceof Error ? error.message : String(error);
    }
  }

  function renumberTools(toolIds: RibbonToolId[]): Partial<ToolOrders> {
    return Object.fromEntries(toolIds.map((tool, index) => [tool, (index + 1) * 10])) as Partial<ToolOrders>;
  }

  function isRibbonToolId(value: string | null | undefined): value is RibbonToolId {
    return value === 'notes'
      || value === 'new-note'
      || value === 'settings'
      || value === 'outline';
  }

  function runRibbonTool(tool: RibbonToolId) {
    if (tool === 'notes') {
      settingsOpen = false;
      return;
    }

    if (tool === 'new-note') {
      if (!workspace) {
        status = 'Open a workspace first.';
        return;
      }

      createNewNote();
      return;
    }

    if (tool === 'settings') {
      settingsOpen = true;
    }
  }

  function openSettings() {
    settingsOpen = true;
  }

  function handleToolClick(tool: RibbonToolId) {
    if (suppressNextToolClick) {
      suppressNextToolClick = false;
      return;
    }

    runRibbonTool(tool);
  }

  async function handleWindowBadgePointerDown(event: PointerEvent) {
    if (event.button !== 0 || !event.isPrimary) return;

    event.preventDefault();
    event.stopPropagation();

    const now = Date.now();
    const isDoubleClick = lastWindowBadgeClick
      && now - lastWindowBadgeClick.time < 500
      && Math.hypot(event.clientX - lastWindowBadgeClick.x, event.clientY - lastWindowBadgeClick.y) < 6;

    lastWindowBadgeClick = isDoubleClick ? null : { time: now, x: event.clientX, y: event.clientY };

    if (isDoubleClick) {
      await toggleWindowMaximize();
      return;
    }

    try {
      await getCurrentWindow().startDragging();
    } catch (error) {
      console.error('Failed to start window drag', error);
      status = 'Window drag failed.';
    }
  }

  async function toggleWindowMaximize() {
    try {
      await getCurrentWindow().toggleMaximize();
    } catch (error) {
      console.error('Failed to toggle window maximize', error);
      status = 'Window maximize failed.';
    }
  }

  async function persistUiSettings() {
    const settings = await updateAppSettings({
      left_panel_width: Math.round(leftPanelWidth),
      right_panel_width: Math.round(rightPanelWidth),
      left_panel_mode: leftPanelMode,
      right_panel_mode: rightPanelMode,
      notes_dock: toolDocks.notes,
      new_note_dock: toolDocks['new-note'],
      settings_dock: toolDocks.settings,
      outline_dock: toolDocks.outline,
      notes_anchor: toolAnchors.notes,
      new_note_anchor: toolAnchors['new-note'],
      settings_anchor: toolAnchors.settings,
      outline_anchor: toolAnchors.outline,
      notes_order: toolOrders.notes,
      new_note_order: toolOrders['new-note'],
      settings_order: toolOrders.settings,
      outline_order: toolOrders.outline,
      notes_hud_height: hudHeights.notes,
      outline_hud_height: hudHeights.outline
    });
    applyPersistedSettings(settings);
  }

  function persistUiSettingsInBackground() {
    void persistUiSettings().catch((error) => {
      status = error instanceof Error ? error.message : String(error);
    });
  }

  function syncLiveFieldsFromSource() {
    const split = splitMarkdownPlusSource(noteSource);
    liveBody = split.body;
    propertyRows = split.frontmatter
      .split(/\r?\n/)
      .map((line) => line.match(/^([A-Za-z0-9_-]+):\s*(.*)$/))
      .filter((match): match is RegExpMatchArray => Boolean(match))
      .map((match) => ({
        key: match[1],
        value: match[2] ?? ''
      }));
  }

  function updateSourceFromLiveFields(shouldAutosave = true) {
    const yaml = propertyRows
      .filter((property) => property.key.trim())
      .map((property) => `${property.key.trim()}: ${property.value}`)
      .join('\n');

    noteSource = `---\n${yaml}\n---\n${liveBody}`;

    if (shouldAutosave) {
      scheduleAutosave();
    }
  }

  function updateProperty(index: number, field: keyof PropertyRow, value: string) {
    propertyRows = propertyRows.map((property, propertyIndex) =>
      propertyIndex === index ? { ...property, [field]: value } : property
    );
    updateSourceFromLiveFields();
  }

  function setPropertyType(index: number, type: PropertyType) {
    const property = propertyRows[index];
    if (!property) return;

    propertyTypeOverrides = {
      ...propertyTypeOverrides,
      [property.key.trim()]: type
    };
    tokenPropertyDrafts = {
      ...tokenPropertyDrafts,
      [index]: ''
    };
    propertyTypeMenuIndex = null;
    propertyRows = propertyRows.map((row, propertyIndex) =>
      propertyIndex === index ? { ...row, value: valueForPropertyType(type, row.value) } : row
    );
    updateSourceFromLiveFields();
  }

  function updatePropertyValueFromLiveInput(index: number, value: string) {
    propertyRows = propertyRows.map((property, propertyIndex) =>
      propertyIndex === index ? { ...property, value: sourceValueFromLiveInput(property.key, property.value, value) } : property
    );
    updateSourceFromLiveFields();
  }

  function updateTokenPropertyDraft(index: number, value: string) {
    tokenPropertyDrafts = {
      ...tokenPropertyDrafts,
      [index]: value
    };
  }

  function addProperty() {
    propertyTypeMenuIndex = null;
    propertyRows = [...propertyRows, { key: 'property', value: 'value' }];
    updateSourceFromLiveFields();
  }

  function removeProperty(index: number) {
    propertyTypeMenuIndex = null;
    propertyRows = propertyRows.filter((_, propertyIndex) => propertyIndex !== index);
    updateSourceFromLiveFields();
  }

  function updateLiveBody(value: string) {
    liveBody = value;
    updateSourceFromLiveFields();
  }

  function isSystemProperty(key: string) {
    return Object.prototype.hasOwnProperty.call(systemPropertyLabels, key.trim());
  }

  function propertyLabel(key: string) {
    const normalized = key.trim();
    return systemPropertyLabels[normalized] ?? normalized.replace(/[_-]+/g, ' ');
  }

  function unquoteYamlScalar(value: string) {
    const trimmed = value.trim();
    if (
      (trimmed.startsWith('"') && trimmed.endsWith('"'))
      || (trimmed.startsWith("'") && trimmed.endsWith("'"))
    ) {
      return trimmed.slice(1, -1);
    }

    return trimmed;
  }

  function formatDateParts(year: string, month: string, day: string) {
    return `${year}/${month}/${day}`;
  }

  function formatTimeParts(hour: string, minute: string, year: string, month: string, day: string) {
    return `${hour}:${minute} ${formatDateParts(year, month, day)}`;
  }

  function quoteYamlString(value: string) {
    return JSON.stringify(value);
  }

  function isTokenProperty(property: PropertyRow) {
    const type = propertyTypeFor(property);
    return type === 'tags' || type === 'list';
  }

  function normalizeTokenValue(type: PropertyType, value: string) {
    const trimmed = value.trim();
    if (type === 'tags') {
      return trimmed.replace(/^#+/, '').trim();
    }

    return trimmed;
  }

  function propertyTypeFor(property: PropertyRow): PropertyType {
    const normalizedKey = property.key.trim();
    const override = propertyTypeOverrides[normalizedKey];
    if (override) return override;
    if (normalizedKey === 'tags') return 'tags';
    if (normalizedKey === 'aliases') return 'list';

    const value = unquoteYamlScalar(property.value.trim());
    if (/^(true|false)$/i.test(value)) return 'checkbox';
    if (/^\d{4}-\d{2}-\d{2}[T\s]\d{2}:\d{2}/.test(value)) return 'datetime';
    if (/^\d{4}-\d{2}-\d{2}$/.test(value)) return 'date';
    if (/^-?\d+(?:\.\d+)?$/.test(value)) return 'number';
    if (property.value.trim().startsWith('[') && property.value.trim().endsWith(']')) return 'list';
    return 'text';
  }

  function propertyTypeLabel(type: PropertyType) {
    return propertyTypeOptions.find((option) => option.id === type)?.label ?? 'Text';
  }

  function dateInputValue(value: string) {
    const unquoted = unquoteYamlScalar(value);
    return unquoted.match(/^(\d{4}-\d{2}-\d{2})/)?.[1] ?? '';
  }

  function datetimeInputValue(value: string) {
    const unquoted = unquoteYamlScalar(value);
    const match = unquoted.match(/^(\d{4}-\d{2}-\d{2})[T\s](\d{2}:\d{2})/);
    return match ? `${match[1]}T${match[2]}` : '';
  }

  function checkboxInputValue(value: string) {
    return /^true$/i.test(unquoteYamlScalar(value));
  }

  function todayDateValue() {
    return new Date().toISOString().slice(0, 10);
  }

  function valueForPropertyType(type: PropertyType, currentValue: string) {
    const text = unquoteYamlScalar(currentValue.trim());
    const tokens = parseYamlArrayValue(currentValue);

    if (type === 'checkbox') {
      return /^(true|yes|1|checked)$/i.test(text) ? 'true' : 'false';
    }

    if (type === 'date') {
      return dateInputValue(currentValue) || todayDateValue();
    }

    if (type === 'datetime') {
      const existing = datetimeInputValue(currentValue);
      return existing ? `${existing}:00Z` : new Date().toISOString().replace(/\.\d{3}Z$/, 'Z');
    }

    if (type === 'number') {
      return /^-?\d+(?:\.\d+)?$/.test(text) ? text : '0';
    }

    if (type === 'tags') {
      if (tokens.length) return sourceTagValueFromTokens(tokens.map((token) => normalizeTokenValue(type, token)));
      if (text && text !== '[]') return sourceTagValueFromTokens([normalizeTokenValue(type, text)]);
      return '[]';
    }

    if (type === 'list') {
      if (tokens.length) return sourceValueFromTokens(tokens.map((token) => normalizeTokenValue(type, token)));
      if (text && text !== '[]') return sourceValueFromTokens([normalizeTokenValue(type, text)]);
      return '[]';
    }

    if (tokens.length) return tokens.join(', ');
    if (/^(true|false)$/i.test(text)) return text.toLowerCase();
    return text;
  }

  function parseYamlArrayValue(value: string) {
    const raw = value.trim();
    if (!raw || raw === '[]') return [];

    if (raw.startsWith('[') && raw.endsWith(']')) {
      try {
        const parsed = JSON.parse(raw);
        if (Array.isArray(parsed)) {
          return parsed.map((item) => String(item)).filter(Boolean);
        }
      } catch {
        return raw
          .slice(1, -1)
          .split(',')
          .map((item) => unquoteYamlScalar(item.trim()))
          .filter(Boolean);
      }
    }

    return raw
      .split(',')
      .map((item) => unquoteYamlScalar(item.trim()))
      .filter(Boolean);
  }

  function sourceValueFromTokens(tokens: string[]) {
    return `[${tokens.map(quoteYamlString).join(', ')}]`;
  }

  function parseTagValue(value: string) {
    return parseYamlArrayValue(value)
      .flatMap((token) => token.split(/[\s,]+/))
      .map((token) => normalizeTokenValue('tags', token))
      .filter(Boolean);
  }

  function sourceTagValueFromTokens(tokens: string[]) {
    const deduped = Array.from(new Set(tokens.map((token) => normalizeTokenValue('tags', token)).filter(Boolean)));
    return deduped.length ? deduped.map((token) => `#${token}`).join(', ') : '[]';
  }

  function propertyTokens(property: PropertyRow) {
    return propertyTypeFor(property) === 'tags' ? parseTagValue(property.value) : parseYamlArrayValue(property.value);
  }

  function updateTokenProperty(index: number, tokens: string[]) {
    const property = propertyRows[index];
    const type = property ? propertyTypeFor(property) : 'list';
    const deduped = Array.from(new Set(tokens.map((token) => normalizeTokenValue(type, token)).filter(Boolean)));
    propertyRows = propertyRows.map((property, propertyIndex) =>
      propertyIndex === index
        ? { ...property, value: type === 'tags' ? sourceTagValueFromTokens(deduped) : sourceValueFromTokens(deduped) }
        : property
    );
    updateSourceFromLiveFields();
  }

  function commitTokenPropertyDraft(index: number, type: PropertyType) {
    const draft = tokenPropertyDrafts[index] ?? '';
    const tokensToAdd = draft
      .split(',')
      .map((token) => normalizeTokenValue(type, token))
      .filter(Boolean);

    if (!tokensToAdd.length) return;

    updateTokenProperty(index, [...propertyTokens(propertyRows[index]), ...tokensToAdd]);
    tokenPropertyDrafts = {
      ...tokenPropertyDrafts,
      [index]: ''
    };
  }

  function removeTokenPropertyItem(index: number, token: string) {
    updateTokenProperty(index, propertyTokens(propertyRows[index]).filter((item) => item !== token));
  }

  function handleTokenPropertyKeydown(event: KeyboardEvent, index: number, type: PropertyType) {
    if (event.key === 'Enter' || event.key === ',') {
      event.preventDefault();
      commitTokenPropertyDraft(index, type);
      return;
    }

    if (event.key === 'Backspace' && !(tokenPropertyDrafts[index] ?? '')) {
      const tokens = propertyTokens(propertyRows[index]);
      if (tokens.length) {
        event.preventDefault();
        updateTokenProperty(index, tokens.slice(0, -1));
      }
    }
  }

  function sourceValueFromLiveInput(key: string, currentSourceValue: string, inputValue: string) {
    const normalizedKey = key.trim();
    const trimmed = inputValue.trim();
    const current = currentSourceValue.trim();
    const wasQuoted = current.startsWith('"') && current.endsWith('"');

    if (normalizedKey === 'tags') {
      if (!trimmed || trimmed.toLowerCase() === 'none' || trimmed === '[]') {
        return '[]';
      }

      return sourceTagValueFromTokens(trimmed.split(/[\s,]+/));
    }

    if (normalizedKey === 'aliases') {
      if (!trimmed || trimmed.toLowerCase() === 'none' || trimmed === '[]') {
        return '[]';
      }

      if (trimmed.startsWith('[')) {
        return trimmed;
      }

      const values = trimmed
        .split(',')
        .map((item) => item.trim())
        .filter(Boolean);

      return `[${values.map(quoteYamlString).join(', ')}]`;
    }

    const friendlyTimestamp = trimmed.match(/^(\d{1,2}):(\d{2})\s+(\d{4})[/-](\d{2})[/-](\d{2})$/);
    if (friendlyTimestamp) {
      const [, hour, minute, year, month, day] = friendlyTimestamp;
      const source = `${year}-${month}-${day}T${hour.padStart(2, '0')}:${minute}:00Z`;
      return wasQuoted ? quoteYamlString(source) : source;
    }

    const friendlyDate = trimmed.match(/^(\d{4})[/-](\d{2})[/-](\d{2})$/);
    if (friendlyDate) {
      const [, year, month, day] = friendlyDate;
      const source = `${year}-${month}-${day}`;
      return wasQuoted ? quoteYamlString(source) : source;
    }

    return inputValue;
  }

  function propertyDisplayValue(key: string, value: string): PropertyDisplay {
    const raw = value.trim();
    const unquoted = unquoteYamlScalar(raw);
    const normalizedKey = key.trim();
    const timestamp = unquoted.match(/^(\d{4})-(\d{2})-(\d{2})[T\s](\d{2}):(\d{2})(?::\d{2}(?:\.\d+)?)?(?:Z|[+-]\d{2}:?\d{2})?$/);

    if (timestamp) {
      return {
        text: formatTimeParts(timestamp[4], timestamp[5], timestamp[1], timestamp[2], timestamp[3]),
        formatted: true
      };
    }

    const date = unquoted.match(/^(\d{4})-(\d{2})-(\d{2})$/);
    if (date) {
      return {
        text: formatDateParts(date[1], date[2], date[3]),
        formatted: true
      };
    }

    if (normalizedKey === 'tags') {
      const tags = parseTagValue(raw);
      return {
        text: tags.length ? tags.join(', ') : 'None',
        formatted: true
      };
    }

    if (normalizedKey === 'aliases') {
      if (raw === '[]') {
        return { text: 'None', formatted: true };
      }

      if (raw.startsWith('[') && raw.endsWith(']')) {
        try {
          const parsed = JSON.parse(raw);
          if (Array.isArray(parsed)) {
            return {
              text: parsed.length ? parsed.join(', ') : 'None',
              formatted: true
            };
          }
        } catch {
          return { text: raw.slice(1, -1), formatted: true };
        }
      }
    }

    return { text: unquoted, formatted: raw !== unquoted };
  }

  function splitMarkdownPlusSource(source: string): { frontmatter: string; body: string } {
    if (!source.startsWith('---')) {
      return { frontmatter: '', body: source };
    }

    const delimiter = source.indexOf('\n---', 3);
    if (delimiter === -1) {
      return { frontmatter: '', body: source };
    }

    const frontmatterStart = source.startsWith('---\r\n') ? 5 : 4;
    return {
      frontmatter: source.slice(frontmatterStart, delimiter),
      body: source.slice(delimiter + 4).replace(/^\r?\n+/, '')
    };
  }

  function handleEditorKeydown(event: KeyboardEvent, target: 'source' | 'body' = 'source') {
    if (handleTextareaPairing(event, target)) {
      return;
    }

    if (event.key !== 'Tab') {
      return;
    }

    event.preventDefault();
    const textarea = event.currentTarget as HTMLTextAreaElement;
    const selectionStart = textarea.selectionStart;
    const selectionEnd = textarea.selectionEnd;

    if (event.shiftKey) {
      outdentSelection(textarea, selectionStart, selectionEnd, target);
      return;
    }

    indentSelection(textarea, selectionStart, selectionEnd, target);
  }

  function handleSourceInput(value: string) {
    noteSource = value;
    syncLiveFieldsFromSource();
    scheduleAutosave();
  }

  function setEditorText(target: 'source' | 'body', value: string) {
    if (target === 'body') {
      updateLiveBody(value);
      return;
    }

    handleSourceInput(value);
  }

  function indentSelection(
    textarea: HTMLTextAreaElement,
    selectionStart: number,
    selectionEnd: number,
    target: 'source' | 'body'
  ) {
    const indent = '  ';
    const text = target === 'body' ? liveBody : noteSource;

    if (selectionStart === selectionEnd) {
      setEditorText(target, text.slice(0, selectionStart) + indent + text.slice(selectionEnd));
      queueSelection(textarea, selectionStart + indent.length, selectionStart + indent.length);
      return;
    }

    const lineStart = text.lastIndexOf('\n', selectionStart - 1) + 1;
    const selected = text.slice(lineStart, selectionEnd);
    const indented = selected.replace(/^/gm, indent);
    setEditorText(target, text.slice(0, lineStart) + indented + text.slice(selectionEnd));
    queueSelection(textarea, selectionStart + indent.length, selectionEnd + indented.length - selected.length);
  }

  function outdentSelection(
    textarea: HTMLTextAreaElement,
    selectionStart: number,
    selectionEnd: number,
    target: 'source' | 'body'
  ) {
    const text = target === 'body' ? liveBody : noteSource;
    const lineStart = text.lastIndexOf('\n', selectionStart - 1) + 1;
    const selected = text.slice(lineStart, selectionEnd);
    const outdented = selected.replace(/^( {1,2}|\t)/gm, '');
    const removedBeforeSelection = selected
      .slice(0, selectionStart - lineStart)
      .match(/^( {1,2}|\t)/gm)
      ?.join('').length ?? 0;

    setEditorText(target, text.slice(0, lineStart) + outdented + text.slice(selectionEnd));

    const removedTotal = selected.length - outdented.length;
    queueSelection(
      textarea,
      Math.max(lineStart, selectionStart - removedBeforeSelection),
      Math.max(lineStart, selectionEnd - removedTotal)
    );
  }

  function queueSelection(textarea: HTMLTextAreaElement, start: number, end: number) {
    requestAnimationFrame(() => {
      textarea.selectionStart = start;
      textarea.selectionEnd = end;
    });
  }

  function handleTextareaPairing(event: KeyboardEvent, target: 'source' | 'body') {
    if (event.ctrlKey || event.altKey || event.metaKey || event.isComposing) {
      return false;
    }

    const open = event.key;
    const close = enclosingPairs[open];
    if (close) {
      event.preventDefault();
      const textarea = event.currentTarget as HTMLTextAreaElement;
      const text = target === 'body' ? liveBody : noteSource;
      const selectionStart = textarea.selectionStart;
      const selectionEnd = textarea.selectionEnd;
      const selected = text.slice(selectionStart, selectionEnd);

      if (!selected && open === close && text.slice(selectionStart, selectionStart + close.length) === close) {
        queueSelection(textarea, selectionStart + close.length, selectionStart + close.length);
        return true;
      }

      setEditorText(target, text.slice(0, selectionStart) + open + selected + close + text.slice(selectionEnd));
      const cursor = selected ? selectionEnd + open.length + close.length : selectionStart + open.length;
      queueSelection(textarea, cursor, cursor);
      return true;
    }

    if (closingPairs.has(event.key)) {
      const textarea = event.currentTarget as HTMLTextAreaElement;
      const text = target === 'body' ? liveBody : noteSource;
      const selectionStart = textarea.selectionStart;
      const selectionEnd = textarea.selectionEnd;
      if (selectionStart === selectionEnd && text.slice(selectionStart, selectionStart + event.key.length) === event.key) {
        event.preventDefault();
        queueSelection(textarea, selectionStart + event.key.length, selectionStart + event.key.length);
        return true;
      }
    }

    return false;
  }

  async function handlePreviewClick(event: MouseEvent) {
    const target = event.target instanceof Element
      ? event.target.closest<HTMLAnchorElement>('a[data-mdp-internal-link]')
      : null;
    if (!target) return;

    event.preventDefault();
    await openInternalLinkTarget(target.dataset.mdpInternalLink ?? '');
  }

  async function openInternalLinkTarget(target: string) {
    const noteId = resolveInternalLink(target);
    if (!noteId) {
      status = `No note found for ${target}.`;
      return;
    }

    editorMode = 'live';
    await selectNote(noteId);
  }

  function openExternalLinkTarget(target: string) {
    const trimmed = target.trim();
    if (!trimmed) return;

    window.open(trimmed, '_blank', 'noopener,noreferrer');
  }

  function resolveInternalLink(target: string) {
    const normalizedTarget = normalizeInternalLinkTarget(target);
    return notes.find((note) =>
      normalizeInternalLinkTarget(note.title) === normalizedTarget
      || normalizeInternalLinkTarget(note.id) === normalizedTarget
      || normalizeInternalLinkTarget(note.path.replace(/\\/g, '/').split('/').pop()?.replace(/\.mdp$/i, '') ?? '') === normalizedTarget
    )?.id ?? null;
  }

  function normalizeInternalLinkTarget(target: string) {
    return target
      .trim()
      .replace(/\\/g, '/')
      .split('#')[0]
      .split('/')
      .pop()
      ?.replace(/\.(md|mdp)$/i, '')
      .toLowerCase() ?? '';
  }
</script>

<svelte:head>
  <title>MarkdownPlus</title>
</svelte:head>

<main
  class="app-shell"
  style={`--left-panel-width: ${leftPanelColumnWidth}px; --right-panel-width: ${rightPanelColumnWidth}px;`}
>
  <div
    class="window-drag-badge"
    aria-label="Move MarkdownPlus window"
    title="Move window"
    role="button"
    tabindex="0"
    on:pointerdown={handleWindowBadgePointerDown}
  >
    <img src={appIconUrl} alt="" aria-hidden="true" />
  </div>

  <aside
    class:ribbon-panel={leftPanelMode === 'ribbon'}
    class="sidebar"
    data-tool-drop-side="left"
  >
    {#if leftPanelMode === 'ribbon'}
      <nav
        class="panel-ribbon"
        aria-label="Left ribbon"
        data-tool-drop-side="left"
      >
        {#each leftToolZones as zone}
          <div
            class:center-zone={zone.anchor === 'center'}
            class:bottom-zone={zone.anchor === 'bottom'}
            class:drop-ready={Boolean(draggingTool)}
            class="tool-zone"
            role="group"
            aria-label={`Left ${zone.anchor} tools`}
            data-tool-drop-side="left"
            data-tool-drop-anchor={zone.anchor}
          >
            {#each zone.tools as tool}
              <button
                class:active={tool.id === 'settings' && settingsOpen}
                class:drag-enabled={true}
                class="ribbon-button"
                aria-label={tool.label}
                aria-disabled={tool.id === 'new-note' && !workspace}
                title={`${tool.label} - drag to move`}
                data-tool-drop-side="left"
                data-tool-drop-anchor={zone.anchor}
                data-tool-drop-target={tool.id}
                on:click={() => handleToolClick(tool.id)}
                on:dblclick={() => moveToolToOppositeDock(tool.id)}
                on:pointerdown={(event) => startToolPointerDrag(event, tool.id)}
              >
                {#if tool.id === 'notes'}
                  <NotebookText size={17} />
                {:else if tool.id === 'new-note'}
                  <FilePlus size={17} />
                {:else if tool.id === 'settings'}
                  <Cog size={17} />
                {:else if tool.id === 'outline'}
                  <ListTree size={17} />
                {/if}
              </button>
            {/each}
          </div>
        {/each}
      </nav>
      <button
        class:active={settingsOpen}
        class="fixed-settings-button ribbon-button"
        aria-label="Settings"
        title="Settings"
        on:click={openSettings}
      >
        <Cog size={17} />
      </button>
    {:else}
      <div class="brand">
        <div data-tauri-drag-region>
          <h1>MarkdownPlus</h1>
          <p>Local .mdp workspace</p>
        </div>
        <button
          class:active={settingsOpen}
          class="fixed-settings-button icon-button"
          aria-label="Settings"
          title="Settings"
          on:click={openSettings}
        >
          <Cog size={15} />
        </button>
      </div>

      <div class="panel-tool-stack" aria-label="Left panel tools">
        {#each leftToolZones as zone}
          <div
            class:center-zone={zone.anchor === 'center'}
            class:bottom-zone={zone.anchor === 'bottom'}
            class:drop-ready={Boolean(draggingTool)}
            class="tool-zone"
            role="group"
            aria-label={`Left ${zone.anchor} tools`}
            data-tool-drop-side="left"
            data-tool-drop-anchor={zone.anchor}
          >
            {#each zone.tools as tool}
              <section
                class="panel-tool-group"
                role="group"
                aria-label={tool.label}
                data-tool-drop-side="left"
                data-tool-drop-anchor={zone.anchor}
                data-tool-drop-target={tool.id}
              >
            <div
              class:has-compact-action={tool.id === 'notes'}
              class="panel-tool-row"
              role="group"
              aria-label={`${tool.label} tool handle`}
              on:pointerdown={(event) => startToolPointerDrag(event, tool.id)}
            >
              <button
                class:active={tool.id === 'settings' && settingsOpen}
                class="panel-tool-button"
                aria-label={tool.label}
                aria-disabled={tool.id === 'new-note' && !workspace}
                title={`${tool.label} - drag to move`}
                on:click={() => handleToolClick(tool.id)}
                on:dblclick={() => moveToolToOppositeDock(tool.id)}
              >
                {#if tool.id === 'notes'}
                  <NotebookText size={15} />
                {:else if tool.id === 'new-note'}
                  <FilePlus size={15} />
                {:else if tool.id === 'settings'}
                  <Cog size={15} />
                {:else if tool.id === 'outline'}
                  <ListTree size={15} />
                {/if}
                <span>{tool.label}</span>
              </button>
              {#if tool.id === 'notes'}
                <div class="compact-actions">
                  <button class="compact-action" disabled={!workspace} on:click={createNewNote}>New</button>
                  <button class="compact-action" disabled={!workspace} on:click={createNewBase}>Base</button>
                </div>
              {/if}
            </div>

            {#if tool.id === 'notes'}
              <div class="tool-hud notes-hud" style={`--hud-height: ${hudHeights.notes}px;`}>
                {#if workspace}
                  <nav class="notes-list" aria-label="Notes">
                    {#each notes as note}
                      <button
                        class:active={note.id === selectedId}
                        class="note-row"
                        on:click={() => {
                          settingsOpen = false;
                          editorMode = 'live';
                          selectNote(note.id);
                        }}
                        on:contextmenu={(event) => openNoteContextMenu(event, note)}
                      >
                        {#if note.note_type === 'base'}
                          <Table size={13} />
                        {:else}
                          <NotebookText size={13} />
                        {/if}
                        <span class="note-title">{note.title}</span>
                      </button>
                    {/each}
                  </nav>
                  <div class="notes-count">{notes.length} documents</div>
                {:else}
                  <div class="sidebar-empty">Open Settings to choose a workspace.</div>
                {/if}
              </div>
              <button
                class="hud-resize-handle"
                type="button"
                aria-label="Resize Notes HUD"
                on:pointerdown={(event) => startHudResize(event, 'notes')}
                on:keydown={(event) => handleHudResizeKeydown(event, 'notes')}
              ></button>
            {:else if tool.id === 'outline'}
              <div class="tool-hud outline-hud" style={`--hud-height: ${hudHeights.outline}px;`}>
                {#if selectedNoteSource && outlineItems.length}
                  <nav class="outline-list" aria-label="Note outline">
                    {#each outlineItems as item}
                      <div class:level-two={item.level === 2} class:level-three={item.level === 3} class="outline-row">
                        {item.text}
                      </div>
                    {/each}
                  </nav>
                {:else}
                  <div class="right-panel-empty">
                    No headings found.
                  </div>
                {/if}
              </div>
              <button
                class="hud-resize-handle"
                type="button"
                aria-label="Resize Outline HUD"
                on:pointerdown={(event) => startHudResize(event, 'outline')}
                on:keydown={(event) => handleHudResizeKeydown(event, 'outline')}
              ></button>
            {/if}
              </section>
            {/each}
          </div>
        {/each}

        {#if !leftHasHudTool}
          <div class="sidebar-empty">
            {workspace ? 'Drop tools here.' : 'Open Settings to choose a workspace.'}
          </div>
        {/if}
      </div>

    {/if}
  </aside>

  <button
    class="panel-resize-handle"
    type="button"
    aria-label="Resize left panel"
    disabled={leftPanelMode === 'ribbon'}
    on:pointerdown={startLeftPanelResize}
    on:keydown={handleLeftPanelResizeKeydown}
  ></button>

  <section class="editor">
    {#if settingsOpen}
      <div class="settings-view">
        <header class="settings-view-header" data-tauri-drag-region>
          <div>
            <h2>Settings</h2>
            <p>MarkdownPlusData</p>
          </div>
          <button class="icon-button" aria-label="Close settings" on:click={() => (settingsOpen = false)}>
            <X size={15} />
          </button>
        </header>

        <div class="settings-view-content">
          <section class="settings-section">
            <div class="settings-section-heading">
              <FolderOpen size={16} />
              <h3>Workspace</h3>
            </div>

            <label for="workspace-path">Workspace folder</label>
            <div class="workspace-row">
              <input
                id="workspace-path"
                bind:value={workspacePath}
                placeholder="/home/lua/MarkdownPlus Workspace"
              />
              <button on:click={browseWorkspace} disabled={browsing}>
                {browsing ? '...' : 'Browse'}
              </button>
              <button class="primary" on:click={openCurrentWorkspace}>Open</button>
            </div>
            {#if portableRoot}
              <p class="settings-note">Portable app data: {portableRoot}</p>
            {/if}
          </section>

          <section class="settings-section">
            <div class="settings-section-heading">
              <SlidersHorizontal size={16} />
              <h3>Panels</h3>
            </div>

            <div class="setting-row">
              <div>
                <span>Left panel</span>
                <p>{leftPanelMode === 'ribbon' ? 'Ribbon' : `${leftPanelWidth}px`}</p>
              </div>
              <div class="segmented-control" aria-label="Left panel mode">
                <button class:active={leftPanelMode === 'view'} on:click={() => setPanelMode('left', 'view')}>View</button>
                <button class:active={leftPanelMode === 'ribbon'} on:click={() => setPanelMode('left', 'ribbon')}>Ribbon</button>
              </div>
            </div>

            <div class="setting-row">
              <div>
                <span>Right panel</span>
                <p>{rightPanelMode === 'ribbon' ? 'Ribbon' : `${rightPanelWidth}px`}</p>
              </div>
              <div class="segmented-control" aria-label="Right panel mode">
                <button class:active={rightPanelMode === 'view'} on:click={() => setPanelMode('right', 'view')}>View</button>
                <button class:active={rightPanelMode === 'ribbon'} on:click={() => setPanelMode('right', 'ribbon')}>Ribbon</button>
              </div>
            </div>

            <div class="settings-subsection">
              <h4>Panel tools</h4>
              {#each ribbonTools as tool}
                <div class="setting-row">
                  <div>
                    <span>{tool.label}</span>
                    <p>{tool.dock === 'left' ? 'Left panel' : 'Right panel'} · {tool.anchor}</p>
                  </div>
                  <div class="setting-actions">
                    <div class="segmented-control" aria-label={`${tool.label} dock`}>
                      <button class:active={tool.dock === 'left'} on:click={() => setToolDock(tool.id, 'left')}>Left</button>
                      <button class:active={tool.dock === 'right'} on:click={() => setToolDock(tool.id, 'right')}>Right</button>
                    </div>
                    <div class="segmented-control three-options" aria-label={`${tool.label} anchor`}>
                      <button class:active={tool.anchor === 'top'} on:click={() => setToolAnchor(tool.id, 'top')}>Top</button>
                      <button class:active={tool.anchor === 'center'} on:click={() => setToolAnchor(tool.id, 'center')}>Center</button>
                      <button class:active={tool.anchor === 'bottom'} on:click={() => setToolAnchor(tool.id, 'bottom')}>Bottom</button>
                    </div>
                  </div>
                </div>
              {/each}
            </div>
          </section>
        </div>
      </div>
    {:else if selectedNoteSource && selectedIsBase}
      <BasesView
        {notes}
        baseTitle={selectedTitle}
        baseId={selectedId}
        selectedId={selectedId}
        onOpenNote={(id) => {
          editorMode = 'live';
          void selectNote(id);
        }}
        onNotesChanged={(updatedNotes) => {
          notes = updatedNotes;
        }}
        setStatus={(message) => {
          status = message;
        }}
      />
    {:else if selectedNoteSource}
      <div class="note-page">
        <header class="editor-header">
          <h2>{selectedTitle}</h2>
        </header>

        {#if editorMode === 'live'}
          <div class="live-editor">
            <section class="live-properties" aria-label="Note properties">
              <div class="property-list">
                {#each propertyRows as property, index}
                  {@const propertyType = propertyTypeFor(property)}
                  {@const displayValue = propertyDisplayValue(property.key, property.value)}
                  <div class="property-row">
                    <div class="property-type-cell">
                      <button
                        class:active={propertyTypeMenuIndex === index}
                        class="property-type-button"
                        type="button"
                        aria-label={`Change ${propertyLabel(property.key)} property type`}
                        title={propertyTypeLabel(propertyType)}
                        on:click={() => (propertyTypeMenuIndex = propertyTypeMenuIndex === index ? null : index)}
                      >
                        {#if propertyType === 'checkbox'}
                          <SquareCheck size={14} />
                        {:else if propertyType === 'date'}
                          <CalendarDays size={14} />
                        {:else if propertyType === 'datetime'}
                          <CalendarClock size={14} />
                        {:else if propertyType === 'list'}
                          <List size={14} />
                        {:else if propertyType === 'number'}
                          <Hash size={14} />
                        {:else if propertyType === 'tags'}
                          <Tags size={14} />
                        {:else}
                          <Type size={14} />
                        {/if}
                      </button>
                      {#if propertyTypeMenuIndex === index}
                        <div class="property-type-menu" role="menu" aria-label="Property type">
                          {#each propertyTypeOptions as option}
                            <button
                              class:active={propertyType === option.id}
                              type="button"
                              role="menuitemradio"
                              aria-checked={propertyType === option.id}
                              on:click={() => setPropertyType(index, option.id)}
                            >
                              {#if option.id === 'checkbox'}
                                <SquareCheck size={14} />
                              {:else if option.id === 'date'}
                                <CalendarDays size={14} />
                              {:else if option.id === 'datetime'}
                                <CalendarClock size={14} />
                              {:else if option.id === 'list'}
                                <List size={14} />
                              {:else if option.id === 'number'}
                                <Hash size={14} />
                              {:else if option.id === 'tags'}
                                <Tags size={14} />
                              {:else}
                                <Type size={14} />
                              {/if}
                              <span>{option.label}</span>
                              {#if propertyType === option.id}
                                <SquareCheck size={13} />
                              {/if}
                            </button>
                          {/each}
                        </div>
                      {/if}
                    </div>
                    {#if isSystemProperty(property.key)}
                      <span class="property-label" title={property.key}>{propertyLabel(property.key)}</span>
                    {:else}
                      <input
                        class="property-name-input"
                        aria-label="Property name"
                        value={property.key}
                        on:input={(event) => updateProperty(index, 'key', event.currentTarget.value)}
                      />
                    {/if}
                    {#if isTokenProperty(property)}
                      {@const tokens = propertyTokens(property)}
                      <div class="token-property-field" title={property.value}>
                        {#each tokens as token}
                          <span class="property-token">
                            {token}
                            <button
                              type="button"
                              aria-label={`Remove ${token}`}
                              on:click={() => removeTokenPropertyItem(index, token)}
                            >
                              <X size={11} />
                            </button>
                          </span>
                        {/each}
                        <input
                          aria-label={`Add ${propertyLabel(property.key)}`}
                          class="token-property-input"
                          placeholder={tokens.length ? '' : `Add ${propertyLabel(property.key).toLowerCase()}`}
                          value={tokenPropertyDrafts[index] ?? ''}
                          on:input={(event) => updateTokenPropertyDraft(index, event.currentTarget.value)}
                          on:keydown={(event) => handleTokenPropertyKeydown(event, index, propertyType)}
                          on:blur={() => commitTokenPropertyDraft(index, propertyType)}
                        />
                      </div>
                    {:else if propertyType === 'checkbox'}
                      <label class="checkbox-property-field">
                        <input
                          type="checkbox"
                          checked={checkboxInputValue(property.value)}
                          on:change={(event) => updateProperty(index, 'value', event.currentTarget.checked ? 'true' : 'false')}
                        />
                        <span>{checkboxInputValue(property.value) ? 'Checked' : 'Unchecked'}</span>
                      </label>
                    {:else if propertyType === 'date'}
                      <input
                        aria-label="Property date"
                        class="formatted-property-input"
                        type="date"
                        value={dateInputValue(property.value)}
                        on:input={(event) => updateProperty(index, 'value', event.currentTarget.value)}
                      />
                    {:else if propertyType === 'datetime'}
                      <input
                        aria-label="Property date and time"
                        class="formatted-property-input"
                        type="datetime-local"
                        value={datetimeInputValue(property.value)}
                        on:input={(event) => updateProperty(index, 'value', event.currentTarget.value ? `${event.currentTarget.value}:00Z` : '')}
                      />
                    {:else if propertyType === 'number'}
                      <input
                        aria-label="Property number"
                        type="number"
                        value={unquoteYamlScalar(property.value)}
                        on:input={(event) => updateProperty(index, 'value', event.currentTarget.value)}
                      />
                    {:else}
                      <input
                        aria-label="Property value"
                        class:formatted-property-input={displayValue.formatted}
                        title={displayValue.formatted ? property.value : undefined}
                        value={displayValue.text}
                        on:input={(event) => updatePropertyValueFromLiveInput(index, event.currentTarget.value)}
                      />
                    {/if}
                    <button class="property-remove-button" aria-label="Remove property" on:click={() => removeProperty(index)}>
                      <X size={13} />
                    </button>
                  </div>
                {/each}
              </div>

              <button class="add-property-button" on:click={addProperty}>Add property</button>
            </section>

            <div class="live-body-editor">
              <MarkdownPlusEditor
                value={liveBody}
                ariaLabel="MarkdownPlus body"
                onChange={updateLiveBody}
                onInternalLink={(target) => void openInternalLinkTarget(target)}
                onExternalLink={openExternalLinkTarget}
              />
            </div>
          </div>
        {:else}
          <div
            class:preview-only={editorMode === 'preview'}
            class:source-only={editorMode === 'source'}
            class="body-shell"
          >
            {#if editorMode !== 'preview'}
            <textarea
              class="body-editor"
              value={noteSource}
              aria-label="MarkdownPlus source"
              on:input={(event) => handleSourceInput(event.currentTarget.value)}
              on:keydown={handleEditorKeydown}
            ></textarea>
            {/if}

            {#if editorMode !== 'source'}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
            <article class="markdown-preview" on:click={handlePreviewClick}>
              {@html markdownHtml}
            </article>
            {/if}
          </div>
        {/if}
      </div>
    {:else}
      <div class="empty-state" data-tauri-drag-region>
        <h2>Open a workspace</h2>
        <p>Use Settings in the sidebar to select a local workspace folder.</p>
      </div>
    {/if}

    <footer>
      <span>{status}</span>
      {#if selectedNoteSource && !selectedIsBase && !settingsOpen}
        <div class="mode-group" aria-label="Note view">
          <button
            class:active={editorMode === 'live'}
            class="mode-button"
            on:click={() => (editorMode = 'live')}
          >
            Live
          </button>
          <button
            class:active={editorMode === 'source'}
            class="mode-button"
            on:click={() => (editorMode = 'source')}
          >
            Source
          </button>
          <button
            class:active={editorMode === 'split'}
            class="mode-button"
            on:click={() => (editorMode = 'split')}
          >
            Split
          </button>
          <button
            class:active={editorMode === 'preview'}
            class="mode-button"
            on:click={() => (editorMode = 'preview')}
          >
            Preview
          </button>
        </div>
      {/if}
    </footer>
  </section>

  {#if noteContextMenu}
    <div
      bind:this={noteContextMenuElement}
      class="note-context-menu"
      role="menu"
      aria-label={`${noteContextMenu.note.title} actions`}
      style={`left: ${noteContextMenu.x}px; top: ${noteContextMenu.y}px;`}
    >
      <button type="button" role="menuitem" on:click={openContextMenuNote}>Open</button>
      <button type="button" role="menuitem" on:click={createNewNote}>New note</button>
      <button type="button" role="menuitem" on:click={createNewBase}>New base</button>
      <div class="context-menu-separator"></div>
      <button type="button" role="menuitem" on:click={copyContextMenuPath}>Copy path</button>
      <button type="button" role="menuitem" on:click={copyContextMenuId}>Copy ID</button>
      <div class="context-menu-separator"></div>
      <button class="danger" type="button" role="menuitem" on:click={deleteContextMenuNote}>
        Delete {noteContextMenu.note.note_type === 'base' ? 'base' : 'note'}
      </button>
    </div>
  {/if}

  <button
    class="panel-resize-handle right-resize-handle"
    type="button"
    aria-label="Resize right panel"
    disabled={rightPanelMode === 'ribbon'}
    on:pointerdown={startRightPanelResize}
    on:keydown={handleRightPanelResizeKeydown}
  ></button>

  <aside
    class:ribbon-panel={rightPanelMode === 'ribbon'}
    class="right-panel"
    data-tool-drop-side="right"
  >
    {#if rightPanelMode === 'ribbon'}
      <nav
        class="panel-ribbon"
        aria-label="Right ribbon"
        data-tool-drop-side="right"
      >
        {#each rightToolZones as zone}
          <div
            class:center-zone={zone.anchor === 'center'}
            class:bottom-zone={zone.anchor === 'bottom'}
            class:drop-ready={Boolean(draggingTool)}
            class="tool-zone"
            role="group"
            aria-label={`Right ${zone.anchor} tools`}
            data-tool-drop-side="right"
            data-tool-drop-anchor={zone.anchor}
          >
            {#each zone.tools as tool}
              <button
                class:active={tool.id === 'settings' && settingsOpen}
                class:drag-enabled={true}
                class="ribbon-button"
                aria-label={tool.label}
                aria-disabled={tool.id === 'new-note' && !workspace}
                title={`${tool.label} - drag to move`}
                data-tool-drop-side="right"
                data-tool-drop-anchor={zone.anchor}
                data-tool-drop-target={tool.id}
                on:click={() => handleToolClick(tool.id)}
                on:dblclick={() => moveToolToOppositeDock(tool.id)}
                on:pointerdown={(event) => startToolPointerDrag(event, tool.id)}
              >
                {#if tool.id === 'notes'}
                  <NotebookText size={17} />
                {:else if tool.id === 'new-note'}
                  <FilePlus size={17} />
                {:else if tool.id === 'settings'}
                  <Cog size={17} />
                {:else if tool.id === 'outline'}
                  <ListTree size={17} />
                {/if}
              </button>
            {/each}
          </div>
        {/each}
      </nav>
    {:else}
      <div class="panel-tool-stack" aria-label="Right panel tools">
        {#each rightToolZones as zone}
          <div
            class:center-zone={zone.anchor === 'center'}
            class:bottom-zone={zone.anchor === 'bottom'}
            class:drop-ready={Boolean(draggingTool)}
            class="tool-zone"
            role="group"
            aria-label={`Right ${zone.anchor} tools`}
            data-tool-drop-side="right"
            data-tool-drop-anchor={zone.anchor}
          >
            {#each zone.tools as tool}
              <section
                class="panel-tool-group"
                role="group"
                aria-label={tool.label}
                data-tool-drop-side="right"
                data-tool-drop-anchor={zone.anchor}
                data-tool-drop-target={tool.id}
              >
            <div
              class:has-compact-action={tool.id === 'notes'}
              class="panel-tool-row"
              role="group"
              aria-label={`${tool.label} tool handle`}
              on:pointerdown={(event) => startToolPointerDrag(event, tool.id)}
            >
              <button
                class:active={tool.id === 'settings' && settingsOpen}
                class="panel-tool-button"
                aria-label={tool.label}
                aria-disabled={tool.id === 'new-note' && !workspace}
                title={`${tool.label} - drag to move`}
                on:click={() => handleToolClick(tool.id)}
                on:dblclick={() => moveToolToOppositeDock(tool.id)}
              >
                {#if tool.id === 'notes'}
                  <NotebookText size={15} />
                {:else if tool.id === 'new-note'}
                  <FilePlus size={15} />
                {:else if tool.id === 'settings'}
                  <Cog size={15} />
                {:else if tool.id === 'outline'}
                  <ListTree size={15} />
                {/if}
                <span>{tool.label}</span>
              </button>
              {#if tool.id === 'notes'}
                <div class="compact-actions">
                  <button class="compact-action" disabled={!workspace} on:click={createNewNote}>New</button>
                  <button class="compact-action" disabled={!workspace} on:click={createNewBase}>Base</button>
                </div>
              {/if}
            </div>

            {#if tool.id === 'notes'}
              <div class="tool-hud notes-hud" style={`--hud-height: ${hudHeights.notes}px;`}>
                {#if workspace}
                  <nav class="notes-list" aria-label="Notes">
                    {#each notes as note}
                      <button
                        class:active={note.id === selectedId}
                        class="note-row"
                        on:click={() => {
                          settingsOpen = false;
                          editorMode = 'live';
                          selectNote(note.id);
                        }}
                        on:contextmenu={(event) => openNoteContextMenu(event, note)}
                      >
                        {#if note.note_type === 'base'}
                          <Table size={13} />
                        {:else}
                          <NotebookText size={13} />
                        {/if}
                        <span class="note-title">{note.title}</span>
                      </button>
                    {/each}
                  </nav>
                  <div class="notes-count">{notes.length} documents</div>
                {:else}
                  <div class="sidebar-empty">Open Settings to choose a workspace.</div>
                {/if}
              </div>
              <button
                class="hud-resize-handle"
                type="button"
                aria-label="Resize Notes HUD"
                on:pointerdown={(event) => startHudResize(event, 'notes')}
                on:keydown={(event) => handleHudResizeKeydown(event, 'notes')}
              ></button>
            {:else if tool.id === 'outline'}
              <div class="tool-hud outline-hud" style={`--hud-height: ${hudHeights.outline}px;`}>
                {#if selectedNoteSource && outlineItems.length}
                  <nav class="outline-list" aria-label="Note outline">
                    {#each outlineItems as item}
                      <div class:level-two={item.level === 2} class:level-three={item.level === 3} class="outline-row">
                        {item.text}
                      </div>
                    {/each}
                  </nav>
                {:else}
                  <div class="right-panel-empty">
                    No headings found.
                  </div>
                {/if}
              </div>
              <button
                class="hud-resize-handle"
                type="button"
                aria-label="Resize Outline HUD"
                on:pointerdown={(event) => startHudResize(event, 'outline')}
                on:keydown={(event) => handleHudResizeKeydown(event, 'outline')}
              ></button>
            {/if}
              </section>
            {/each}
          </div>
        {/each}

        {#if !rightHasHudTool}
          <div class="right-panel-empty">
            Drop tools here.
          </div>
        {/if}
      </div>
    {/if}
  </aside>
</main>

<style>
  .app-shell {
    display: grid;
    grid-template-columns: var(--left-panel-width) 4px minmax(0, 1fr) 4px var(--right-panel-width);
    position: relative;
    min-height: 100vh;
    max-height: 100vh;
    background: #0d1117;
  }

  .window-drag-badge {
    display: grid;
    place-items: center;
    position: absolute;
    top: 0.25rem;
    left: 0.25rem;
    z-index: 20;
    width: 3rem;
    height: 3rem;
    border: 1px solid #2a3b45;
    border-radius: 6px;
    background: #10211e;
    padding: 0.34rem;
    color: #9fdcc9;
    line-height: 1;
    cursor: move;
    user-select: none;
  }

  .window-drag-badge img {
    display: block;
    width: 100%;
    height: 100%;
    border-radius: 3px;
    object-fit: contain;
    pointer-events: none;
  }

  .window-drag-badge:hover,
  .window-drag-badge:focus {
    border-color: #2ea987;
    background: #10211e;
    color: #baf0de;
  }

  .sidebar {
    display: flex;
    grid-column: 1;
    grid-row: 1;
    flex-direction: column;
    gap: 0.72rem;
    background: #0b0f14;
    padding: 3.65rem 0.72rem 0.72rem;
    min-height: 0;
  }

  .sidebar.ribbon-panel,
  .right-panel.ribbon-panel {
    align-items: center;
    gap: 0;
    padding: 0.55rem 0.2rem;
  }

  .sidebar.ribbon-panel {
    padding-top: 3.65rem;
  }

  .panel-ribbon {
    display: grid;
    grid-template-rows: minmax(0, 1fr) 0 minmax(0, 1fr);
    flex: 1 1 auto;
    gap: 0.36rem;
    width: 100%;
    min-height: 0;
    overflow: hidden;
  }

  .tool-zone {
    display: grid;
    align-content: start;
    gap: 0.32rem;
    min-height: 0;
    min-width: 0;
    width: 100%;
  }

  .tool-zone.drop-ready {
    min-height: 1.65rem;
    outline: 1px dashed transparent;
    outline-offset: -2px;
  }

  .tool-zone.drop-ready:empty {
    border-radius: 4px;
    outline-color: #23303d;
  }

  .tool-zone.drop-ready:empty:hover {
    outline-color: #2ea987;
  }

  .panel-ribbon .tool-zone {
    justify-items: center;
  }

  .tool-zone.center-zone {
    align-content: center;
  }

  .tool-zone.bottom-zone {
    align-content: end;
  }

  .tool-zone.drop-ready:empty {
    align-self: start;
  }

  .tool-zone.center-zone.drop-ready:empty {
    align-self: center;
  }

  .tool-zone.bottom-zone.drop-ready:empty {
    align-self: end;
  }

  .ribbon-button {
    display: grid;
    place-items: center;
    width: 1.85rem;
    height: 1.85rem;
    border-color: transparent;
    background: transparent;
    padding: 0;
    color: #8d98a6;
  }

  .fixed-settings-button {
    flex: 0 0 auto;
  }

  .sidebar.ribbon-panel .fixed-settings-button {
    margin-top: 0.55rem;
  }

  .ribbon-button:hover,
  .ribbon-button:focus,
  .ribbon-button.active {
    border-color: #2ea987;
    background: #10211e;
    color: #e6edf3;
  }

  .ribbon-button.drag-enabled {
    cursor: grab;
  }

  .ribbon-button.drag-enabled:active {
    cursor: grabbing;
  }

  .ribbon-button[aria-disabled='true'],
  .panel-tool-button[aria-disabled='true'] {
    opacity: 0.42;
  }

  .panel-tool-stack {
    display: flex;
    flex-direction: column;
    flex: 1 1 auto;
    min-height: 0;
    gap: 0.38rem;
    overflow: auto;
  }

  .panel-tool-stack .tool-zone {
    align-content: start;
    align-self: stretch;
    flex: 0 0 auto;
  }

  .panel-tool-stack .tool-zone.center-zone,
  .panel-tool-stack .tool-zone.bottom-zone {
    align-content: start;
    align-self: stretch;
  }

  .panel-tool-group {
    display: grid;
    gap: 0.16rem;
    min-height: 0;
  }

  .panel-tool-row {
    cursor: grab;
  }

  .panel-tool-row:active {
    cursor: grabbing;
  }

  .panel-tool-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: 0.3rem;
  }

  .panel-tool-row.has-compact-action {
    grid-template-columns: minmax(0, 1fr) auto;
  }

  .panel-tool-button {
    display: grid;
    grid-template-columns: 1rem minmax(0, 1fr);
    align-items: center;
    gap: 0.3rem;
    width: 100%;
    border-color: transparent;
    background: transparent;
    padding: 0.28rem 0.32rem;
    color: #aeb8c4;
    font-size: 0.76rem;
    text-align: left;
  }

  .compact-action {
    min-height: 1.45rem;
    padding: 0.18rem 0.38rem;
    font-size: 0.72rem;
    line-height: 1;
  }

  .compact-actions {
    display: inline-flex;
    gap: 0.2rem;
  }

  .panel-tool-button span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .panel-tool-button:hover,
  .panel-tool-button:focus,
  .panel-tool-button.active {
    border-color: #2ea987;
    background: #10211e;
    color: #e6edf3;
  }

  .tool-hud {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    gap: 0.34rem;
    height: min(var(--hud-height, 10rem), 30vh);
    min-height: 0;
    max-height: 18rem;
    overflow: hidden;
    padding: 0.22rem 0 0.08rem 1.42rem;
  }

  .notes-hud {
    grid-template-rows: minmax(0, 1fr) auto;
    gap: 0.22rem;
    height: min(var(--hud-height, 13rem), 32vh);
    max-height: 20rem;
    padding-left: 0;
  }

  .outline-hud {
    grid-template-rows: minmax(0, 1fr);
    height: min(var(--hud-height, 8rem), 24vh);
    max-height: 14rem;
  }

  .hud-resize-handle {
    width: calc(100% - 1.42rem);
    height: 5px;
    min-height: 5px;
    justify-self: end;
    border: 0;
    border-radius: 0;
    background: linear-gradient(to bottom, transparent 0 2px, #232b36 2px 3px, transparent 3px 5px);
    cursor: row-resize;
    padding: 0;
  }

  .hud-resize-handle:hover,
  .hud-resize-handle:focus {
    background: linear-gradient(to bottom, transparent 0 2px, #2ea987 2px 3px, transparent 3px 5px);
    outline: none;
  }

  .panel-resize-handle {
    grid-column: 2;
    grid-row: 1;
    width: 4px;
    min-width: 4px;
    height: 100%;
    border: 0;
    border-right: 1px solid #232b36;
    border-left: 1px solid transparent;
    border-radius: 0;
    background: #0b0f14;
    cursor: col-resize;
    padding: 0;
  }

  .panel-resize-handle:hover,
  .panel-resize-handle:focus {
    border-left-color: #2ea987;
    border-right-color: #2ea987;
    outline: none;
  }

  .panel-resize-handle:disabled {
    cursor: default;
    opacity: 0.55;
  }

  .panel-resize-handle:disabled:hover,
  .panel-resize-handle:disabled:focus {
    border-left-color: transparent;
    border-right-color: #232b36;
  }

  .right-resize-handle {
    grid-column: 4;
    border-right: 1px solid transparent;
    border-left: 1px solid #232b36;
  }

  .right-resize-handle:disabled:hover,
  .right-resize-handle:disabled:focus {
    border-right-color: transparent;
    border-left-color: #232b36;
  }

  :global(body.is-resizing-panel),
  :global(body.is-resizing-panel *) {
    cursor: col-resize !important;
    user-select: none !important;
  }

  :global(body.is-resizing-hud),
  :global(body.is-resizing-hud *) {
    cursor: row-resize !important;
    user-select: none !important;
  }

  :global(body.is-moving-tool),
  :global(body.is-moving-tool *) {
    cursor: grabbing !important;
    user-select: none !important;
  }

  .right-panel {
    display: flex;
    grid-column: 5;
    grid-row: 1;
    flex-direction: column;
    gap: 0.54rem;
    border-left: 1px solid #232b36;
    background: #0b0f14;
    padding: 0.72rem 0.64rem;
    min-width: 0;
    min-height: 0;
  }

  .outline-list {
    display: grid;
    align-content: start;
    flex: 1 1 auto;
    gap: 0.08rem;
    min-height: 0;
    overflow: auto;
  }

  .outline-row {
    overflow: hidden;
    border-radius: 4px;
    padding: 0.22rem 0.18rem;
    color: #aeb8c4;
    font-size: 0.76rem;
    line-height: 1.25;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .outline-row.level-two {
    padding-left: 0.76rem;
    color: #9aa6b2;
  }

  .outline-row.level-three {
    padding-left: 1.28rem;
    color: #8d98a6;
  }

  .outline-row:hover {
    background: #10161f;
    color: #e6edf3;
  }

  .right-panel-empty {
    display: grid;
    align-content: start;
    color: #687586;
    font-size: 0.76rem;
    line-height: 1.35;
  }

  .brand {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: start;
    gap: 0.5rem;
  }

  .brand h1 {
    margin: 0;
    color: #f0f4f8;
    font-size: 1rem;
    line-height: 1.15;
  }

  .brand p,
  .empty-state p,
  footer {
    margin: 0.15rem 0 0;
    color: #7d8896;
    font-size: 0.75rem;
  }

  .workspace-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto auto;
    gap: 0.38rem;
  }

  .notes-list {
    display: grid;
    align-content: start;
    flex: 1 1 auto;
    gap: 0.12rem;
    overflow: auto;
    min-height: 0;
  }

  .notes-count {
    overflow: hidden;
    color: #7d8896;
    font-size: 0.7rem;
    line-height: 1.2;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sidebar-empty {
    display: grid;
    align-content: start;
    color: #7d8896;
    font-size: 0.78rem;
    line-height: 1.35;
  }

  .settings-view {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    gap: 1rem;
    min-height: 0;
    color: #d7dde4;
  }

  .settings-view-header {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: start;
    gap: 0.72rem;
  }

  .settings-view-header h2 {
    margin: 0;
    color: #f0f4f8;
    font-size: 1.16rem;
    line-height: 1.2;
  }

  .settings-view-header p,
  .settings-section p {
    margin: 0.14rem 0 0;
    color: #7d8896;
    font-size: 0.75rem;
  }

  .settings-view-content {
    display: grid;
    align-content: start;
    gap: 1rem;
    overflow: auto;
    min-height: 0;
    max-width: 58rem;
  }

  .settings-section {
    display: grid;
    gap: 0.7rem;
    border-top: 1px solid #232b36;
    padding-top: 0.72rem;
  }

  .settings-section-heading {
    display: flex;
    align-items: center;
    gap: 0.42rem;
    color: #aeb8c4;
  }

  .settings-section-heading h3 {
    margin: 0;
    color: #e6edf3;
    font-size: 0.88rem;
  }

  .settings-subsection {
    display: grid;
    gap: 0.46rem;
    border-top: 1px solid #1b222c;
    padding-top: 0.62rem;
  }

  .settings-subsection h4 {
    margin: 0;
    color: #d7dde4;
    font-size: 0.8rem;
  }

  .settings-section label,
  .setting-row span {
    color: #9aa6b2;
    font-size: 0.75rem;
    font-weight: 650;
  }

  .setting-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: 1rem;
    min-height: 2.35rem;
  }

  .setting-actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: end;
    gap: 0.34rem;
  }

  .segmented-control {
    display: inline-grid;
    grid-template-columns: repeat(2, auto);
    gap: 0.16rem;
    border: 1px solid #232b36;
    border-radius: 5px;
    background: #0b0f14;
    padding: 0.12rem;
  }

  .segmented-control.three-options {
    grid-template-columns: repeat(3, auto);
  }

  .segmented-control button {
    border-color: transparent;
    padding: 0.18rem 0.48rem;
    font-size: 0.72rem;
  }

  .segmented-control button.active {
    border-color: #2ea987;
    background: #10211e;
    color: #e6edf3;
  }

  .settings-note {
    overflow: hidden;
    margin: 0;
    color: #7d8896;
    font-size: 0.68rem;
    line-height: 1.3;
    text-overflow: ellipsis;
  }

  .icon-button {
    display: grid;
    place-items: center;
    width: 1.55rem;
    height: 1.55rem;
    padding: 0;
    font-size: 0.72rem;
  }

  .icon-button.active,
  .fixed-settings-button.active {
    border-color: #2ea987;
    background: #10211e;
    color: #e6edf3;
  }

  .note-row {
    display: grid;
    grid-template-columns: 0.9rem minmax(0, 1fr);
    align-items: center;
    gap: 0.28rem;
    width: 100%;
    text-align: left;
    background: #0f141b;
    border-color: transparent;
    min-height: 1.55rem;
    padding: 0.24rem 0.38rem;
  }

  .note-row.active {
    background: #10211e;
    border-color: #2ea987;
  }

  .note-title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: #e6edf3;
    font-size: 0.78rem;
    font-weight: 650;
    line-height: 1.15;
  }

  .note-context-menu {
    position: fixed;
    z-index: 80;
    display: grid;
    gap: 0.12rem;
    min-width: 11.5rem;
    border: 1px solid #303946;
    border-radius: 6px;
    background: #10161f;
    box-shadow: 0 16px 36px rgba(0, 0, 0, 0.42);
    padding: 0.28rem;
  }

  .note-context-menu button {
    width: 100%;
    border-color: transparent;
    background: transparent;
    padding: 0.34rem 0.44rem;
    text-align: left;
    color: #d7dde4;
    font-size: 0.78rem;
  }

  .note-context-menu button:hover,
  .note-context-menu button:focus {
    border-color: transparent;
    background: #1c232d;
    outline: none;
  }

  .note-context-menu button.danger {
    color: #ffb4a8;
  }

  .note-context-menu button.danger:hover,
  .note-context-menu button.danger:focus {
    background: rgba(248, 81, 73, 0.14);
    color: #ffd2cb;
  }

  .context-menu-separator {
    height: 1px;
    margin: 0.16rem 0;
    background: #232b36;
  }

  .editor {
    display: grid;
    grid-column: 3;
    grid-row: 1;
    grid-template-rows: minmax(0, 1fr) auto;
    gap: 0.34rem;
    padding: 0.72rem;
    min-width: 0;
    min-height: 0;
    background: #0d1117;
  }

  .note-page {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    gap: 0.28rem;
    min-height: 0;
    color: #d7dde4;
  }

  .editor-header {
    display: block;
    text-align: center;
  }

  .editor-header h2 {
    overflow: hidden;
    margin: 0;
    color: #8bd5bd;
    font-size: 1.16rem;
    font-weight: 700;
    line-height: 1.2;
    text-align: center;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .mode-group {
    display: inline-grid;
    grid-auto-flow: column;
    grid-auto-columns: max-content;
    gap: 0.16rem;
    border: 1px solid #232b36;
    border-radius: 5px;
    background: #0b0f14;
    padding: 0.12rem;
  }

  .mode-button {
    border-color: transparent;
    padding: 0.16rem 0.38rem;
    font-size: 0.7rem;
  }

  .mode-button.active {
    border-color: #2ea987;
    background: #10211e;
    color: #e6edf3;
  }

  .body-shell {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    gap: 0.55rem;
    min-height: 0;
  }

  .body-shell.source-only,
  .body-shell.preview-only {
    grid-template-columns: minmax(0, 1fr);
  }

  .live-editor {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    gap: 0.1rem;
    min-height: 0;
  }

  .live-properties {
    display: grid;
    gap: 0.12rem;
    padding: 0.1rem 0 0.25rem;
  }

  .property-list {
    display: grid;
    gap: 0.08rem;
  }

  .property-row {
    display: grid;
    grid-template-columns: 1.45rem minmax(5.5rem, 0.26fr) minmax(0, 1fr) 1.35rem;
    gap: 0.22rem;
    align-items: center;
    position: relative;
    min-height: 1.34rem;
  }

  .property-type-cell {
    position: relative;
    display: grid;
    place-items: center;
    min-width: 0;
  }

  .property-type-button {
    display: grid;
    place-items: center;
    width: 1.28rem;
    height: 1.28rem;
    border-color: transparent;
    background: transparent;
    color: #8d98a6;
    padding: 0;
  }

  .property-type-button:hover,
  .property-type-button:focus,
  .property-type-button.active {
    border-color: #303946;
    background: #10161f;
    color: #d7dde4;
  }

  .property-type-menu {
    display: grid;
    position: absolute;
    top: 1.45rem;
    left: 0;
    z-index: 35;
    width: 12rem;
    border: 1px solid #2a2035;
    border-radius: 5px;
    background: #120318;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.42);
    padding: 0.25rem;
  }

  .property-type-menu button {
    display: grid;
    grid-template-columns: 1rem minmax(0, 1fr) 1rem;
    align-items: center;
    gap: 0.44rem;
    min-height: 1.72rem;
    border-color: transparent;
    background: transparent;
    color: #b9c7d5;
    padding: 0.22rem 0.38rem;
    text-align: left;
  }

  .property-type-menu button:hover,
  .property-type-menu button:focus,
  .property-type-menu button.active {
    background: #24152d;
    color: #f0f4f8;
  }

  .property-type-menu button span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .property-row input,
  .property-label {
    border-color: transparent;
    background: transparent;
    min-height: 1.26rem;
    padding: 0.04rem 0.18rem;
    font-size: 0.8rem;
  }

  .property-name-input,
  .property-label {
    color: #8d98a6;
  }

  .property-label {
    display: flex;
    align-items: center;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .property-label {
    text-transform: none;
  }

  .formatted-property-input {
    color: #d7dde4;
  }

  .token-property-field {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 0.22rem;
    min-width: 0;
    min-height: 1.26rem;
    padding: 0.02rem 0.18rem;
  }

  .property-token {
    display: inline-flex;
    align-items: center;
    max-width: 100%;
    gap: 0.12rem;
    border: 1px solid #245c50;
    border-radius: 999px;
    background: #10211e;
    color: #8bd5bd;
    padding: 0.08rem 0.22rem 0.08rem 0.36rem;
    font-size: 0.76rem;
    line-height: 1.15;
  }

  .property-token button {
    display: grid;
    place-items: center;
    width: 0.92rem;
    height: 0.92rem;
    border: 0;
    background: transparent;
    color: #61b89e;
    padding: 0;
  }

  .property-token button:hover,
  .property-token button:focus {
    color: #c4f5e5;
    outline: none;
  }

  .property-row .token-property-input {
    flex: 1 1 4.5rem;
    width: auto;
    min-width: 4.5rem;
    padding-left: 0.08rem;
  }

  .property-row input:focus {
    border-color: #303946;
    background: #10161f;
  }

  .checkbox-property-field {
    display: inline-flex;
    align-items: center;
    gap: 0.42rem;
    min-width: 0;
    min-height: 1.26rem;
    padding: 0.04rem 0.18rem;
    color: #d7dde4;
    font-size: 0.8rem;
  }

  .checkbox-property-field input {
    width: 0.92rem;
    height: 0.92rem;
    min-height: 0;
    accent-color: #2ea987;
  }

  .add-property-button {
    justify-self: start;
    border-color: transparent;
    background: transparent;
    color: #8d98a6;
    padding: 0.08rem 0.18rem;
    font-size: 0.72rem;
  }

  .add-property-button:hover {
    border-color: #303946;
    background: #10161f;
    color: #d7dde4;
  }

  .property-remove-button {
    display: grid;
    place-items: center;
    width: 1.28rem;
    height: 1.28rem;
    border-color: transparent;
    background: transparent;
    padding: 0;
    color: #5f6b78;
    font-size: 0.68rem;
    opacity: 0;
  }

  .property-row:hover .property-remove-button,
  .property-remove-button:focus {
    opacity: 1;
  }

  .property-remove-button:hover {
    border-color: #303946;
    background: #10161f;
    color: #d7dde4;
  }

  .body-editor {
    min-height: 0;
    height: 100%;
    line-height: 1.35;
    border-color: #232b36;
    background: #0b0f14;
    tab-size: 2;
    font-family:
      "SFMono-Regular", Consolas, "Liberation Mono", Menlo, ui-monospace, monospace;
    font-size: 0.88rem;
  }

  .live-body-editor {
    min-height: 0;
    height: 100%;
  }

  .body-shell.source-only .body-editor {
    border-color: transparent;
    border-radius: 0;
    background: transparent;
    padding: 0.12rem 0 0;
    box-shadow: none;
  }

  .body-shell.source-only .body-editor:focus {
    border-color: transparent;
    box-shadow: none;
  }

  .markdown-preview {
    min-height: 0;
    overflow: auto;
    border: 1px solid #232b36;
    border-radius: 5px;
    background: #0b0f14;
    padding: 0.65rem 0.72rem;
    color: #d7dde4;
    line-height: 1.35;
    font-size: 0.9rem;
    tab-size: 2;
  }

  .markdown-preview :global(*) {
    max-width: 100%;
  }

  .markdown-preview :global(h1),
  .markdown-preview :global(h2),
  .markdown-preview :global(h3) {
    margin: 0 0 0.28rem;
    color: #8bd5bd;
    line-height: 1.15;
  }

  .markdown-preview :global(h1) {
    font-size: 1.08rem;
  }

  .markdown-preview :global(h2) {
    font-size: 0.98rem;
  }

  .markdown-preview :global(h3) {
    font-size: 0.92rem;
  }

  .markdown-preview :global(p),
  .markdown-preview :global(li),
  .markdown-preview :global(ul),
  .markdown-preview :global(ol),
  .markdown-preview :global(blockquote),
  .markdown-preview :global(pre) {
    margin: 0 0 0.28rem;
  }

  .markdown-preview :global(p),
  .markdown-preview :global(li) {
    white-space: pre-wrap;
  }

  .markdown-preview :global(ul),
  .markdown-preview :global(ol) {
    padding-left: 1.36rem;
  }

  .markdown-preview :global(li:has(> input[type='checkbox'])) {
    display: flex;
    align-items: center;
    gap: 0.28rem;
    list-style: none;
    margin-left: -1.1rem;
    white-space: normal;
  }

  .markdown-preview :global(input[type='checkbox']) {
    flex: 0 0 auto;
    box-sizing: border-box;
    width: 0.82rem;
    height: 0.82rem;
    min-height: 0;
    margin: 0;
    border-color: #245c50;
    padding: 0;
    accent-color: #2ea987;
    vertical-align: -0.12rem;
  }

  .markdown-preview :global(strong) {
    color: #f0f4f8;
    font-weight: 750;
  }

  .markdown-preview :global(em) {
    color: #b9c7d5;
  }

  .markdown-preview :global(code) {
    border: 1px solid #303946;
    border-radius: 4px;
    background: #111820;
    padding: 0.08rem 0.28rem;
    color: #8bd5bd;
    font-family:
      "SFMono-Regular", Consolas, "Liberation Mono", Menlo, ui-monospace, monospace;
    font-size: 0.88em;
  }

  .markdown-preview :global(pre) {
    overflow: auto;
    border: 1px solid #303946;
    border-radius: 5px;
    background: #111820;
    padding: 0.45rem;
  }

  .markdown-preview :global(pre code) {
    border: 0;
    background: transparent;
    padding: 0;
  }

  .markdown-preview :global(a) {
    color: #4fbda0;
  }

  .markdown-preview :global(.mdp-internal-link) {
    border-radius: 4px;
    background: rgba(79, 189, 160, 0.08);
    padding: 0 0.16rem;
    color: #8bd5bd;
    text-decoration: none;
  }

  .markdown-preview :global(.mdp-internal-link:hover),
  .markdown-preview :global(.mdp-internal-link:focus) {
    background: rgba(79, 189, 160, 0.16);
    color: #c4f5e5;
    text-decoration: underline;
  }

  .markdown-preview :global(.mdp-inline-tag) {
    border: 1px solid #245c50;
    border-radius: 999px;
    background: #10211e;
    color: #8bd5bd;
    padding: 0 0.22rem;
    font-weight: 650;
  }

  .markdown-preview :global(.mdp-blank-line) {
    display: block;
    height: 1.35em;
    margin: 0;
  }

  .markdown-preview :global([data-mdp-rule='underline']) {
    display: block;
    height: 0;
    margin: 0.35rem 0;
    border: 0;
    border-top: 2px solid #5d6b7c;
    opacity: 1;
  }

  .empty-state {
    display: grid;
    place-content: center;
    min-height: 100%;
    text-align: center;
    color: #d7dde4;
  }

  .empty-state h2 {
    margin: 0 0 0.35rem;
    font-size: 1.1rem;
  }

  footer {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: 0.5rem;
    border-top: 1px solid #232b36;
    padding-top: 0.28rem;
  }

  footer span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  @media (max-width: 399px) {
    .app-shell {
      grid-template-columns: 1fr;
      grid-template-rows: auto minmax(0, 1fr) auto;
    }

    .sidebar {
      grid-column: 1;
      grid-row: 1;
      border-right: 0;
      border-bottom: 1px solid #232b36;
    }

    .panel-resize-handle {
      display: none;
    }

    .right-panel {
      grid-column: 1;
      grid-row: 3;
      border-top: 1px solid #232b36;
      border-left: 0;
      max-height: 28vh;
    }

    .editor {
      grid-column: 1;
      grid-row: 2;
    }

  }
</style>
