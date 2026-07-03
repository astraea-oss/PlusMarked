<script module lang="ts">
  import { getNoteSource, saveNoteSource } from './api';

  const canvasDocumentSourceCache: Record<string, string> = {};
  const canvasDocumentSourceInflight = new Map<string, Promise<string>>();

  function cachedCanvasDocumentSource(id: string): Promise<string> {
    if (canvasDocumentSourceCache[id]) return Promise.resolve(canvasDocumentSourceCache[id]);

    const existing = canvasDocumentSourceInflight.get(id);
    if (existing) return existing;

    const request = getNoteSource(id)
      .then((source) => {
        canvasDocumentSourceCache[id] = source.source;
        return source.source;
      })
      .finally(() => {
        canvasDocumentSourceInflight.delete(id);
      });

    canvasDocumentSourceInflight.set(id, request);
    return request;
  }

  async function saveCachedCanvasDocumentSource(id: string, source: string) {
    const result = await saveNoteSource({ id, source });
    canvasDocumentSourceCache[id] = source;
    return result.note;
  }
</script>

<script lang="ts">
  import { tick } from 'svelte';
  import DOMPurify from 'dompurify';
  import { marked } from 'marked';
  import {
    CalendarClock,
    CalendarDays,
    ExternalLink,
    FileText,
    Globe2,
    Group,
    Hash,
    List,
    Plus,
    SquareCheck,
    Tags,
    Trash2,
    Type,
    X
  } from '@lucide/svelte';
  import MarkdownPlusEditor from './MarkdownPlusEditor.svelte';
  import type { NoteSummary } from './types';

  type CanvasNodeType = 'text' | 'file' | 'link' | 'group';
  type CanvasSide = 'top' | 'right' | 'bottom' | 'left';
  type ResizeCorner = 'nw' | 'ne' | 'sw' | 'se';
  type ConnectionDraft = {
    pointerId: number;
    fromNode: string;
    fromSide: CanvasSide;
    x1: number;
    y1: number;
    x2: number;
    y2: number;
  };
  type CanvasNode = {
    id: string;
    type: CanvasNodeType;
    x: number;
    y: number;
    width: number;
    height: number;
    color?: string;
    text?: string;
    file?: string;
    subpath?: string;
    url?: string;
    label?: string;
    background?: string;
    backgroundStyle?: string;
  };
  type CanvasEdge = {
    id: string;
    fromNode: string;
    fromSide?: CanvasSide;
    fromEnd?: 'none' | 'arrow';
    toNode: string;
    toSide?: CanvasSide;
    toEnd?: 'none' | 'arrow';
    color?: string;
    label?: string;
  };
  type ParsedCanvas = {
    nodes: CanvasNode[];
    edges: CanvasEdge[];
    raw: Record<string, unknown>;
    error: string;
  };
  type CanvasBounds = {
    minX: number;
    minY: number;
    width: number;
    height: number;
  };
  type CanvasContextMenu = {
    x: number;
    y: number;
    worldX: number;
    worldY: number;
    nodeId?: string;
  };
  type CanvasDraftKind = 'text' | 'file' | 'group';
  type CanvasDraft = {
    mode: 'create' | 'edit';
    kind: CanvasDraftKind;
    x: number;
    y: number;
    value: string;
    nodeId?: string;
  };
  type FileCardPreview = {
    title: string;
    kind: string;
    status: 'missing' | 'loading' | 'ready';
    properties: FileCardProperty[];
    lines: string[];
    bodyHtml: string;
  };
  type LinkCompletion = {
    label: string;
    detail: string;
    type: string;
    apply?: string;
  };
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
  type FileCardProperty = {
    key: string;
    label: string;
    type: 'text' | 'tokens' | 'empty';
    value: string;
    tokens: string[];
  };

  export let source = '';
  export let notes: NoteSummary[] = [];
  export let documentSources: Record<string, string> = {};
  export let onOpenNote: (id: string) => void = () => {};
  export let onNotesChanged: (notes: NoteSummary[]) => void = () => {};
  export let onChange: (source: string) => void = () => {};
  export let setStatus: (message: string) => void = () => {};

  const canvasStableOrigin = 10000;
  const snapGridSize = 32;
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

  let scale = 1;
  let panX = -canvasStableOrigin;
  let panY = -canvasStableOrigin;
  let dragStart: { pointerId: number; x: number; y: number; panX: number; panY: number } | null = null;
  let nodeDrag:
    | { pointerId: number; nodeId: string; x: number; y: number; startX: number; startY: number }
    | null = null;
  let nodeResize:
    | {
        pointerId: number;
        nodeId: string;
        corner: ResizeCorner;
        x: number;
        y: number;
        startX: number;
        startY: number;
        startWidth: number;
        startHeight: number;
      }
    | null = null;
  let connectionDraft: ConnectionDraft | null = null;
  let contextMenu: CanvasContextMenu | null = null;
  let contextMenuElement: HTMLDivElement;
  let draftDialog: CanvasDraft | null = null;
  let draftInputElement: HTMLInputElement;
  let loadedDocumentSources: Record<string, string> = {};
  let loadingDocumentSourceIds = new Set<string>();
  let failedDocumentSourceMessages: Record<string, string> = {};
  let editingFileNodeId: string | null = null;
  let editingFileSource = '';
  let editingFileProperties: PropertyRow[] = [];
  let editingFileBody = '';
  let editingPropertyTypeOverrides: Record<string, PropertyType> = {};
  let editingTokenPropertyDrafts: Record<number, string> = {};
  let editingPropertyTypeMenuIndex: number | null = null;
  let editingFileSaveTimer: ReturnType<typeof setTimeout> | null = null;
  let savingFileNodeId: string | null = null;

  $: canvas = parseCanvas(source);
  $: bounds = canvasBounds(canvas.nodes);
  $: renderedCanvasNodes = [...canvas.nodes].sort((left, right) => nodeLayer(left) - nodeLayer(right));
  $: nodeById = new Map(canvas.nodes.map((node) => [node.id, node]));
  $: canvasDocumentResolutionSignature = notes
    .map((note) => `${note.id}:${note.title}:${note.path}:${note.note_type}`)
    .join('|');
  $: internalLinkCompletions = buildInternalLinkCompletions(notes);
  $: documentMatches = draftDialog?.kind === 'file' ? matchingDocuments(draftDialog.value) : [];
  $: canvasFileTargetIds = canvasDocumentTargetIdsFor(canvas.nodes, canvasDocumentResolutionSignature);
  $: canvasFileSourceSignature = Array.from(new Set(canvasFileTargetIds))
    .sort()
    .map(
      (id) =>
        `${id}:${documentSources[id]?.length ?? 0}:${loadedDocumentSources[id]?.length ?? 0}:${
          loadingDocumentSourceIds.has(id) ? '1' : '0'
        }:${failedDocumentSourceMessages[id] ?? ''}`
    )
    .join('|');
  $: syncCanvasDocumentSources(canvasFileTargetIds, canvasFileSourceSignature);

  function parseCanvas(input: string): ParsedCanvas {
    if (!input.trim()) {
      return { nodes: [], edges: [], raw: {}, error: '' };
    }

    try {
      const parsed = JSON.parse(input) as Record<string, unknown>;
      const nodes = Array.isArray(parsed.nodes) ? parsed.nodes.map(normalizeNode).filter(isCanvasNode) : [];
      const edges = Array.isArray(parsed.edges) ? parsed.edges.map(normalizeEdge).filter(isCanvasEdge) : [];
      return { nodes, edges, raw: parsed, error: '' };
    } catch (error) {
      return {
        nodes: [],
        edges: [],
        raw: {},
        error: error instanceof Error ? error.message : String(error)
      };
    }
  }

  function normalizeNode(value: unknown): Partial<CanvasNode> {
    const node = value && typeof value === 'object' ? value as Record<string, unknown> : {};
    return {
      id: String(node.id ?? ''),
      type: String(node.type ?? '') as CanvasNodeType,
      x: Number(node.x),
      y: Number(node.y),
      width: Number(node.width),
      height: Number(node.height),
      color: typeof node.color === 'string' ? node.color : undefined,
      text: typeof node.text === 'string' ? node.text : undefined,
      file: typeof node.file === 'string' ? node.file : undefined,
      subpath: typeof node.subpath === 'string' ? node.subpath : undefined,
      url: typeof node.url === 'string' ? node.url : undefined,
      label: typeof node.label === 'string' ? node.label : undefined,
      background: typeof node.background === 'string' ? node.background : undefined,
      backgroundStyle: typeof node.backgroundStyle === 'string' ? node.backgroundStyle : undefined
    };
  }

  function normalizeEdge(value: unknown): Partial<CanvasEdge> {
    const edge = value && typeof value === 'object' ? value as Record<string, unknown> : {};
    return {
      id: String(edge.id ?? ''),
      fromNode: String(edge.fromNode ?? ''),
      fromSide: normalizeSide(edge.fromSide),
      fromEnd: edge.fromEnd === 'arrow' ? 'arrow' : 'none',
      toNode: String(edge.toNode ?? ''),
      toSide: normalizeSide(edge.toSide),
      toEnd: edge.toEnd === 'none' ? 'none' : 'arrow',
      color: typeof edge.color === 'string' ? edge.color : undefined,
      label: typeof edge.label === 'string' ? edge.label : undefined
    };
  }

  function isCanvasNode(node: Partial<CanvasNode>): node is CanvasNode {
    return Boolean(
      node.id &&
        ['text', 'file', 'link', 'group'].includes(String(node.type)) &&
        Number.isFinite(node.x) &&
        Number.isFinite(node.y) &&
        Number.isFinite(node.width) &&
        Number.isFinite(node.height)
    );
  }

  function isCanvasEdge(edge: Partial<CanvasEdge>): edge is CanvasEdge {
    return Boolean(edge.id && edge.fromNode && edge.toNode);
  }

  function normalizeSide(value: unknown): CanvasSide | undefined {
    return value === 'top' || value === 'right' || value === 'bottom' || value === 'left'
      ? value
      : undefined;
  }

  function canvasBounds(nodes: CanvasNode[]): CanvasBounds {
    if (!nodes.length) {
      return {
        minX: -canvasStableOrigin,
        minY: -canvasStableOrigin,
        width: canvasStableOrigin * 2,
        height: canvasStableOrigin * 2
      };
    }

    const padding = 120;
    const minX = Math.min(-canvasStableOrigin, ...nodes.map((node) => node.x - padding));
    const minY = Math.min(-canvasStableOrigin, ...nodes.map((node) => node.y - padding));
    const maxX = Math.max(canvasStableOrigin, ...nodes.map((node) => node.x + node.width + padding));
    const maxY = Math.max(canvasStableOrigin, ...nodes.map((node) => node.y + node.height + padding));
    return {
      minX,
      minY,
      width: maxX - minX,
      height: maxY - minY
    };
  }

  function canvasX(node: CanvasNode) {
    return node.x - bounds.minX;
  }

  function canvasY(node: CanvasNode) {
    return node.y - bounds.minY;
  }

  function nodeStyle(node: CanvasNode) {
    const accent = canvasColor(node.color);
    return [
      `left: ${canvasX(node)}px`,
      `top: ${canvasY(node)}px`,
      `width: ${Math.max(80, node.width)}px`,
      `height: ${Math.max(48, node.height)}px`,
      `--canvas-accent: ${accent}`
    ].join('; ');
  }

  function edgeLine(edge: CanvasEdge) {
    const from = nodeById.get(edge.fromNode);
    const to = nodeById.get(edge.toNode);
    if (!from || !to) return null;

    const start = connectorPoint(from, edge.fromSide);
    const end = connectorPoint(to, edge.toSide);
    return {
      x1: start.x,
      y1: start.y,
      x2: end.x,
      y2: end.y,
      midX: (start.x + end.x) / 2,
      midY: (start.y + end.y) / 2,
      color: canvasColor(edge.color)
    };
  }

  function connectorPoint(node: CanvasNode, side?: CanvasSide) {
    const x = canvasX(node);
    const y = canvasY(node);
    if (side === 'top') return { x: x + node.width / 2, y };
    if (side === 'right') return { x: x + node.width, y: y + node.height / 2 };
    if (side === 'bottom') return { x: x + node.width / 2, y: y + node.height };
    if (side === 'left') return { x, y: y + node.height / 2 };
    return { x: x + node.width / 2, y: y + node.height / 2 };
  }

  function canvasColor(value?: string) {
    if (value?.startsWith('#')) return value;
    if (value === '1') return '#f87171';
    if (value === '2') return '#fb923c';
    if (value === '3') return '#facc15';
    if (value === '4') return '#5ee68f';
    if (value === '5') return '#76e4d0';
    if (value === '6') return '#c084fc';
    return '#76e4d0';
  }

  function canvasIcon(type: CanvasNodeType) {
    if (type === 'text') return Type;
    if (type === 'file') return FileText;
    if (type === 'link') return Globe2;
    return Group;
  }

  function nodeLayer(node: CanvasNode) {
    return node.type === 'group' ? 0 : 1;
  }

  function commitCanvas(nodes: CanvasNode[], edges = canvas.edges) {
    onChange(`${JSON.stringify({ ...canvas.raw, nodes, edges }, null, 2)}\n`);
  }

  function createEdge(fromNode: string, fromSide: CanvasSide, toNode: string, toSide: CanvasSide) {
    if (fromNode === toNode) return;
    const edge: CanvasEdge = {
      id: createId(),
      fromNode,
      fromSide,
      fromEnd: 'none',
      toNode,
      toSide,
      toEnd: 'arrow'
    };
    commitCanvas(canvas.nodes, [...canvas.edges, edge]);
    setStatus('Added connector.');
  }

  function createNode(type: CanvasNodeType, fields: Partial<CanvasNode> = {}, position = defaultNodePosition()) {
    const node: CanvasNode = {
      id: createId(),
      type,
      x: snapToGrid(position.x),
      y: snapToGrid(position.y),
      width: type === 'group' ? 420 : 280,
      height: type === 'group' ? 220 : 160,
      ...fields
    };
    node.width = snapSize(node.width, minimumNodeWidth(node));
    node.height = snapSize(node.height, minimumNodeHeight(node));
    commitCanvas([...canvas.nodes, node]);
    setStatus(`Added ${type} card.`);
  }

  function defaultNodePosition() {
    return {
      x: snapToGrid(Math.round((80 - panX) / scale + bounds.minX)),
      y: snapToGrid(Math.round((80 - panY) / scale + bounds.minY))
    };
  }

  function createId() {
    return globalThis.crypto?.randomUUID?.() ?? `node-${Date.now()}-${Math.random().toString(16).slice(2)}`;
  }

  function addTextNode() {
    openCreateDraft('text');
  }

  function addFileNode() {
    openCreateDraft('file');
  }

  function addGroupNode() {
    openCreateDraft('group');
  }

  function insertTextNodeAt(position: { x: number; y: number }) {
    openCreateDraft('text', position);
  }

  function insertDocumentNodeAt(position: { x: number; y: number }) {
    openCreateDraft('file', position);
  }

  function insertGroupNodeAt(position: { x: number; y: number }) {
    openCreateDraft('group', position);
  }

  function updateNode(nodeId: string, updates: Partial<CanvasNode>) {
    commitCanvas(canvas.nodes.map((node) => (node.id === nodeId ? { ...node, ...updates } : node)));
  }

  function deleteNode(nodeId: string) {
    commitCanvas(
      canvas.nodes.filter((node) => node.id !== nodeId),
      canvas.edges.filter((edge) => edge.fromNode !== nodeId && edge.toNode !== nodeId)
    );
    setStatus('Deleted card.');
  }

  function duplicateNode(node: CanvasNode) {
    const duplicate: CanvasNode = {
      ...node,
      id: createId(),
      x: snapToGrid(node.x + snapGridSize),
      y: snapToGrid(node.y + snapGridSize)
    };
    commitCanvas([...canvas.nodes, duplicate]);
    setStatus('Duplicated card.');
  }

  function editNode(node: CanvasNode) {
    openEditDraft(node);
  }

  function openCreateDraft(kind: CanvasDraftKind, position = defaultNodePosition()) {
    openDraft({
      mode: 'create',
      kind,
      x: position.x,
      y: position.y,
      value: kind === 'group' ? 'Group' : ''
    });
  }

  function openEditDraft(node: CanvasNode) {
    const valueByType: Record<CanvasNodeType, string> = {
      text: node.text ?? '',
      file: node.file ?? '',
      link: node.url ?? '',
      group: node.label ?? 'Group'
    };
    openDraft({
      mode: 'edit',
      kind: node.type === 'link' ? 'text' : node.type,
      x: node.x,
      y: node.y,
      value: valueByType[node.type],
      nodeId: node.id
    });
  }

  function openDraft(draft: CanvasDraft) {
    closeContextMenu();
    draftDialog = draft;
    void tick().then(() => {
      if (draft.kind !== 'text') {
        draftInputElement?.focus();
        draftInputElement?.select();
      }
    });
  }

  function closeDraft() {
    draftDialog = null;
  }

  function updateDraftValue(value: string) {
    if (!draftDialog) return;
    draftDialog = { ...draftDialog, value };
  }

  function submitDraft(document?: NoteSummary) {
    if (!draftDialog || !draftCanSubmit(draftDialog, document)) return;

    const value = draftDialog.value.trim();
    const targetDocument = document ?? (draftDialog.kind === 'file' ? findDocumentByInput(value) : null);
    const fields = draftFields(draftDialog, targetDocument);
    if (!fields) return;

    if (draftDialog.mode === 'edit' && draftDialog.nodeId) {
      updateNode(draftDialog.nodeId, fields);
      setStatus('Updated card.');
    } else {
      createNode(draftDialog.kind, fields, { x: draftDialog.x, y: draftDialog.y });
    }

    if (targetDocument) {
      void cachedCanvasDocumentSource(targetDocument.id);
    }

    closeDraft();
  }

  function draftFields(draft: CanvasDraft, document?: NoteSummary | null): Partial<CanvasNode> | null {
    const value = draft.value.trim();
    if (draft.kind === 'text') return { text: draft.value };
    if (draft.kind === 'file') {
      const file = document ? fileName(document.path) : value;
      return file ? { file } : null;
    }
    return { label: value || 'Group' };
  }

  function draftCanSubmit(draft: CanvasDraft, document?: NoteSummary) {
    if (draft.kind === 'text' || draft.kind === 'group') return true;
    if (document) return true;
    return Boolean(draft.value.trim());
  }

  function draftTitle(draft: CanvasDraft) {
    const verb = draft.mode === 'edit' ? 'Edit' : 'Insert';
    if (draft.kind === 'text') return `${verb} text card`;
    if (draft.kind === 'file') return `${verb} document card`;
    return `${verb} group`;
  }

  function draftPlaceholder(draft: CanvasDraft) {
    if (draft.kind === 'text') return 'Card text';
    if (draft.kind === 'file') return 'Search notes, bases, canvases, or type a file path';
    return 'Group name';
  }

  function startNodeDrag(event: PointerEvent, node: CanvasNode) {
    if (event.button !== 0) return;
    if (
      event.target instanceof HTMLElement
      && (event.target.closest('button') || event.target.closest('.embedded-properties') || event.target.closest('.embedded-note-editor'))
    ) return;

    nodeDrag = {
      pointerId: event.pointerId,
      nodeId: node.id,
      x: event.clientX,
      y: event.clientY,
      startX: node.x,
      startY: node.y
    };
  }

  function startConnection(event: PointerEvent, node: CanvasNode, side: CanvasSide) {
    if (event.button !== 0) return;
    event.preventDefault();
    event.stopPropagation();
    closeContextMenu();

    const start = connectorPoint(node, side);
    connectionDraft = {
      pointerId: event.pointerId,
      fromNode: node.id,
      fromSide: side,
      x1: start.x,
      y1: start.y,
      x2: start.x,
      y2: start.y
    };
  }

  function pointerCanvasPoint(event: PointerEvent) {
    const stage = event.currentTarget instanceof HTMLElement
      ? event.currentTarget.querySelector<HTMLElement>('.canvas-stage')
      : null;
    const rect = stage?.getBoundingClientRect();
    if (!rect) return { x: 0, y: 0 };
    return {
      x: (event.clientX - rect.left) / scale,
      y: (event.clientY - rect.top) / scale
    };
  }

  function moveConnectionDraft(event: PointerEvent) {
    if (!connectionDraft || connectionDraft.pointerId !== event.pointerId) return false;
    const point = pointerCanvasPoint(event);
    connectionDraft = {
      ...connectionDraft,
      x2: point.x,
      y2: point.y
    };
    return true;
  }

  function endConnectionDraft(event: PointerEvent) {
    if (!connectionDraft || connectionDraft.pointerId !== event.pointerId) return false;

    const target = event.target instanceof HTMLElement
      ? event.target.closest<HTMLElement>('.connection-handle')
      : null;
    const toNode = target?.dataset.nodeId;
    const toSide = normalizeSide(target?.dataset.side);

    if (toNode && toSide) {
      createEdge(connectionDraft.fromNode, connectionDraft.fromSide, toNode, toSide);
    }

    connectionDraft = null;
    return true;
  }

  function startNodeResize(event: PointerEvent, node: CanvasNode, corner: ResizeCorner) {
    if (event.button !== 0) return;
    event.preventDefault();
    closeContextMenu();

    nodeResize = {
      pointerId: event.pointerId,
      nodeId: node.id,
      corner,
      x: event.clientX,
      y: event.clientY,
      startX: node.x,
      startY: node.y,
      startWidth: node.width,
      startHeight: node.height
    };
  }

  function moveNodeDrag(event: PointerEvent) {
    if (!nodeDrag || nodeDrag.pointerId !== event.pointerId) return false;

    const nextX = snapToGrid(nodeDrag.startX + (event.clientX - nodeDrag.x) / scale);
    const nextY = snapToGrid(nodeDrag.startY + (event.clientY - nodeDrag.y) / scale);
    updateNode(nodeDrag.nodeId, { x: nextX, y: nextY });
    return true;
  }

  function moveNodeResize(event: PointerEvent) {
    if (!nodeResize || nodeResize.pointerId !== event.pointerId) return false;

    const node = nodeById.get(nodeResize.nodeId);
    if (!node) return false;

    const minWidth = minimumNodeWidth(node);
    const minHeight = minimumNodeHeight(node);
    const deltaX = Math.round((event.clientX - nodeResize.x) / scale);
    const deltaY = Math.round((event.clientY - nodeResize.y) / scale);
    let x = nodeResize.startX;
    let y = nodeResize.startY;
    let width = nodeResize.startWidth;
    let height = nodeResize.startHeight;

    if (nodeResize.corner.includes('e')) {
      width = snapSize(nodeResize.startWidth + deltaX, minWidth);
    }
    if (nodeResize.corner.includes('s')) {
      height = snapSize(nodeResize.startHeight + deltaY, minHeight);
    }
    if (nodeResize.corner.includes('w')) {
      width = snapSize(nodeResize.startWidth - deltaX, minWidth);
      x = snapToGrid(nodeResize.startX + nodeResize.startWidth - width);
    }
    if (nodeResize.corner.includes('n')) {
      height = snapSize(nodeResize.startHeight - deltaY, minHeight);
      y = snapToGrid(nodeResize.startY + nodeResize.startHeight - height);
    }

    updateNode(nodeResize.nodeId, { x, y, width, height });
    return true;
  }

  function minimumNodeWidth(node: CanvasNode) {
    if (node.type === 'group') return 180;
    return 140;
  }

  function minimumNodeHeight(node: CanvasNode) {
    if (node.type === 'group') return 120;
    return 90;
  }

  function snapToGrid(value: number) {
    return Math.round(value / snapGridSize) * snapGridSize;
  }

  function snapSize(value: number, minimum: number) {
    return Math.max(snapToGrid(value), snapToGrid(minimum));
  }

  function endNodeDrag(event: PointerEvent) {
    if (nodeDrag?.pointerId === event.pointerId) {
      nodeDrag = null;
      setStatus('Canvas saved.');
    }
    if (nodeResize?.pointerId === event.pointerId) {
      nodeResize = null;
      setStatus('Canvas saved.');
    }
  }

  function findDocumentByInput(input: string) {
    const normalized = normalizeStem(pathStem(input));
    if (!normalized) return null;
    return notes.find((note) => documentMatchesNormalizedTarget(note, normalized)) ?? null;
  }

  function matchingDocuments(input: string) {
    const query = normalizeStem(input);
    const matches = query
      ? notes.filter((note) => {
          const haystack = [
            note.title,
            pathStem(note.path),
            fileName(note.path),
            documentTypeLabel(note)
          ]
            .map(normalizeStem)
            .join(' ');
          return haystack.includes(query);
        })
      : notes;

    return matches.slice(0, 8);
  }

  function documentTypeLabel(note: NoteSummary) {
    if (note.note_type === 'base') return 'base';
    if (note.note_type === 'canvas') return 'canvas';
    return 'note';
  }

  function buildInternalLinkCompletions(items: NoteSummary[]): LinkCompletion[] {
    const seen = new Set<string>();
    const completions: LinkCompletion[] = [];

    for (const item of items) {
      const label = pathStem(item.path) || item.title;
      const normalized = normalizeStem(label);
      if (!label || seen.has(normalized)) continue;

      seen.add(normalized);
      completions.push({
        label,
        apply: label,
        type: item.note_type === 'note' ? 'text' : 'class',
        detail: documentTypeLabel(item)
      });
    }

    return completions.sort((left, right) =>
      left.detail.localeCompare(right.detail) || left.label.localeCompare(right.label)
    );
  }

  function resolveInternalLink(target: string) {
    const normalized = normalizeStem(pathStem(target));
    if (!normalized) return null;
    return notes.find((note) => documentMatchesNormalizedTarget(note, normalized))?.id ?? null;
  }

  function internalLinkExists(target: string) {
    return Boolean(resolveInternalLink(target));
  }

  function openInternalLinkTarget(target: string) {
    const id = resolveInternalLink(target);
    if (!id) {
      setStatus(`Linked note not found: ${target}.`);
      return;
    }
    onOpenNote(id);
  }

  function externalEmbedForTarget(label: string, rawUrl: string) {
    const url = normalizeExternalUrl(rawUrl);
    return {
      label: label.trim() || url,
      url,
      embedUrl: null
    };
  }

  function canvasDocumentTargetIdsFor(nodes: CanvasNode[], _resolutionSignature: string) {
    const fileIds = nodes
      .filter((node) => node.type === 'file')
      .map((node) => fileNodeTarget(node)?.id)
      .filter((id): id is string => Boolean(id));
    const textEmbedIds = nodes
      .filter((node) => node.type === 'text')
      .flatMap((node) => internalEmbedTargets(node.text ?? ''))
      .map(resolveInternalLink)
      .filter((id): id is string => Boolean(id));

    return Array.from(new Set([...fileIds, ...textEmbedIds]));
  }

  function internalEmbedTargets(source: string) {
    return Array.from(source.matchAll(/\[\[!([^\]\n]+)\]\]/g))
      .map((match) => (match[1] ?? '').split('|')[0].trim())
      .filter(Boolean);
  }

  function syncCanvasDocumentSources(targetIds: string[], _signature: string) {
    const ids = Array.from(new Set(targetIds));
    const cached = Object.fromEntries(
      ids
        .filter((id) => (documentSources[id] || canvasDocumentSourceCache[id]) && !loadedDocumentSources[id])
        .map((id) => [id, documentSources[id] || canvasDocumentSourceCache[id]])
    );
    if (Object.keys(cached).length) {
      loadedDocumentSources = {
        ...loadedDocumentSources,
        ...cached
      };
      failedDocumentSourceMessages = Object.fromEntries(
        Object.entries(failedDocumentSourceMessages).filter(([id]) => !cached[id])
      );
    }

    const missing = ids.filter(
      (id) =>
        !documentSources[id]
        && !loadedDocumentSources[id]
        && !loadingDocumentSourceIds.has(id)
        && !failedDocumentSourceMessages[id]
    );
    if (!missing.length) return;

    void loadCanvasDocumentSources(missing);
  }

  async function loadCanvasDocumentSources(ids: string[]) {
    const missing = ids.filter((id) => !loadingDocumentSourceIds.has(id));
    if (!missing.length) return;

    loadingDocumentSourceIds = new Set([...loadingDocumentSourceIds, ...missing]);
    try {
      const loaded = await Promise.all(
        missing.map(async (id) => [id, await cachedCanvasDocumentSource(id)] as const)
      );
      loadedDocumentSources = {
        ...loadedDocumentSources,
        ...Object.fromEntries(loaded)
      };
      failedDocumentSourceMessages = Object.fromEntries(
        Object.entries(failedDocumentSourceMessages).filter(([id]) => !missing.includes(id))
      );
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      failedDocumentSourceMessages = {
        ...failedDocumentSourceMessages,
        ...Object.fromEntries(missing.map((id) => [id, message]))
      };
      setStatus(message);
    } finally {
      loadingDocumentSourceIds = new Set([...loadingDocumentSourceIds].filter((id) => !missing.includes(id)));
    }
  }

  function fileNodeTarget(node: CanvasNode) {
    if (!node.file) return null;
    const normalized = normalizeStem(pathStem(node.file));
    if (!normalized) return null;
    return notes.find((note) => documentMatchesNormalizedTarget(note, normalized)) ?? null;
  }

  function documentMatchesNormalizedTarget(note: NoteSummary, normalized: string) {
    return normalizeStem(note.title) === normalized
      || normalizeStem(pathStem(note.path)) === normalized
      || normalizeStem(fileName(note.path)) === normalized;
  }

  function fileNodePreview(
    node: CanvasNode,
    _sourceSignature: string,
    _resolutionSignature: string
  ): FileCardPreview {
    const target = fileNodeTarget(node);
    if (!target) {
      return {
        title: fileNodeTitle(node),
        kind: 'missing',
        status: 'missing',
        properties: [],
        lines: ['Document not found.'],
        bodyHtml: ''
      };
    }

    const source = documentSources[target.id] || loadedDocumentSources[target.id];
    if (!source) {
      const failedMessage = failedDocumentSourceMessages[target.id];
      return {
        title: target.title,
        kind: documentTypeLabel(target),
        status: failedMessage ? 'missing' : 'loading',
        properties: [textProperty('type', 'Type', documentTypeLabel(target))],
        lines: [failedMessage ? `Preview failed: ${failedMessage}` : 'Loading preview...'],
        bodyHtml: ''
      };
    }

    if (target.note_type === 'base') return basePreview(target, source);
    if (target.note_type === 'canvas') return canvasPreview(target, source);
    return notePreview(target, source);
  }

  function notePreview(note: NoteSummary, source: string): FileCardPreview {
    const split = splitMarkdownPlusSource(source);
    const properties = parseFrontmatterProperties(split.frontmatter);
    return {
      title: unquoteScalar(properties.title) || note.title,
      kind: 'note',
      status: 'ready',
      properties: previewPropertyRows(properties),
      lines: bodyPreviewLines(split.body),
      bodyHtml: renderMarkdownPlusBody(split.body)
    };
  }

  function basePreview(note: NoteSummary, source: string): FileCardPreview {
    const viewCount = (source.match(/^\s*-\s+type:/gm) ?? []).length;
    const propertyCount = (source.match(/^\s{2}[A-Za-z0-9_-]+:\s*$/gm) ?? []).length;
    return {
      title: note.title,
      kind: 'base',
      status: 'ready',
      properties: [
        textProperty('type', 'Type', 'base'),
        textProperty('views', 'Views', String(viewCount)),
        textProperty('properties', 'Properties', String(propertyCount))
      ],
      lines: ['Base view configuration'],
      bodyHtml: ''
    };
  }

  function canvasPreview(note: NoteSummary, source: string): FileCardPreview {
    try {
      const parsed = JSON.parse(source) as { nodes?: unknown[]; edges?: unknown[] };
      return {
        title: note.title,
        kind: 'canvas',
        status: 'ready',
        properties: [
          textProperty('type', 'Type', 'canvas'),
          textProperty('nodes', 'Nodes', String(Array.isArray(parsed.nodes) ? parsed.nodes.length : 0)),
          textProperty('edges', 'Edges', String(Array.isArray(parsed.edges) ? parsed.edges.length : 0))
        ],
        lines: ['Canvas document'],
        bodyHtml: ''
      };
    } catch {
      return {
        title: note.title,
        kind: 'canvas',
        status: 'ready',
        properties: [textProperty('type', 'Type', 'canvas')],
        lines: ['Invalid canvas JSON'],
        bodyHtml: ''
      };
    }
  }

  function splitMarkdownPlusSource(input: string) {
    if (!input.startsWith('---')) return { frontmatter: '', body: input };
    const delimiter = input.indexOf('\n---', 3);
    if (delimiter === -1) return { frontmatter: '', body: input };
    return {
      frontmatter: input.slice(3, delimiter).trim(),
      body: input.slice(delimiter + 4).replace(/^\r?\n+/, '')
    };
  }

  function replaceMarkdownPlusBody(input: string, body: string) {
    if (!input.startsWith('---')) return body;
    const delimiter = input.indexOf('\n---', 3);
    if (delimiter === -1) return body;
    return `${input.slice(0, delimiter + 4)}\n${body}`;
  }

  function sourceFromEditableFields(properties: PropertyRow[], body: string) {
    const yaml = properties
      .filter((property) => property.key.trim())
      .map((property) => `${property.key.trim()}: ${property.value}`)
      .join('\n');

    return `---\n${yaml}\n---\n${body}`;
  }

  function propertyRowsFromFrontmatter(frontmatter: string): PropertyRow[] {
    return frontmatter
      .split(/\r?\n/)
      .map((line) => line.match(/^([A-Za-z0-9_-]+):\s*(.*)$/))
      .filter((match): match is RegExpMatchArray => Boolean(match))
      .map((match) => ({
        key: match[1],
        value: match[2] ?? ''
      }));
  }

  function parseFrontmatterProperties(frontmatter: string) {
    const properties: Record<string, string> = {};
    for (const line of frontmatter.split(/\r?\n/)) {
      const match = line.match(/^([A-Za-z0-9_-]+):\s*(.*)$/);
      if (match) properties[match[1]] = match[2] ?? '';
    }
    return properties;
  }

  function previewPropertyRows(properties: Record<string, string>) {
    const priority = ['id', 'title', 'created_at', 'updated_at', 'tags', 'aliases', 'type'];
    const keys = [
      ...priority,
      ...Object.keys(properties).filter((key) => !priority.includes(key))
    ].filter((key, index, all) => all.indexOf(key) === index && Object.prototype.hasOwnProperty.call(properties, key));

    return keys.map((key) => propertyDisplayRow(key, properties[key] ?? ''));
  }

  function propertyLabel(key: string) {
    return systemPropertyLabels[key.trim()] ?? key.replace(/[_-]+/g, ' ');
  }

  function isSystemProperty(key: string) {
    return Object.prototype.hasOwnProperty.call(systemPropertyLabels, key.trim());
  }

  function propertyDisplayRow(key: string, value: string): FileCardProperty {
    const label = propertyLabel(key);
    if (key === 'tags') {
      const tokens = parseTagValue(value);
      return tokens.length ? tokenProperty(key, label, tokens) : emptyProperty(key, label, 'Add tags');
    }
    if (key === 'aliases') {
      const tokens = parseListValue(value);
      return tokens.length ? tokenProperty(key, label, tokens) : emptyProperty(key, label, 'Add aliases');
    }

    const text = propertyDisplayValue(key, value).text;
    return text ? textProperty(key, label, text) : emptyProperty(key, label, 'Empty');
  }

  function textProperty(key: string, label: string, value: string): FileCardProperty {
    return { key, label, type: 'text', value, tokens: [] };
  }

  function tokenProperty(key: string, label: string, tokens: string[]): FileCardProperty {
    return { key, label, type: 'tokens', value: tokens.join(', '), tokens };
  }

  function emptyProperty(key: string, label: string, value: string): FileCardProperty {
    return { key, label, type: 'empty', value, tokens: [] };
  }

  function parseTagValue(value: string) {
    return parseYamlArrayValue(value)
      .flatMap((item) => item.split(/[\s,]+/))
      .map((item) => item.replace(/^#/, '').trim())
      .filter(Boolean);
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
          .map((item) => unquoteScalar(item.trim()))
          .filter(Boolean);
      }
    }

    return raw
      .split(',')
      .map((item) => unquoteScalar(item.trim()))
      .filter(Boolean);
  }

  function parseListValue(value: string) {
    const trimmed = value.trim();
    if (!trimmed) return [];
    if (trimmed.startsWith('[') && trimmed.endsWith(']')) {
      return trimmed
        .slice(1, -1)
        .split(',')
        .map(unquoteScalar)
        .filter(Boolean);
    }
    return [unquoteScalar(trimmed)];
  }

  function unquoteScalar(value: string) {
    const trimmed = value.trim();
    if (
      (trimmed.startsWith('"') && trimmed.endsWith('"'))
      || (trimmed.startsWith("'") && trimmed.endsWith("'"))
    ) {
      return trimmed.slice(1, -1);
    }
    return trimmed;
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
    return type === 'tags' ? trimmed.replace(/^#+/, '').trim() : trimmed;
  }

  function propertyTypeFor(property: PropertyRow): PropertyType {
    const normalizedKey = property.key.trim();
    const override = editingPropertyTypeOverrides[normalizedKey];
    if (override) return override;
    if (normalizedKey === 'tags') return 'tags';
    if (normalizedKey === 'aliases') return 'list';

    const value = unquoteScalar(property.value.trim());
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
    return unquoteScalar(value).match(/^(\d{4}-\d{2}-\d{2})/)?.[1] ?? '';
  }

  function datetimeInputValue(value: string) {
    const match = unquoteScalar(value).match(/^(\d{4}-\d{2}-\d{2})[T\s](\d{2}:\d{2})/);
    return match ? `${match[1]}T${match[2]}` : '';
  }

  function checkboxInputValue(value: string) {
    return /^true$/i.test(unquoteScalar(value));
  }

  function todayDateValue() {
    return new Date().toISOString().slice(0, 10);
  }

  function valueForPropertyType(type: PropertyType, currentValue: string) {
    const text = unquoteScalar(currentValue.trim());
    const tokens = parseYamlArrayValue(currentValue);

    if (type === 'checkbox') return /^(true|yes|1|checked)$/i.test(text) ? 'true' : 'false';
    if (type === 'date') return dateInputValue(currentValue) || todayDateValue();
    if (type === 'datetime') {
      const existing = datetimeInputValue(currentValue);
      return existing ? `${existing}:00Z` : new Date().toISOString().replace(/\.\d{3}Z$/, 'Z');
    }
    if (type === 'number') return /^-?\d+(?:\.\d+)?$/.test(text) ? text : '0';
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

  function sourceValueFromTokens(tokens: string[]) {
    return `[${tokens.map(quoteYamlString).join(', ')}]`;
  }

  function sourceTagValueFromTokens(tokens: string[]) {
    const deduped = Array.from(new Set(tokens.map((token) => normalizeTokenValue('tags', token)).filter(Boolean)));
    return deduped.length ? deduped.map((token) => `#${token}`).join(', ') : '[]';
  }

  function propertyTokens(property: PropertyRow) {
    return propertyTypeFor(property) === 'tags' ? parseTagValue(property.value) : parseYamlArrayValue(property.value);
  }

  function propertyDisplayValue(key: string, value: string): PropertyDisplay {
    const raw = value.trim();
    const unquoted = unquoteScalar(raw);
    const timestamp = unquoted.match(/^(\d{4})-(\d{2})-(\d{2})[T\s](\d{2}):(\d{2})(?::\d{2}(?:\.\d+)?)?(?:Z|[+-]\d{2}:?\d{2})?$/);
    if (timestamp) return { text: formatDateTimeParts(timestamp[4], timestamp[5], timestamp[1], timestamp[2], timestamp[3]), formatted: true };

    const date = unquoted.match(/^(\d{4})-(\d{2})-(\d{2})$/);
    if (date) return { text: formatDateParts(date[1], date[2], date[3]), formatted: true };

    if (key.trim() === 'tags') {
      const tags = parseTagValue(raw);
      return { text: tags.length ? tags.join(', ') : 'None', formatted: true };
    }

    if (key.trim() === 'aliases') {
      const aliases = parseYamlArrayValue(raw);
      return { text: aliases.length ? aliases.join(', ') : 'None', formatted: true };
    }

    return { text: unquoted, formatted: raw !== unquoted };
  }

  function formatDateParts(year: string, month: string, day: string) {
    return `${day}/${month}/${year}`;
  }

  function formatDateTimeParts(hour: string, minute: string, year: string, month: string, day: string) {
    return `${formatDateParts(year, month, day)} ${hour}:${minute}`;
  }

  function sourceValueFromLiveInput(key: string, currentSourceValue: string, inputValue: string) {
    const normalizedKey = key.trim();
    const trimmed = inputValue.trim();
    const current = currentSourceValue.trim();
    const wasQuoted = current.startsWith('"') && current.endsWith('"');

    if (normalizedKey === 'tags') {
      if (!trimmed || trimmed.toLowerCase() === 'none' || trimmed === '[]') return '[]';
      return sourceTagValueFromTokens(trimmed.split(/[\s,]+/));
    }

    if (normalizedKey === 'aliases') {
      if (!trimmed || trimmed.toLowerCase() === 'none' || trimmed === '[]') return '[]';
      if (trimmed.startsWith('[')) return trimmed;
      const values = trimmed.split(',').map((item) => item.trim()).filter(Boolean);
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

  function bodyPreviewLines(body: string) {
    const lines: string[] = [];
    let inFence = false;

    for (const rawLine of body.split(/\r?\n/)) {
      if (/^\s*(```|~~~)/.test(rawLine)) {
        inFence = !inFence;
        continue;
      }
      if (inFence) continue;

      const line = previewLineText(rawLine);
      if (line.trim() || lines.length) lines.push(line);
      if (lines.length >= 24) break;
    }

    return lines;
  }

  function previewLineText(line: string) {
    return line
      .replace(/!\[\[([^\]]+)\]\]/g, '$1')
      .replace(/\[\[!?([^\]|]+)(?:\|([^\]]+))?\]\]/g, (_match, target: string, label?: string) => label || target)
      .replace(/\[([^\]]+)\]\([^)]+\)/g, '$1')
      .replace(/^(\s*)#{1,6}\s+/, '$1')
      .replace(/^(\s*)[-*+]\s+/, '$1- ')
      .replace(/[*_`]/g, '')
      .replace(/\t/g, '    ')
      .replace(/[ \t]+$/g, '');
  }

  function renderMarkdownPlusBody(body: string, _sourceSignature = '', _resolutionSignature = '') {
    const html = marked.parse(markdownPlusPreviewSource(body), {
      async: false,
      breaks: true
    }) as string;

    return DOMPurify.sanitize(html, {
      ADD_TAGS: ['iframe'],
      ADD_ATTR: [
        'allow',
        'allowfullscreen',
        'data-mdp-external-link',
        'data-mdp-internal-link',
        'frameborder',
        'loading',
        'referrerpolicy',
        'src',
        'target'
      ]
    });
  }

  function markdownPlusPreviewSource(input: string) {
    return preserveMarkdownBlankLines(renderMarkdownPlusBlocks(renderInlineTags(renderWikiLinks(renderExternalEmbeds(input)))));
  }

  function renderMarkdownPlusBlocks(input: string) {
    const lines = input.split(/\r?\n/);
    let inFence = false;

    return lines
      .map((line) => {
        if (/^\s*(```|~~~)/.test(line)) {
          inFence = !inFence;
          return line;
        }
        if (inFence) return line;

        const heading = line.match(/^([ \t]*)(#{1,6})[ \t]+(.+)$/);
        if (heading) {
          const level = heading[2].length;
          return `${heading[1]}<h${level}>${heading[3].trim()}</h${level}>`;
        }

        if (/^[ \t]*-{3,}[ \t]*$/.test(line)) {
          return '<hr data-mdp-rule="underline">';
        }

        return line;
      })
      .join('\n');
  }

  function preserveMarkdownBlankLines(input: string) {
    const lines = input.split(/\r?\n/);
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

  function renderWikiLinks(input: string) {
    return input.replace(/\[\[([^\]\n]+)\]\]/g, (_match, rawLink: string) => {
      const [rawTarget, rawLabel] = rawLink.split('|');
      const isEmbed = rawTarget.trim().startsWith('!');
      const target = isEmbed ? rawTarget.trim().slice(1).trim() : rawTarget.trim();
      const label = (rawLabel ?? target).replace(/^!/, '').trim();
      if (!target) return _match;

      if (isEmbed) {
        return renderInternalEmbed(target, label || target);
      }

      return `<a href="#${encodeURIComponent(target)}" class="mdp-internal-link" data-mdp-internal-link="${escapeHtml(target)}">${escapeHtml(label)}</a>`;
    });
  }

  function renderInternalEmbed(target: string, label: string) {
    const noteId = resolveInternalLink(target);
    if (!noteId) {
      return `\n<div class="mdp-embed mdp-internal-embed mdp-missing-embed"><span class="mdp-embed-title">${escapeHtml(label || target)}</span><p>Document not found.</p></div>\n`;
    }

    const note = notes.find((candidate) => candidate.id === noteId);
    const source = documentSources[noteId] || loadedDocumentSources[noteId] || canvasDocumentSourceCache[noteId];
    const title = escapeHtml(note?.title ?? (label || target));
    const kind = note?.note_type ?? 'note';
    const excerpt = source ? embedExcerptForSource(kind, source) : 'Loading preview...';

    return `\n<div class="mdp-embed mdp-internal-embed mdp-${kind}-embed"><a href="#${encodeURIComponent(target)}" class="mdp-embed-title" data-mdp-internal-link="${escapeHtml(target)}">${title}</a><p>${escapeHtml(excerpt)}</p></div>\n`;
  }

  function embedExcerptForSource(kind: string, source: string) {
    if (kind === 'base') return 'Base view configuration';
    if (kind === 'canvas') {
      try {
        const parsed = JSON.parse(source) as { nodes?: unknown[]; edges?: unknown[] };
        return `Canvas · ${Array.isArray(parsed.nodes) ? parsed.nodes.length : 0} nodes · ${Array.isArray(parsed.edges) ? parsed.edges.length : 0} edges`;
      } catch {
        return 'Canvas document';
      }
    }

    const body = splitMarkdownPlusSource(source).body;
    return bodyPreviewLines(body).join(' ').replace(/\s+/g, ' ').trim() || 'Empty note';
  }

  function renderExternalEmbeds(input: string) {
    return input.replace(/\[!([^\]\n]+)\]\(([^)\n]+)\)/g, (_match, rawLabel: string, rawUrl: string) => {
      const preview = externalEmbedForTarget(rawLabel, rawUrl);
      if (!preview.url) return _match;
      const title = escapeHtml(preview.label || preview.url);
      const url = escapeHtml(preview.url);
      return `\n<div class="mdp-embed mdp-external-embed"><a class="mdp-embed-title" href="${url}" data-mdp-external-link="${url}">${title}</a><p>${url}</p></div>\n`;
    });
  }

  function renderInlineTags(input: string) {
    return input.replace(/(^|[\s([{])#([A-Za-z0-9_/-]+)/g, (_match, prefix: string, tag: string) =>
      `${prefix}<span class="mdp-inline-tag">${escapeHtml(tag)}</span>`
    );
  }

  function escapeHtml(value: string) {
    return value
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;');
  }

  function startInlineFileEdit(node: CanvasNode) {
    const target = fileNodeTarget(node);
    if (!target || target.note_type !== 'note') {
      editNode(node);
      return;
    }

    const source = documentSources[target.id] || loadedDocumentSources[target.id] || canvasDocumentSourceCache[target.id];
    if (!source) {
      void cachedCanvasDocumentSource(target.id).then((loaded) => {
        loadedDocumentSources = { ...loadedDocumentSources, [target.id]: loaded };
        editingFileNodeId = node.id;
        editingFileSource = loaded;
        syncEditingFieldsFromSource(loaded);
      });
      return;
    }

    editingFileNodeId = node.id;
    editingFileSource = source;
    syncEditingFieldsFromSource(source);
  }

  function syncEditingFieldsFromSource(source: string) {
    const split = splitMarkdownPlusSource(source);
    editingFileProperties = propertyRowsFromFrontmatter(split.frontmatter);
    editingFileBody = split.body;
    editingTokenPropertyDrafts = {};
    editingPropertyTypeMenuIndex = null;
  }

  function updateEditingSourceFromFields() {
    editingFileSource = sourceFromEditableFields(editingFileProperties, editingFileBody);
    scheduleInlineFileSave();
  }

  function updateEditingFileBody(value: string) {
    editingFileBody = value;
    updateEditingSourceFromFields();
  }

  function updateEditingProperty(index: number, field: keyof PropertyRow, value: string) {
    editingFileProperties = editingFileProperties.map((property, propertyIndex) =>
      propertyIndex === index ? { ...property, [field]: value } : property
    );
    updateEditingSourceFromFields();
  }

  function setEditingPropertyType(index: number, type: PropertyType) {
    const property = editingFileProperties[index];
    if (!property) return;

    editingPropertyTypeOverrides = {
      ...editingPropertyTypeOverrides,
      [property.key.trim()]: type
    };
    editingTokenPropertyDrafts = {
      ...editingTokenPropertyDrafts,
      [index]: ''
    };
    editingPropertyTypeMenuIndex = null;
    editingFileProperties = editingFileProperties.map((row, propertyIndex) =>
      propertyIndex === index ? { ...row, value: valueForPropertyType(type, row.value) } : row
    );
    updateEditingSourceFromFields();
  }

  function updateEditingPropertyValueFromLiveInput(index: number, value: string) {
    editingFileProperties = editingFileProperties.map((property, propertyIndex) =>
      propertyIndex === index
        ? { ...property, value: sourceValueFromLiveInput(property.key, property.value, value) }
        : property
    );
    updateEditingSourceFromFields();
  }

  function updateEditingTokenPropertyDraft(index: number, value: string) {
    editingTokenPropertyDrafts = {
      ...editingTokenPropertyDrafts,
      [index]: value
    };
  }

  function updateEditingTokenProperty(index: number, tokens: string[]) {
    const property = editingFileProperties[index];
    const type = property ? propertyTypeFor(property) : 'list';
    const deduped = Array.from(new Set(tokens.map((token) => normalizeTokenValue(type, token)).filter(Boolean)));
    editingFileProperties = editingFileProperties.map((property, propertyIndex) =>
      propertyIndex === index
        ? { ...property, value: type === 'tags' ? sourceTagValueFromTokens(deduped) : sourceValueFromTokens(deduped) }
        : property
    );
    updateEditingSourceFromFields();
  }

  function commitEditingTokenPropertyDraft(index: number, type: PropertyType) {
    const property = editingFileProperties[index];
    if (!property) return;
    const tokensToAdd = (editingTokenPropertyDrafts[index] ?? '')
      .split(',')
      .map((token) => normalizeTokenValue(type, token))
      .filter(Boolean);
    if (!tokensToAdd.length) return;

    updateEditingTokenProperty(index, [...propertyTokens(property), ...tokensToAdd]);
    editingTokenPropertyDrafts = {
      ...editingTokenPropertyDrafts,
      [index]: ''
    };
  }

  function removeEditingTokenPropertyItem(index: number, token: string) {
    const property = editingFileProperties[index];
    if (!property) return;
    updateEditingTokenProperty(index, propertyTokens(property).filter((item) => item !== token));
  }

  function handleEditingTokenPropertyKeydown(event: KeyboardEvent, index: number, type: PropertyType) {
    if (event.key === 'Enter' || event.key === ',') {
      event.preventDefault();
      commitEditingTokenPropertyDraft(index, type);
      return;
    }

    if (event.key === 'Backspace' && !(editingTokenPropertyDrafts[index] ?? '')) {
      const property = editingFileProperties[index];
      const tokens = property ? propertyTokens(property) : [];
      if (tokens.length) {
        event.preventDefault();
        updateEditingTokenProperty(index, tokens.slice(0, -1));
      }
    }
  }

  function addEditingProperty() {
    editingPropertyTypeMenuIndex = null;
    editingFileProperties = [...editingFileProperties, { key: 'property', value: 'value' }];
    updateEditingSourceFromFields();
  }

  function removeEditingProperty(index: number) {
    editingPropertyTypeMenuIndex = null;
    editingFileProperties = editingFileProperties.filter((_, propertyIndex) => propertyIndex !== index);
    updateEditingSourceFromFields();
  }

  function scheduleInlineFileSave() {
    if (editingFileSaveTimer) clearTimeout(editingFileSaveTimer);
    editingFileSaveTimer = setTimeout(() => {
      editingFileSaveTimer = null;
      void saveInlineFileEdit();
    }, 700);
  }

  async function saveInlineFileEdit() {
    if (!editingFileNodeId) return;
    const node = nodeById.get(editingFileNodeId);
    if (!node) return;
    const target = fileNodeTarget(node);
    if (!target || target.note_type !== 'note') return;

    savingFileNodeId = node.id;
    try {
      const updatedNote = await saveCachedCanvasDocumentSource(target.id, editingFileSource);
      loadedDocumentSources = {
        ...loadedDocumentSources,
        [target.id]: editingFileSource
      };
      onNotesChanged(upsertNoteSummary(notes, updatedNote));
      setStatus('Saved embedded note.');
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      savingFileNodeId = null;
    }
  }

  function upsertNoteSummary(items: NoteSummary[], note: NoteSummary): NoteSummary[] {
    const index = items.findIndex((item) => item.id === note.id);
    if (index === -1) return [...items, note].sort((left, right) => left.title.localeCompare(right.title));
    return items.map((item, itemIndex) => (itemIndex === index ? note : item));
  }

  async function finishInlineFileEdit() {
    if (editingFileSaveTimer) {
      clearTimeout(editingFileSaveTimer);
      editingFileSaveTimer = null;
    }
    await saveInlineFileEdit();
    editingFileNodeId = null;
  }

  function fileName(path: string) {
    return path.split(/[\\/]/).pop() ?? path;
  }

  function fileNodeTitle(node: CanvasNode) {
    return pathStem(node.file ?? '') || node.file || 'Untitled file';
  }

  function pathStem(path: string) {
    const filename = path.split(/[\\/]/).pop() ?? path;
    return filename.replace(/\.[^.]+$/, '');
  }

  function normalizeStem(value: string) {
    return value.trim().toLowerCase().replace(/\s+/g, ' ');
  }

  function openFileNode(node: CanvasNode) {
    const target = fileNodeTarget(node);
    if (!target) {
      setStatus(`Canvas file target not found: ${node.file ?? 'Untitled file'}.`);
      return;
    }

    onOpenNote(target.id);
  }

  function normalizeExternalUrl(url: string) {
    const trimmed = url.trim();
    if (!trimmed) return '';
    if (/^[a-z][a-z0-9+.-]*:\/\//i.test(trimmed)) return trimmed;
    return `https://${trimmed}`;
  }

  function openLinkNode(node: CanvasNode) {
    const url = normalizeExternalUrl(node.url ?? '');
    if (!url) return;
    window.open(url, '_blank', 'noopener,noreferrer');
  }

  function handleWheel(event: WheelEvent) {
    if (!event.ctrlKey && !event.metaKey) {
      panX -= event.deltaX;
      panY -= event.deltaY;
      return;
    }

    event.preventDefault();
    const nextScale = Math.min(2.5, Math.max(0.25, scale - event.deltaY * 0.001));
    scale = Number(nextScale.toFixed(2));
  }

  function startPan(event: PointerEvent) {
    if (event.button !== 0) return;
    if (!(event.target instanceof HTMLElement) || event.target.closest('.canvas-node')) return;

    dragStart = {
      pointerId: event.pointerId,
      x: event.clientX,
      y: event.clientY,
      panX,
      panY
    };
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  }

  function movePan(event: PointerEvent) {
    if (moveConnectionDraft(event)) return;
    if (moveNodeResize(event)) return;
    if (moveNodeDrag(event)) return;
    if (!dragStart || dragStart.pointerId !== event.pointerId) return;
    panX = dragStart.panX + event.clientX - dragStart.x;
    panY = dragStart.panY + event.clientY - dragStart.y;
  }

  function endPan(event: PointerEvent) {
    if (endConnectionDraft(event)) return;
    endNodeDrag(event);
    if (dragStart?.pointerId === event.pointerId) dragStart = null;
  }

  function resetView() {
    scale = 1;
    panX = -canvasStableOrigin;
    panY = -canvasStableOrigin;
  }

  function openCanvasContextMenu(event: MouseEvent, node?: CanvasNode) {
    event.preventDefault();
    event.stopPropagation();

    const position = pointerWorldPosition(event);
    const menuWidth = node ? 190 : 210;
    const menuHeight = node ? 192 : 224;
    contextMenu = {
      x: Math.min(event.clientX, Math.max(0, window.innerWidth - menuWidth - 8)),
      y: Math.min(event.clientY, Math.max(0, window.innerHeight - menuHeight - 8)),
      worldX: position.x,
      worldY: position.y,
      nodeId: node?.id
    };
  }

  function pointerWorldPosition(event: MouseEvent | PointerEvent) {
    const viewport = event.currentTarget instanceof HTMLElement
      ? event.currentTarget.closest('.canvas-viewport') ?? event.currentTarget
      : null;
    const rect = viewport?.getBoundingClientRect() ?? { left: 0, top: 0 };
    return {
      x: snapToGrid((event.clientX - rect.left - panX) / scale + bounds.minX),
      y: snapToGrid((event.clientY - rect.top - panY) / scale + bounds.minY)
    };
  }

  function closeContextMenu() {
    contextMenu = null;
  }

  function contextMenuNode() {
    return contextMenu?.nodeId ? nodeById.get(contextMenu.nodeId) ?? null : null;
  }

  function runCanvasMenuAction(action: (position: { x: number; y: number }) => void) {
    const menu = contextMenu;
    closeContextMenu();
    if (!menu) return;
    action({ x: menu.worldX, y: menu.worldY });
  }

  function openContextMenuNode() {
    const node = contextMenuNode();
    closeContextMenu();
    if (!node) return;
    if (node.type === 'file') openFileNode(node);
    if (node.type === 'link') openLinkNode(node);
  }

  function editContextMenuNode() {
    const node = contextMenuNode();
    closeContextMenu();
    if (node) editNode(node);
  }

  function duplicateContextMenuNode() {
    const node = contextMenuNode();
    closeContextMenu();
    if (node) duplicateNode(node);
  }

  function deleteContextMenuNode() {
    const node = contextMenuNode();
    closeContextMenu();
    if (node) deleteNode(node.id);
  }

  function handleWindowPointerDown(event: PointerEvent) {
    if (!contextMenu) return;
    if (event.target instanceof Node && contextMenuElement?.contains(event.target)) return;
    closeContextMenu();
  }

  function handleWindowKeydown(event: KeyboardEvent) {
    if (event.key !== 'Escape') return;
    if (editingFileNodeId) {
      event.preventDefault();
      void finishInlineFileEdit();
      return;
    }
    if (draftDialog) {
      closeDraft();
      return;
    }
    closeContextMenu();
  }
</script>

<svelte:window on:pointerdown={handleWindowPointerDown} on:keydown={handleWindowKeydown} />

<section class="canvas-view">
  <header class="canvas-header">
    <div>
      <h2>Canvas</h2>
      <p>{canvas.nodes.length} nodes x {canvas.edges.length} edges</p>
    </div>
    <div class="canvas-actions">
      <button type="button" on:click={addTextNode}><Type size={14} /> Text</button>
      <button type="button" on:click={addFileNode}><FileText size={14} /> File</button>
      <button type="button" on:click={addGroupNode}><Plus size={14} /> Group</button>
      <button type="button" on:click={resetView}>Reset</button>
    </div>
  </header>

  {#if canvas.error}
    <div class="canvas-error">Invalid canvas JSON: {canvas.error}</div>
  {:else}
    <div
      class:dragging={Boolean(dragStart)}
      class="canvas-viewport"
      role="application"
      aria-label="Canvas"
      on:pointerdown={startPan}
      on:pointermove={movePan}
      on:pointerup={endPan}
      on:pointercancel={endPan}
      on:contextmenu={openCanvasContextMenu}
      on:wheel={handleWheel}
    >
      <div
        class="canvas-stage"
        style={`width: ${bounds.width}px; height: ${bounds.height}px; transform: translate(${panX}px, ${panY}px) scale(${scale});`}
      >
        <svg class="canvas-edges" width={bounds.width} height={bounds.height} viewBox={`0 0 ${bounds.width} ${bounds.height}`}>
          <defs>
            <marker id="canvas-arrow" markerWidth="10" markerHeight="10" refX="8" refY="5" orient="auto" markerUnits="strokeWidth">
              <path d="M 0 0 L 10 5 L 0 10 z" />
            </marker>
          </defs>
          {#each canvas.edges as edge (edge.id)}
            {@const line = edgeLine(edge)}
            {#if line}
              <line
                x1={line.x1}
                y1={line.y1}
                x2={line.x2}
                y2={line.y2}
                stroke={line.color}
                marker-start={edge.fromEnd === 'arrow' ? 'url(#canvas-arrow)' : undefined}
                marker-end={edge.toEnd === 'none' ? undefined : 'url(#canvas-arrow)'}
              />
              {#if edge.label}
                <text x={line.midX} y={line.midY - 8} fill={line.color}>{edge.label}</text>
              {/if}
            {/if}
          {/each}
          {#if connectionDraft}
            <line
              class="connection-draft-line"
              x1={connectionDraft.x1}
              y1={connectionDraft.y1}
              x2={connectionDraft.x2}
              y2={connectionDraft.y2}
              stroke="#8bd5bd"
              marker-end="url(#canvas-arrow)"
            />
          {/if}
        </svg>

        {#if !canvas.nodes.length}
          <div class="canvas-empty">Empty canvas</div>
        {/if}

        {#each renderedCanvasNodes as node (node.id)}
          {@const Icon = canvasIcon(node.type)}
          <article
            class:canvas-group-node={node.type === 'group'}
            class="canvas-node"
            role="group"
            style={nodeStyle(node)}
            on:pointerdown={(event) => startNodeDrag(event, node)}
            on:dblclick={() => node.type === 'file' ? startInlineFileEdit(node) : editNode(node)}
            on:contextmenu={(event) => openCanvasContextMenu(event, node)}
          >
            <div class="canvas-node-header">
              <Icon size={15} />
              {#if node.type === 'file'}
                <button type="button" on:click={() => openFileNode(node)}>{fileNodeTitle(node)}</button>
              {:else if node.type === 'link'}
                <button type="button" on:click={() => openLinkNode(node)}>{node.url ?? 'Link'}</button>
                <ExternalLink size={13} />
              {:else if node.type === 'group'}
                <span>{node.label ?? 'Group'}</span>
              {:else}
                <span>Text</span>
              {/if}
              <button class="canvas-node-delete" type="button" aria-label="Delete card" on:click={() => deleteNode(node.id)}>
                <Trash2 size={13} />
              </button>
            </div>

            {#if node.type === 'text'}
              <div class="canvas-text-body">
                {@html renderMarkdownPlusBody(node.text ?? '', canvasFileSourceSignature, canvasDocumentResolutionSignature)}
              </div>
            {:else if node.type === 'file'}
              {@const preview = fileNodePreview(node, canvasFileSourceSignature, canvasDocumentResolutionSignature)}
              <div
                class:editing-file-preview={editingFileNodeId === node.id}
                class:missing-preview={preview.status === 'missing'}
                class="file-card-preview"
              >
                {#if editingFileNodeId === node.id}
                  <section class="embedded-properties" aria-label={`${preview.title} properties`} on:pointerdown|stopPropagation>
                    <div class="embedded-property-list">
                      {#each editingFileProperties as property, index}
                        {@const propertyType = propertyTypeFor(property)}
                        {@const displayValue = propertyDisplayValue(property.key, property.value)}
                        <div class="embedded-property-row">
                          <div class="embedded-property-type-cell">
                            <button
                              class:active={editingPropertyTypeMenuIndex === index}
                              class="embedded-property-type-button"
                              type="button"
                              aria-label={`Change ${propertyLabel(property.key)} property type`}
                              title={propertyTypeLabel(propertyType)}
                              on:click={() => (editingPropertyTypeMenuIndex = editingPropertyTypeMenuIndex === index ? null : index)}
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
                            {#if editingPropertyTypeMenuIndex === index}
                              <div class="embedded-property-type-menu" role="menu" aria-label="Property type">
                                {#each propertyTypeOptions as option}
                                  <button
                                    class:active={propertyType === option.id}
                                    type="button"
                                    role="menuitemradio"
                                    aria-checked={propertyType === option.id}
                                    on:click={() => setEditingPropertyType(index, option.id)}
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
                            <span class="embedded-property-label" title={property.key}>{propertyLabel(property.key)}</span>
                          {:else}
                            <input
                              class="embedded-property-name-input"
                              aria-label="Property name"
                              value={property.key}
                              on:input={(event) => updateEditingProperty(index, 'key', event.currentTarget.value)}
                            />
                          {/if}
                          {#if isTokenProperty(property)}
                            {@const tokens = propertyTokens(property)}
                            <div class="embedded-token-property-field" title={property.value}>
                              {#each tokens as token}
                                <span class="embedded-property-token">
                                  {token}
                                  <button
                                    type="button"
                                    aria-label={`Remove ${token}`}
                                    on:click={() => removeEditingTokenPropertyItem(index, token)}
                                  >
                                    <X size={11} />
                                  </button>
                                </span>
                              {/each}
                              <input
                                aria-label={`Add ${propertyLabel(property.key)}`}
                                class="embedded-token-property-input"
                                placeholder={tokens.length ? '' : `Add ${propertyLabel(property.key).toLowerCase()}`}
                                value={editingTokenPropertyDrafts[index] ?? ''}
                                on:input={(event) => updateEditingTokenPropertyDraft(index, event.currentTarget.value)}
                                on:keydown={(event) => handleEditingTokenPropertyKeydown(event, index, propertyType)}
                                on:blur={() => commitEditingTokenPropertyDraft(index, propertyType)}
                              />
                            </div>
                          {:else if propertyType === 'checkbox'}
                            <label class="embedded-checkbox-property-field">
                              <input
                                type="checkbox"
                                checked={checkboxInputValue(property.value)}
                                on:change={(event) => updateEditingProperty(index, 'value', event.currentTarget.checked ? 'true' : 'false')}
                              />
                              <span>{checkboxInputValue(property.value) ? 'Checked' : 'Unchecked'}</span>
                            </label>
                          {:else if propertyType === 'date'}
                            <input
                              aria-label="Property date"
                              class="embedded-formatted-property-input"
                              type="date"
                              value={dateInputValue(property.value)}
                              on:input={(event) => updateEditingProperty(index, 'value', event.currentTarget.value)}
                            />
                          {:else if propertyType === 'datetime'}
                            <input
                              aria-label="Property date and time"
                              class="embedded-formatted-property-input"
                              type="datetime-local"
                              value={datetimeInputValue(property.value)}
                              on:input={(event) => updateEditingProperty(index, 'value', event.currentTarget.value ? `${event.currentTarget.value}:00Z` : '')}
                            />
                          {:else if propertyType === 'number'}
                            <input
                              aria-label="Property number"
                              type="number"
                              value={unquoteScalar(property.value)}
                              on:input={(event) => updateEditingProperty(index, 'value', event.currentTarget.value)}
                            />
                          {:else}
                            <input
                              aria-label="Property value"
                              class:embedded-formatted-property-input={displayValue.formatted}
                              title={displayValue.formatted ? property.value : undefined}
                              value={displayValue.text}
                              on:input={(event) => updateEditingPropertyValueFromLiveInput(index, event.currentTarget.value)}
                            />
                          {/if}
                          <button class="embedded-property-remove-button" aria-label="Remove property" on:click={() => removeEditingProperty(index)}>
                            <X size={13} />
                          </button>
                        </div>
                      {/each}
                    </div>
                    <button class="embedded-add-property-button" on:click={addEditingProperty}>Add property</button>
                  </section>
                  <div class="embedded-note-editor" role="presentation" on:pointerdown|stopPropagation>
                    <MarkdownPlusEditor
                      value={editingFileBody}
                      ariaLabel={`Edit ${preview.title}`}
                      {internalLinkExists}
                      {internalLinkCompletions}
                      internalLinkSignature={canvasDocumentResolutionSignature}
                      embedSignature={canvasFileSourceSignature}
                      {externalEmbedForTarget}
                      onChange={updateEditingFileBody}
                      onInternalLink={(target) => openInternalLinkTarget(target)}
                      onExternalLink={(target) => {
                        const url = normalizeExternalUrl(target);
                        if (url) window.open(url, '_blank', 'noopener,noreferrer');
                      }}
                    />
                  </div>
                  {#if savingFileNodeId === node.id}<span class="embedded-save-state">Saving...</span>{/if}
                {:else}
                  {#if preview.properties.length}
                    <dl>
                      {#each preview.properties as property (property.key)}
                        <div>
                          <dt>{property.label}</dt>
                          <dd class:empty-property={property.type === 'empty'}>
                            {#if property.type === 'tokens'}
                              <span class="property-token-list">
                                {#each property.tokens as token}
                                  <span>{token}</span>
                                {/each}
                              </span>
                            {:else}
                              {property.value}
                            {/if}
                          </dd>
                        </div>
                      {/each}
                    </dl>
                  {/if}
                  {#if preview.bodyHtml}
                    <div class="embedded-note-body">
                      {@html preview.bodyHtml}
                    </div>
                  {:else if preview.lines.length}
                    <div class="file-card-lines">
                      {#each preview.lines as line, index (`${index}:${line}`)}
                        <div class:blank-preview-line={!line.trim()}>{line}</div>
                      {/each}
                    </div>
                  {/if}
                {/if}
              </div>
            {:else if node.type === 'link'}
              <p>{normalizeExternalUrl(node.url ?? '')}</p>
            {:else if node.background}
              <p>{node.background}</p>
            {/if}

            {#each ['nw', 'ne', 'sw', 'se'] as corner}
              <button
                class={`resize-handle resize-${corner}`}
                type="button"
                aria-label={`Resize ${corner}`}
                on:pointerdown|stopPropagation={(event) => startNodeResize(event, node, corner as ResizeCorner)}
              ></button>
            {/each}
            {#each ['top', 'right', 'bottom', 'left'] as side}
              <button
                class={`connection-handle connection-${side}`}
                type="button"
                aria-label={`Connect from ${side}`}
                data-node-id={node.id}
                data-side={side}
                on:pointerdown={(event) => startConnection(event, node, side as CanvasSide)}
              ></button>
            {/each}
          </article>
        {/each}
      </div>
    </div>
  {/if}

  {#if contextMenu}
    {@const activeNode = contextMenuNode()}
    <div
      bind:this={contextMenuElement}
      class="canvas-context-menu"
      role="menu"
      aria-label="Canvas actions"
      style={`left: ${contextMenu.x}px; top: ${contextMenu.y}px;`}
    >
      {#if activeNode}
        {#if activeNode.type === 'file' || activeNode.type === 'link'}
          <button type="button" role="menuitem" on:click={openContextMenuNode}>Open</button>
        {/if}
        {#if activeNode.type !== 'link'}
          <button type="button" role="menuitem" on:click={editContextMenuNode}>Edit card</button>
        {/if}
        <button type="button" role="menuitem" on:click={duplicateContextMenuNode}>Duplicate card</button>
        <div class="context-menu-separator"></div>
        <button class="danger" type="button" role="menuitem" on:click={deleteContextMenuNode}>Delete card</button>
      {:else}
        <button type="button" role="menuitem" on:click={() => runCanvasMenuAction(insertTextNodeAt)}>Insert text card</button>
        <button type="button" role="menuitem" on:click={() => runCanvasMenuAction(insertDocumentNodeAt)}>Insert note card</button>
        <button type="button" role="menuitem" on:click={() => runCanvasMenuAction(insertGroupNodeAt)}>Insert group</button>
        <div class="context-menu-separator"></div>
        <button type="button" role="menuitem" on:click={() => { closeContextMenu(); resetView(); }}>Reset view</button>
      {/if}
    </div>
  {/if}

  {#if draftDialog}
    <div class="canvas-dialog-backdrop">
      <div
        class="canvas-dialog"
        role="dialog"
        aria-modal="true"
        aria-label={draftTitle(draftDialog)}
      >
        <form class="canvas-dialog-form" on:submit|preventDefault={() => submitDraft()}>
          <header>
            <h3>{draftTitle(draftDialog)}</h3>
            <button class="dialog-close" type="button" on:click={closeDraft}>Close</button>
          </header>

          {#if draftDialog.kind === 'text'}
            <div class="canvas-dialog-editor" role="presentation">
              <MarkdownPlusEditor
                value={draftDialog.value}
                ariaLabel={draftPlaceholder(draftDialog)}
                {internalLinkExists}
                {internalLinkCompletions}
                internalLinkSignature={canvasDocumentResolutionSignature}
                embedSignature={canvasFileSourceSignature}
                {externalEmbedForTarget}
                onChange={updateDraftValue}
                onInternalLink={(target) => openInternalLinkTarget(target)}
                onExternalLink={(target) => {
                  const url = normalizeExternalUrl(target);
                  if (url) window.open(url, '_blank', 'noopener,noreferrer');
                }}
              />
            </div>
          {:else}
            <input
              bind:this={draftInputElement}
              value={draftDialog.value}
              placeholder={draftPlaceholder(draftDialog)}
              on:input={(event) => updateDraftValue(event.currentTarget.value)}
            />
          {/if}

          {#if draftDialog.kind === 'file'}
            <div class="document-results">
              {#each documentMatches as document (document.id)}
                <button type="button" on:click={() => submitDraft(document)}>
                  <span>{document.title}</span>
                  <small>{documentTypeLabel(document)} · {fileName(document.path)}</small>
                </button>
              {:else}
                <div class="document-empty">No matching workspace documents</div>
              {/each}
            </div>
          {/if}

          <footer>
            <button type="button" on:click={closeDraft}>Cancel</button>
            <button class="primary" type="submit" disabled={!draftCanSubmit(draftDialog)}>
              {draftDialog.mode === 'edit' ? 'Update' : 'Insert'}
            </button>
          </footer>
        </form>
      </div>
    </div>
  {/if}
</section>

<style>
  .canvas-view {
    display: grid;
    grid-template-rows: auto 1fr;
    min-height: 0;
    height: 100%;
    background: #080d13;
  }

  .canvas-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 10px 14px;
    border-bottom: 1px solid #1d2936;
    background: #0b1017;
  }

  .canvas-header h2 {
    margin: 0;
    color: #8fe8cf;
    font-size: 16px;
  }

  .canvas-header p {
    margin: 2px 0 0;
    color: #7f8a98;
    font-size: 12px;
  }

  .canvas-header button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border: 1px solid #2a3848;
    border-radius: 6px;
    background: #121923;
    color: #d5dde8;
    padding: 6px 10px;
  }

  .canvas-actions {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    flex-wrap: wrap;
    gap: 8px;
  }

  .canvas-viewport {
    position: relative;
    min-height: 0;
    overflow: hidden;
    cursor: grab;
    background:
      linear-gradient(#101721 1px, transparent 1px),
      linear-gradient(90deg, #101721 1px, transparent 1px),
      #080d13;
    background-size: 32px 32px;
  }

  .canvas-viewport.dragging {
    cursor: grabbing;
  }

  .canvas-stage {
    position: relative;
    transform-origin: 0 0;
  }

  .canvas-edges {
    position: absolute;
    inset: 0;
    z-index: 1;
    overflow: visible;
    pointer-events: none;
  }

  .canvas-edges line {
    stroke-width: 2;
    opacity: 0.72;
  }

  .canvas-edges .connection-draft-line {
    stroke-width: 2;
    stroke-dasharray: 6 5;
    opacity: 0.92;
  }

  .canvas-edges marker path {
    fill: currentColor;
  }

  .canvas-edges text {
    paint-order: stroke;
    stroke: #080d13;
    stroke-width: 4px;
    font-size: 12px;
  }

  .canvas-node {
    position: absolute;
    z-index: 2;
    display: grid;
    grid-template-rows: auto 1fr;
    min-width: 0;
    border: 1px solid color-mix(in srgb, var(--canvas-accent) 52%, #26313f);
    border-radius: 8px;
    background: #0c1219;
    box-shadow: 0 8px 24px rgb(0 0 0 / 28%);
    color: #d5dde8;
    overflow: visible;
  }

  .canvas-group-node {
    z-index: 0;
    border-style: dashed;
    background: rgb(11 16 23 / 44%);
    box-shadow: none;
    opacity: 0.72;
  }

  .canvas-node-header {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    padding: 8px 10px;
    color: var(--canvas-accent);
    font-weight: 700;
    font-size: 13px;
    border-bottom: 1px solid #1d2936;
  }

  .canvas-node-header span,
  .canvas-node-header button {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .canvas-node-header button {
    border: 0;
    padding: 0;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .canvas-node-header .canvas-node-delete {
    flex: 0 0 auto;
    margin-left: auto;
    color: #8592a4;
  }

  .resize-handle {
    position: absolute;
    z-index: 2;
    width: 12px;
    height: 12px;
    border: 1px solid color-mix(in srgb, var(--canvas-accent) 72%, #0b1118);
    border-radius: 3px;
    background: #0b1118;
    opacity: 0;
    padding: 0;
  }

  .connection-handle {
    position: absolute;
    z-index: 4;
    width: 13px;
    height: 13px;
    border: 1px solid color-mix(in srgb, var(--canvas-accent) 82%, #0b1118);
    border-radius: 999px;
    background: #081018;
    box-shadow: 0 0 0 2px #0c1219;
    opacity: 0.46;
    padding: 0;
    cursor: crosshair;
  }

  .connection-top {
    top: -6px;
    left: 50%;
    transform: translateX(-50%);
  }

  .connection-right {
    top: 50%;
    right: -6px;
    transform: translateY(-50%);
  }

  .connection-bottom {
    bottom: -6px;
    left: 50%;
    transform: translateX(-50%);
  }

  .connection-left {
    top: 50%;
    left: -6px;
    transform: translateY(-50%);
  }

  .canvas-node:hover .connection-handle,
  .connection-handle:focus {
    opacity: 1;
  }

  .connection-handle:hover,
  .connection-handle:focus {
    background: var(--canvas-accent);
    outline: none;
  }

  .canvas-node:hover .resize-handle,
  .resize-handle:focus {
    opacity: 1;
  }

  .resize-nw {
    top: 3px;
    left: 3px;
    cursor: nwse-resize;
  }

  .resize-ne {
    top: 3px;
    right: 3px;
    cursor: nesw-resize;
  }

  .resize-sw {
    bottom: 3px;
    left: 3px;
    cursor: nesw-resize;
  }

  .resize-se {
    right: 3px;
    bottom: 3px;
    cursor: nwse-resize;
  }

  .canvas-node p {
    margin: 0;
    min-width: 0;
    overflow: hidden;
    padding: 10px;
    color: #98a6b8;
    font-size: 12px;
    text-overflow: ellipsis;
  }

  .canvas-text-body {
    min-height: 0;
    overflow: auto;
    padding: 10px;
    color: #c6d0dc;
    font: 12px/1.4 ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  }

  .canvas-text-body :global(p) {
    margin: 0 0 4px;
    padding: 0;
    overflow: visible;
    color: #c6d0dc;
    font: inherit;
    white-space: normal;
  }

  .canvas-text-body :global(h1),
  .canvas-text-body :global(h2),
  .canvas-text-body :global(h3),
  .canvas-text-body :global(h4),
  .canvas-text-body :global(h5),
  .canvas-text-body :global(h6) {
    margin: 8px 0 4px;
    color: #8fe8cf;
    font-size: 13px;
    line-height: 1.25;
  }

  .canvas-text-body :global(hr) {
    height: 1px;
    margin: 8px 0;
    border: 0;
    background: #607084;
  }

  .canvas-text-body :global(.mdp-embed),
  .embedded-note-body :global(.mdp-embed) {
    display: grid;
    gap: 4px;
    margin: 4px 0 8px;
    border: 1px solid #26313d;
    border-radius: 5px;
    background: #0b1118;
    padding: 8px;
    color: #aeb8c4;
  }

  .canvas-text-body :global(.mdp-embed-title),
  .embedded-note-body :global(.mdp-embed-title) {
    color: #8bd5bd;
    font-weight: 720;
    text-decoration: none;
  }

  .canvas-text-body :global(.mdp-missing-embed .mdp-embed-title),
  .embedded-note-body :global(.mdp-missing-embed .mdp-embed-title) {
    color: #6f827c;
  }

  .file-card-preview {
    display: grid;
    align-content: start;
    gap: 8px;
    min-height: 0;
    overflow: hidden;
    padding: 10px;
  }

  .file-card-preview.editing-file-preview {
    grid-template-rows: auto minmax(0, 1fr) auto;
    align-content: stretch;
    height: 100%;
  }

  .embedded-properties {
    display: grid;
    gap: 4px;
    min-width: 0;
  }

  .embedded-property-list {
    display: grid;
    gap: 2px;
    min-width: 0;
  }

  .embedded-property-row {
    display: grid;
    grid-template-columns: 22px minmax(74px, 0.28fr) minmax(0, 1fr) 22px;
    align-items: center;
    gap: 4px;
    position: relative;
    min-width: 0;
    min-height: 22px;
  }

  .embedded-property-type-cell {
    display: grid;
    place-items: center;
    position: relative;
    min-width: 0;
  }

  .embedded-property-type-button,
  .embedded-property-remove-button {
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    border-color: transparent;
    background: transparent;
    color: #7f8a98;
    padding: 0;
  }

  .embedded-property-type-button:hover,
  .embedded-property-type-button:focus,
  .embedded-property-type-button.active,
  .embedded-property-remove-button:hover,
  .embedded-property-remove-button:focus {
    border-color: #303946;
    background: #10161f;
    color: #d7dde4;
  }

  .embedded-property-type-menu {
    display: grid;
    position: absolute;
    top: 22px;
    left: 0;
    z-index: 12;
    width: 180px;
    border: 1px solid #26313d;
    border-radius: 5px;
    background: #0a1017;
    box-shadow: 0 12px 32px rgb(0 0 0 / 42%);
    padding: 4px;
  }

  .embedded-property-type-menu button {
    display: grid;
    grid-template-columns: 16px minmax(0, 1fr) 16px;
    align-items: center;
    gap: 7px;
    min-height: 27px;
    border-color: transparent;
    background: transparent;
    color: #b9c7d5;
    padding: 3px 6px;
    text-align: left;
  }

  .embedded-property-type-menu button:hover,
  .embedded-property-type-menu button:focus,
  .embedded-property-type-menu button.active {
    background: #101923;
    color: #f0f4f8;
  }

  .embedded-property-label,
  .embedded-property-row input {
    min-width: 0;
    min-height: 20px;
    border-color: transparent;
    background: transparent;
    padding: 1px 3px;
    font-size: 11px;
    line-height: 1.3;
  }

  .embedded-property-label,
  .embedded-property-name-input {
    color: #7f8a98;
  }

  .embedded-property-label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .embedded-property-row input {
    color: #d7dde4;
  }

  .embedded-property-row input:focus {
    border-color: #303946;
    background: #10161f;
    outline: none;
  }

  .embedded-formatted-property-input {
    color: #d7dde4;
  }

  .embedded-token-property-field {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 4px;
    min-width: 0;
    min-height: 20px;
    padding: 1px 3px;
  }

  .embedded-property-token {
    display: inline-flex;
    align-items: center;
    max-width: 100%;
    gap: 2px;
    border: 1px solid #245c50;
    border-radius: 999px;
    background: #10211e;
    color: #8bd5bd;
    padding: 1px 4px 1px 6px;
    font-size: 11px;
    line-height: 1.2;
  }

  .embedded-property-token button {
    display: grid;
    place-items: center;
    width: 14px;
    height: 14px;
    border: 0;
    background: transparent;
    color: #61b89e;
    padding: 0;
  }

  .embedded-property-token button:hover,
  .embedded-property-token button:focus {
    color: #c4f5e5;
    outline: none;
  }

  .embedded-token-property-input {
    flex: 1 1 72px;
    width: auto;
    min-width: 72px;
  }

  .embedded-checkbox-property-field {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    min-height: 20px;
    color: #d7dde4;
    font-size: 11px;
  }

  .embedded-checkbox-property-field input {
    width: 14px;
    height: 14px;
    min-height: 0;
    accent-color: #2ea987;
  }

  .embedded-add-property-button {
    justify-self: start;
    border-color: transparent;
    background: transparent;
    color: #7f8a98;
    padding: 1px 3px;
    font-size: 11px;
  }

  .embedded-add-property-button:hover,
  .embedded-add-property-button:focus {
    border-color: #303946;
    background: #10161f;
    color: #d7dde4;
  }

  .embedded-property-remove-button {
    opacity: 0;
  }

  .embedded-property-row:hover .embedded-property-remove-button,
  .embedded-property-remove-button:focus {
    opacity: 1;
  }

  .file-card-preview dl {
    display: grid;
    gap: 4px;
    margin: 0;
    min-width: 0;
  }

  .file-card-preview dl div {
    display: grid;
    grid-template-columns: minmax(72px, 0.3fr) minmax(0, 1fr);
    align-items: start;
    gap: 10px;
    min-width: 0;
  }

  .file-card-preview dt,
  .file-card-preview dd {
    margin: 0;
    min-width: 0;
    overflow: hidden;
    font-size: 11px;
    line-height: 1.4;
    text-overflow: ellipsis;
  }

  .file-card-preview dt {
    color: #7f8a98;
    white-space: nowrap;
  }

  .file-card-preview dd {
    color: #c6d0dc;
    white-space: pre-wrap;
  }

  .file-card-preview dd.empty-property {
    color: #687586;
  }

  .property-token-list {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    min-width: 0;
  }

  .property-token-list span {
    max-width: 100%;
    overflow: hidden;
    border: 1px solid #2a7666;
    border-radius: 999px;
    background: #123b33;
    color: #8fe8cf;
    padding: 1px 6px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .file-card-lines {
    display: grid;
    align-content: start;
    gap: 2px;
    min-height: 0;
    margin: 0;
    overflow: hidden;
    color: #98a6b8;
    font: 12px/1.35 ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  }

  .file-card-lines div {
    min-height: 1.35em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: pre;
  }

  .file-card-lines .blank-preview-line::before {
    content: ' ';
  }

  .embedded-note-body {
    min-height: 0;
    overflow: auto;
    color: #c6d0dc;
    font: 12px/1.4 ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  }

  .embedded-note-body :global(h1),
  .embedded-note-body :global(h2),
  .embedded-note-body :global(h3),
  .embedded-note-body :global(h4),
  .embedded-note-body :global(h5),
  .embedded-note-body :global(h6) {
    margin: 8px 0 4px;
    color: #8fe8cf;
    font-size: 13px;
    line-height: 1.25;
  }

  .embedded-note-body :global(p) {
    margin: 0 0 4px;
    padding: 0;
    overflow: visible;
    color: #c6d0dc;
    font: inherit;
    white-space: normal;
  }

  .embedded-note-body :global(hr) {
    height: 1px;
    margin: 8px 0;
    border: 0;
    background: #607084;
  }

  .embedded-note-body :global(ul),
  .embedded-note-body :global(ol) {
    margin: 4px 0 6px 18px;
    padding: 0;
  }

  .embedded-note-body :global(pre) {
    margin: 6px 0;
    padding: 8px;
    border: 1px solid #233244;
    border-radius: 6px;
    background: #080d13;
    overflow: auto;
  }

  .embedded-note-body :global(code) {
    color: #8bd5bd;
  }

  .embedded-note-body :global(a),
  .embedded-note-body :global(.mdp-inline-tag) {
    color: #67d9bd;
  }

  .embedded-note-body :global(.mdp-blank-line) {
    height: 1.35em;
  }

  .embedded-note-editor {
    min-width: 0;
    min-height: 0;
    height: 100%;
    border: 1px solid #2a3848;
    border-radius: 6px;
    background: #080d13;
    overflow: hidden;
  }

  .embedded-note-editor :global(.cm-editor) {
    height: 100%;
    font-size: 12px;
  }

  .embedded-note-editor :global(.cm-content) {
    padding: 6px 0;
  }

  .embedded-note-editor :global(.cm-scroller) {
    overflow: auto;
  }

  .embedded-save-state {
    color: #7f8a98;
    font-size: 11px;
  }

  .file-card-preview.missing-preview .file-card-lines {
    color: #8793a3;
  }

  .canvas-context-menu {
    position: fixed;
    z-index: 80;
    display: grid;
    min-width: 190px;
    padding: 6px;
    border: 1px solid #263445;
    border-radius: 7px;
    background: #0c1219;
    box-shadow: 0 18px 40px rgb(0 0 0 / 42%);
  }

  .canvas-context-menu button {
    display: flex;
    align-items: center;
    width: 100%;
    border: 0;
    border-radius: 5px;
    background: transparent;
    color: #d5dde8;
    padding: 7px 9px;
    font: inherit;
    font-size: 13px;
    text-align: left;
  }

  .canvas-context-menu button:hover,
  .canvas-context-menu button:focus {
    background: #13202c;
    outline: none;
  }

  .canvas-context-menu .danger {
    color: #ff9a9a;
  }

  .context-menu-separator {
    height: 1px;
    margin: 5px 3px;
    background: #223041;
  }

  .canvas-dialog-backdrop {
    position: fixed;
    inset: 0;
    z-index: 90;
    display: grid;
    place-items: center;
    padding: 24px;
    background: rgb(0 0 0 / 42%);
  }

  .canvas-dialog {
    width: min(520px, calc(100vw - 48px));
    max-height: min(640px, calc(100vh - 48px));
    overflow: hidden;
    border: 1px solid #263445;
    border-radius: 8px;
    background: #0b1118;
    box-shadow: 0 24px 70px rgb(0 0 0 / 52%);
    padding: 14px;
  }

  .canvas-dialog-form {
    display: grid;
    gap: 12px;
    min-height: 0;
  }

  .canvas-dialog header,
  .canvas-dialog footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }

  .canvas-dialog h3 {
    margin: 0;
    color: #8fe8cf;
    font-size: 15px;
  }

  .canvas-dialog input {
    width: 100%;
    min-width: 0;
    border: 1px solid #2a3848;
    border-radius: 6px;
    background: #090f15;
    color: #d7dde4;
    padding: 9px 10px;
    font: 13px/1.4 ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  }

  .canvas-dialog-editor {
    min-height: 150px;
    border: 1px solid #2a3848;
    border-radius: 6px;
    background: #090f15;
    overflow: hidden;
  }

  .canvas-dialog-editor :global(.cm-editor) {
    min-height: 150px;
    max-height: 320px;
  }

  .canvas-dialog-editor :global(.cm-scroller) {
    overflow: auto;
  }

  .canvas-dialog input:focus,
  .canvas-dialog-editor:focus-within {
    border-color: #58c6ad;
    outline: none;
    box-shadow: 0 0 0 1px rgb(88 198 173 / 32%);
  }

  .canvas-dialog button {
    border: 1px solid #2a3848;
    border-radius: 6px;
    background: #121923;
    color: #d5dde8;
    padding: 7px 11px;
    font: inherit;
    font-size: 13px;
  }

  .canvas-dialog button:hover,
  .canvas-dialog button:focus {
    border-color: #3b526a;
    outline: none;
  }

  .canvas-dialog button:disabled {
    color: #687586;
    cursor: not-allowed;
  }

  .canvas-dialog .primary {
    border-color: #2f7c6b;
    background: #0d3029;
    color: #9ff2dd;
  }

  .canvas-dialog .dialog-close {
    border: 0;
    background: transparent;
    color: #8793a3;
    padding-inline: 6px;
  }

  .document-results {
    display: grid;
    gap: 6px;
    max-height: 260px;
    overflow: auto;
    border: 1px solid #1c2a38;
    border-radius: 6px;
    background: #080d13;
    padding: 6px;
  }

  .document-results button {
    display: grid;
    gap: 2px;
    justify-items: start;
    width: 100%;
    border: 0;
    background: transparent;
    padding: 8px;
    text-align: left;
  }

  .document-results button:hover,
  .document-results button:focus {
    background: #122019;
  }

  .document-results span {
    max-width: 100%;
    overflow: hidden;
    color: #8fe8cf;
    font-weight: 700;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .document-results small,
  .document-empty {
    color: #7f8a98;
    font-size: 12px;
  }

  .document-empty {
    padding: 8px;
  }

  .canvas-empty,
  .canvas-error {
    margin: 18px;
    color: #7f8a98;
  }

  .canvas-error {
    padding: 14px;
    color: #ff9a9a;
  }
</style>
