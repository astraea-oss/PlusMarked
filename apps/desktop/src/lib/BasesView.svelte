<script lang="ts">
  import { onDestroy } from 'svelte';
  import {
    Calculator,
    Columns3,
    Copy,
    Download,
    Filter,
    Plus,
    RefreshCcw,
    Search,
    Table,
    X
  } from '@lucide/svelte';
  import { getNoteSource, listNotes, renameBase, saveNoteSource } from '$lib/api';
  import type { NoteSummary } from '$lib/types';

  type BaseLayout = 'table' | 'kanban' | 'list' | 'cards';
  type SortDirection = 'asc' | 'desc';
  type ColumnType = 'text' | 'number' | 'date' | 'datetime' | 'checkbox' | 'list' | 'formula';
  type FilterOperator = 'contains' | 'equals' | 'not-equals' | 'filled' | 'empty' | 'greater' | 'less';
  type FilterRule = {
    id: string;
    key: string;
    operator: FilterOperator;
    value: string;
  };
  type StoredFormulaColumn = Pick<BaseColumn, 'key' | 'label' | 'visible' | 'formula'>;
  type BaseViewState = {
    version: 1;
    layout: BaseLayout;
    searchText: string;
    filterRules: FilterRule[];
    sortKey: string;
    sortDirection: SortDirection;
    groupKey: string;
    limit: number;
    visibleColumnKeys: string[];
    formulaColumns: StoredFormulaColumn[];
    kanbanPropertyKey: string;
    kanbanLanesByProperty: Record<string, string[]>;
  };
  type BaseColumn = {
    key: string;
    label: string;
    type: ColumnType;
    visible: boolean;
    formula?: string;
  };
  type BaseRow = {
    id: string;
    title: string;
    path: string;
    source: string;
    body: string;
    properties: Record<string, string>;
  };

  export let notes: NoteSummary[] = [];
  export let baseId: string | undefined = undefined;
  export let selectedId: string | undefined = undefined;
  export let baseTitle = 'Bases+';
  export let onOpenNote: (id: string) => void = () => {};
  export let onNotesChanged: (notes: NoteSummary[]) => void = () => {};
  export let setStatus: (message: string) => void = () => {};

  let rows: BaseRow[] = [];
  let loading = false;
  let loadedSignature = '';
  let layout: BaseLayout = 'table';
  let searchText = '';
  let filterKey = '';
  let filterOperator: FilterOperator = 'contains';
  let filterValue = '';
  let filterRules: FilterRule[] = [];
  let sortKey = 'updated_at';
  let sortDirection: SortDirection = 'desc';
  let groupKey = '';
  let limit = 100;
  let visibleColumnKeys: string[] = [];
  let showColumns = false;
  let propertySearch = '';
  let kanbanPropertyKey = 'progress';
  let kanbanLanes: string[] = [];
  let kanbanLanesByProperty: Record<string, string[]> = {};
  let kanbanLaneDraft = '';
  let draggingKanbanRowId: string | null = null;
  let draggingKanbanLane: string | null = null;
  let newPropertyName = '';
  let formulaName = '';
  let formulaExpression = '';
  let formulaColumns: BaseColumn[] = [];
  let titleDraft = baseTitle;
  let lastBaseTitle = baseTitle;
  let titleSaving = false;
  let baseSource = '';
  let loadedBaseStateKey = '';
  let applyingBaseViewState = false;
  let viewStateSaveTimer: ReturnType<typeof setTimeout> | null = null;

  $: baseNoteRows = notes.filter((note) => note.note_type !== 'base');
  $: noteSignature = baseNoteRows.map((note) => `${note.id}:${note.updated_at}`).join('|');
  $: if (noteSignature && noteSignature !== loadedSignature && !loading) {
    void loadBaseRows();
  }
  $: propertyColumns = buildPropertyColumns(rows, formulaColumns);
  $: activeColumns = propertyColumns.filter((column) => isColumnShown(column));
  $: baseStateKey = baseId ?? selectedId ?? 'base';
  $: if (baseStateKey !== loadedBaseStateKey) {
    void loadBaseViewState(baseStateKey);
  }
  $: filteredRows = applyBaseFilters(rows, activeColumns, propertyColumns, filterRules);
  $: sortedRows = sortBaseRows(filteredRows, propertyColumns);
  $: limitedRows = sortedRows.slice(0, Math.max(1, limit || sortedRows.length || 1));
  $: groupedRows = groupBaseRows(limitedRows, groupKey);
  $: kanbanPropertyColumns = propertyColumns.filter((column) => column.type !== 'formula' && column.key !== 'id');
  $: if (!kanbanPropertyKey && kanbanPropertyColumns.length) {
    kanbanPropertyKey = kanbanPropertyColumns.some((column) => column.key === 'progress')
      ? 'progress'
      : kanbanPropertyColumns[0].key;
  }
  $: kanbanPropertyColumn = kanbanPropertyColumns.find((column) => column.key === kanbanPropertyKey);
  $: kanbanLanes = kanbanLanesByProperty[kanbanPropertyKey] ?? [];
  $: kanbanGroups = buildKanbanGroups(limitedRows, kanbanPropertyColumn, kanbanLanes);
  $: activeFilterColumn = propertyColumns.find((column) => column.key === filterKey);
  $: propertySearchNeedle = propertySearch.trim().toLowerCase();
  $: shownPropertyColumns = activeColumns.filter((column) => propertyMenuMatches(column, propertySearchNeedle));
  $: availablePropertyColumns = propertyColumns.filter((column) => !isColumnShown(column) && propertyMenuMatches(column, propertySearchNeedle));
  $: if (!titleSaving && baseTitle !== lastBaseTitle) {
    titleDraft = baseTitle;
    lastBaseTitle = baseTitle;
  }

  onDestroy(() => {
    if (!viewStateSaveTimer) return;
    clearTimeout(viewStateSaveTimer);
    viewStateSaveTimer = null;
    void saveBaseViewState();
  });

  async function loadBaseRows() {
    if (!baseNoteRows.length) {
      rows = [];
      loadedSignature = noteSignature;
      return;
    }

    loading = true;
    setStatus('Loading Bases+ rows...');
    try {
      const loaded = await Promise.all(baseNoteRows.map(async (note) => {
        const source = await getNoteSource(note.id);
        const split = splitMarkdownPlusSource(source.source);
        const properties = parseFrontmatter(split.frontmatter);
        return {
          id: note.id,
          title: unquoteYamlScalar(properties.title ?? '') || note.title,
          path: note.path,
          source: source.source,
          body: split.body,
          properties: {
            id: properties.id ?? note.id,
            title: properties.title ?? quoteYamlString(note.title),
            updated_at: properties.updated_at ?? quoteYamlString(note.updated_at),
            type: properties.type ?? note.note_type,
            ...properties
          }
        };
      }));

      rows = loaded;
      loadedSignature = noteSignature;
      setStatus(`Bases+ loaded ${loaded.length} rows.`);
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      loading = false;
    }
  }

  function buildPropertyColumns(baseRows: BaseRow[], formulas: BaseColumn[]): BaseColumn[] {
    const preferred = ['title', 'type', 'tags', 'aliases', 'created_at', 'updated_at', 'id'];
    const keys = new Set<string>(preferred);
    for (const row of baseRows) {
      for (const key of Object.keys(row.properties)) {
        keys.add(key);
      }
    }

    const ordered = [
      ...preferred.filter((key) => keys.has(key)),
      ...Array.from(keys).filter((key) => !preferred.includes(key)).sort((a, b) => a.localeCompare(b))
    ];

    return [
      ...ordered.map((key) => ({
        key,
        label: propertyLabel(key),
        type: inferColumnType(key, baseRows),
        visible: true
      })),
      ...formulas
    ];
  }

  function inferColumnType(key: string, baseRows: BaseRow[]): ColumnType {
    if (key === 'tags' || key === 'aliases') return 'list';

    const values = baseRows
      .map((row) => unquoteYamlScalar(row.properties[key] ?? '').trim())
      .filter(Boolean);
    if (!values.length) return 'text';
    if (values.every((value) => /^(true|false)$/i.test(value))) return 'checkbox';
    if (values.every((value) => /^-?\d+(?:\.\d+)?$/.test(value))) return 'number';
    if (values.every((value) => /^\d{4}-\d{2}-\d{2}[T\s]\d{2}:\d{2}/.test(value))) return 'datetime';
    if (values.every((value) => /^\d{4}-\d{2}-\d{2}$/.test(value))) return 'date';
    if (values.every((value) => value.startsWith('[') && value.endsWith(']'))) return 'list';
    return 'text';
  }

  function applyBaseFilters(
    baseRows: BaseRow[],
    searchColumns: BaseColumn[],
    filterColumns: BaseColumn[],
    rules: FilterRule[]
  ) {
    const search = searchText.trim().toLowerCase();
    return baseRows.filter((row) => {
      if (search) {
        const haystack = searchColumns
          .filter((column) => column.visible)
          .map((column) => displayCellValue(row, column))
          .join(' ')
          .toLowerCase();
        if (!haystack.includes(search)) return false;
      }

      return rules.every((rule) => rowMatchesFilter(row, rule, filterColumns));
    });
  }

  function rowMatchesFilter(row: BaseRow, rule: FilterRule, filterColumns: BaseColumn[]) {
    const column = filterColumns.find((candidate) => candidate.key === rule.key);
    if (!column) return true;

    const raw = displayCellValue(row, column);
    const value = raw.toLowerCase();
    const needle = rule.value.trim().toLowerCase();

    if (rule.operator === 'filled') return Boolean(raw.trim());
    if (rule.operator === 'empty') return !raw.trim();
    if (rule.operator === 'equals') return value === needle;
    if (rule.operator === 'not-equals') return value !== needle;
    if (rule.operator === 'greater') return numericValue(raw) > numericValue(rule.value);
    if (rule.operator === 'less') return numericValue(raw) < numericValue(rule.value);
    return value.includes(needle);
  }

  function filterNeedsValue(operator: FilterOperator) {
    return operator !== 'filled' && operator !== 'empty';
  }

  function saveFilterRules(nextRules: FilterRule[]) {
    filterRules = nextRules;
    scheduleBaseViewStateSave();
  }

  function addFilterRule() {
    if (!filterKey) return;
    if (filterNeedsValue(filterOperator) && !filterValue.trim()) return;

    const nextRule = {
      id: makeFilterRuleId(),
      key: filterKey,
      operator: filterOperator,
      value: filterNeedsValue(filterOperator) ? filterValue.trim() : ''
    };
    saveFilterRules([...filterRules, nextRule]);
    filterValue = '';
    setStatus(`Added filter: ${filterRuleLabel(nextRule)}.`);
  }

  function removeFilterRule(id: string) {
    saveFilterRules(filterRules.filter((rule) => rule.id !== id));
  }

  function handleFilterDraftKeydown(event: KeyboardEvent) {
    if (event.key !== 'Enter') return;
    event.preventDefault();
    addFilterRule();
  }

  function filterRuleLabel(rule: FilterRule) {
    const column = propertyColumns.find((candidate) => candidate.key === rule.key);
    const label = column?.label ?? propertyLabel(rule.key);
    if (!filterNeedsValue(rule.operator)) return `${label} ${filterOperatorLabel(rule.operator)}`;
    return `${label} ${filterOperatorLabel(rule.operator)} ${rule.value}`;
  }

  function filterOperatorLabel(operator: FilterOperator) {
    const labels: Record<FilterOperator, string> = {
      contains: 'contains',
      equals: 'equals',
      'not-equals': 'is not',
      filled: 'is filled',
      empty: 'is empty',
      greater: '>',
      less: '<'
    };
    return labels[operator];
  }

  function isFilterOperator(value: unknown): value is FilterOperator {
    return ['contains', 'equals', 'not-equals', 'filled', 'empty', 'greater', 'less'].includes(String(value));
  }

  function makeFilterRuleId() {
    if (typeof crypto !== 'undefined' && 'randomUUID' in crypto) return crypto.randomUUID();
    return `${Date.now()}-${Math.random().toString(36).slice(2)}`;
  }

  async function loadBaseViewState(key: string) {
    if (viewStateSaveTimer) {
      clearTimeout(viewStateSaveTimer);
      viewStateSaveTimer = null;
    }
    loadedBaseStateKey = key;
    baseSource = '';

    if (!key || !key.startsWith('base:')) {
      applyBaseViewState(null);
      return;
    }

    try {
      const source = await getNoteSource(key);
      if (loadedBaseStateKey !== key) return;

      baseSource = source.source;
      applyBaseViewState(parseBaseViewState(source.source));
    } catch (error) {
      applyBaseViewState(null);
      setStatus(error instanceof Error ? error.message : String(error));
    }
  }

  function applyBaseViewState(state: Partial<BaseViewState> | null) {
    applyingBaseViewState = true;

    layout = isBaseLayout(state?.layout) ? state.layout : 'table';
    searchText = typeof state?.searchText === 'string' ? state.searchText : '';
    filterRules = sanitizeFilterRules(state?.filterRules);
    sortKey = typeof state?.sortKey === 'string' && state.sortKey ? state.sortKey : 'updated_at';
    sortDirection = state?.sortDirection === 'asc' ? 'asc' : 'desc';
    groupKey = typeof state?.groupKey === 'string' ? state.groupKey : '';
    limit = Number.isFinite(state?.limit) && Number(state?.limit) > 0 ? Number(state?.limit) : 100;
    visibleColumnKeys = Array.isArray(state?.visibleColumnKeys)
      ? state.visibleColumnKeys.map((key) => String(key)).filter(Boolean)
      : [];
    formulaColumns = sanitizeFormulaColumns(state?.formulaColumns);
    kanbanPropertyKey = typeof state?.kanbanPropertyKey === 'string' && state.kanbanPropertyKey
      ? state.kanbanPropertyKey
      : 'progress';
    kanbanLanesByProperty = sanitizeKanbanLanesByProperty(state?.kanbanLanesByProperty);

    applyingBaseViewState = false;
  }

  function scheduleBaseViewStateSave() {
    if (applyingBaseViewState || !baseStateKey.startsWith('base:')) return;
    if (viewStateSaveTimer) clearTimeout(viewStateSaveTimer);
    viewStateSaveTimer = setTimeout(() => {
      viewStateSaveTimer = null;
      void saveBaseViewState();
    }, 250);
  }

  async function saveBaseViewState() {
    if (!baseStateKey.startsWith('base:')) return;

    try {
      const source = baseSource || (await getNoteSource(baseStateKey)).source;
      const nextSource = upsertBaseViewState(source, currentBaseViewState());
      if (nextSource === source) return;

      await saveNoteSource({ id: baseStateKey, source: nextSource });
      baseSource = nextSource;
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    }
  }

  function currentBaseViewState(): BaseViewState {
    return {
      version: 1,
      layout,
      searchText,
      filterRules,
      sortKey,
      sortDirection,
      groupKey,
      limit: Math.max(1, Number(limit) || 100),
      visibleColumnKeys: activeColumns.map((column) => column.key),
      formulaColumns: formulaColumns.map((column) => ({
        key: column.key,
        label: column.label,
        visible: column.visible,
        formula: column.formula ?? ''
      })),
      kanbanPropertyKey,
      kanbanLanesByProperty
    };
  }

  function parseBaseViewState(source: string): Partial<BaseViewState> | null {
    const match = source.match(/^# @plusmarked-view-state (.+)$/m);
    if (!match) return null;

    try {
      const parsed = JSON.parse(match[1]);
      return parsed && typeof parsed === 'object' ? parsed : null;
    } catch {
      return null;
    }
  }

  function upsertBaseViewState(source: string, state: BaseViewState) {
    const line = `# @plusmarked-view-state ${JSON.stringify(state)}`;
    if (/^# @plusmarked-view-state .+$/m.test(source)) {
      return source.replace(/^# @plusmarked-view-state .+$/m, line);
    }

    const suffix = source.endsWith('\n') ? '' : '\n';
    return `${source}${suffix}${line}\n`;
  }

  function sanitizeFilterRules(value: unknown): FilterRule[] {
    return Array.isArray(value)
      ? value
          .filter((rule) => rule && typeof rule === 'object')
          .map((rule) => {
            const candidate = rule as Record<string, unknown>;
            return {
              id: typeof candidate.id === 'string' ? candidate.id : makeFilterRuleId(),
              key: typeof candidate.key === 'string' ? candidate.key : '',
              operator: isFilterOperator(candidate.operator) ? candidate.operator : 'contains',
              value: typeof candidate.value === 'string' ? candidate.value : ''
            };
          })
          .filter((rule) => rule.key && (!filterNeedsValue(rule.operator) || rule.value.trim()))
      : [];
  }

  function sanitizeFormulaColumns(value: unknown): BaseColumn[] {
    return Array.isArray(value)
      ? value
          .filter((column) => column && typeof column === 'object')
          .map((column) => {
            const candidate = column as Record<string, unknown>;
            const key = typeof candidate.key === 'string' ? candidate.key : '';
            return {
              key,
              label: typeof candidate.label === 'string' && candidate.label ? candidate.label : propertyLabel(key),
              type: 'formula' as ColumnType,
              visible: candidate.visible !== false,
              formula: typeof candidate.formula === 'string' ? candidate.formula : ''
            };
          })
          .filter((column) => column.key.startsWith('formula.') && Boolean(column.formula))
      : [];
  }

  function sanitizeKanbanLanesByProperty(value: unknown) {
    if (!value || typeof value !== 'object' || Array.isArray(value)) return {};

    const lanes: Record<string, string[]> = {};
    for (const [key, entries] of Object.entries(value)) {
      if (!Array.isArray(entries)) continue;
      const deduped = Array.from(new Set(entries.map((entry) => String(entry).trim()).filter(Boolean)));
      if (key && deduped.length) lanes[key] = deduped;
    }
    return lanes;
  }

  function isBaseLayout(value: unknown): value is BaseLayout {
    return ['table', 'kanban', 'list', 'cards'].includes(String(value));
  }

  function sortBaseRows(baseRows: BaseRow[], columns: BaseColumn[]) {
    const column = columns.find((candidate) => candidate.key === sortKey);
    if (!column) return baseRows;

    return [...baseRows].sort((a, b) => {
      const left = compareValue(a, column);
      const right = compareValue(b, column);
      const direction = sortDirection === 'asc' ? 1 : -1;

      if (left < right) return -1 * direction;
      if (left > right) return 1 * direction;
      return a.title.localeCompare(b.title);
    });
  }

  function setSort(column: BaseColumn) {
    sortDirection = sortKey === column.key && sortDirection === 'asc' ? 'desc' : 'asc';
    sortKey = column.key;
    scheduleBaseViewStateSave();
  }

  function handleLayoutChange() {
    scheduleBaseViewStateSave();
  }

  function handleSearchInput() {
    scheduleBaseViewStateSave();
  }

  function handleGroupChange() {
    scheduleBaseViewStateSave();
  }

  function handleLimitChange() {
    limit = Math.max(1, Number(limit) || 100);
    scheduleBaseViewStateSave();
  }

  function handleKanbanPropertyChange() {
    scheduleBaseViewStateSave();
  }

  function groupBaseRows(baseRows: BaseRow[], key: string) {
    if (!key) return [{ label: '', rows: baseRows }];

    const column = propertyColumns.find((candidate) => candidate.key === key);
    if (!column) return [{ label: '', rows: baseRows }];

    const groups = new Map<string, BaseRow[]>();
    for (const row of baseRows) {
      const label = displayCellValue(row, column) || 'Empty';
      groups.set(label, [...(groups.get(label) ?? []), row]);
    }

    return Array.from(groups.entries()).map(([label, groupRows]) => ({
      label,
      rows: groupRows
    }));
  }

  function buildKanbanGroups(baseRows: BaseRow[], column: BaseColumn | undefined, lanes: string[]) {
    if (!column) return [{ label: 'No property selected', value: '', rows: baseRows }];

    const groups = new Map<string, BaseRow[]>();
    for (const lane of lanes) {
      const normalized = lane.trim();
      if (normalized) groups.set(normalized, []);
    }

    for (const row of baseRows) {
      const value = displayCellValue(row, column).trim();
      const label = value || 'Empty';
      groups.set(label, [...(groups.get(label) ?? []), row]);
    }

    return Array.from(groups.entries()).map(([label, groupRows]) => ({
      label,
      value: label === 'Empty' ? '' : label,
      rows: groupRows
    }));
  }

  function saveKanbanLanes(nextLanes: string[]) {
    const deduped = Array.from(new Set(nextLanes.map((lane) => lane.trim()).filter(Boolean)));
    kanbanLanesByProperty = {
      ...kanbanLanesByProperty,
      [kanbanPropertyKey]: deduped
    };
    scheduleBaseViewStateSave();
  }

  function addKanbanLane() {
    const lane = kanbanLaneDraft.trim();
    if (!lane) return;

    saveKanbanLanes([...kanbanLanes, lane]);
    kanbanLaneDraft = '';
    setStatus(`Added ${lane} Kanban group.`);
  }

  function handleKanbanLaneKeydown(event: KeyboardEvent) {
    if (event.key !== 'Enter') return;
    event.preventDefault();
    addKanbanLane();
  }

  function handleKanbanDragStart(event: DragEvent, row: BaseRow) {
    draggingKanbanRowId = row.id;
    draggingKanbanLane = null;
    event.dataTransfer?.setData('text/plain', row.id);
    event.dataTransfer?.setData('application/x-plusmarked-kanban-card', row.id);
    if (event.dataTransfer) {
      event.dataTransfer.effectAllowed = 'move';
    }
  }

  function handleKanbanDragEnd() {
    draggingKanbanRowId = null;
  }

  function handleKanbanLaneDragStart(event: DragEvent, label: string) {
    draggingKanbanLane = label;
    draggingKanbanRowId = null;
    event.dataTransfer?.setData('text/plain', label);
    event.dataTransfer?.setData('application/x-plusmarked-kanban-lane', label);
    if (event.dataTransfer) {
      event.dataTransfer.effectAllowed = 'move';
    }
  }

  function handleKanbanLaneDragEnd() {
    draggingKanbanLane = null;
  }

  function handleKanbanDragOver(event: DragEvent) {
    event.preventDefault();
    if (event.dataTransfer) {
      event.dataTransfer.dropEffect = 'move';
    }
  }

  async function handleKanbanDrop(event: DragEvent, value: string) {
    event.preventDefault();
    if (event.dataTransfer?.types.includes('application/x-plusmarked-kanban-lane')) return;

    const rowId = event.dataTransfer?.getData('text/plain') || draggingKanbanRowId;
    draggingKanbanRowId = null;

    const row = rows.find((candidate) => candidate.id === rowId);
    if (!row || !kanbanPropertyColumn) return;

    const current = displayCellValue(row, kanbanPropertyColumn).trim();
    if (current === value) return;

    await updateCell(row, kanbanPropertyColumn, value);
  }

  function handleKanbanLaneDrop(event: DragEvent, targetLabel: string) {
    const sourceLabel = event.dataTransfer?.getData('application/x-plusmarked-kanban-lane') || draggingKanbanLane;
    if (!sourceLabel || sourceLabel === targetLabel) return;

    event.preventDefault();
    const labels = kanbanGroups.map((group) => group.label);
    const ordered = labels.filter((label) => label !== sourceLabel);
    const targetIndex = ordered.indexOf(targetLabel);
    ordered.splice(targetIndex === -1 ? ordered.length : targetIndex, 0, sourceLabel);
    saveKanbanLanes(ordered.filter((label) => label !== 'Empty' && label !== 'No property selected'));
    draggingKanbanLane = null;
  }

  async function saveBaseTitle() {
    const title = titleDraft.trim();
    if (!baseStateKey.startsWith('base:') || !title || title === baseTitle) {
      titleDraft = baseTitle;
      return;
    }

    titleSaving = true;
    try {
      const result = await renameBase(baseStateKey, title);
      const nextNotes = await listNotes();
      onNotesChanged(nextNotes);
      titleDraft = result.title;
      lastBaseTitle = result.title;
      onOpenNote(result.id);
      setStatus(`Renamed base to ${result.title}.`);
    } catch (error) {
      titleDraft = baseTitle;
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      titleSaving = false;
    }
  }

  function handleBaseTitleKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter') {
      event.preventDefault();
      void saveBaseTitle();
    }

    if (event.key === 'Escape') {
      titleDraft = baseTitle;
      (event.currentTarget as HTMLInputElement).blur();
    }
  }

  function compareValue(row: BaseRow, column: BaseColumn): string | number {
    const value = displayCellValue(row, column);
    if (column.type === 'number') return numericValue(value);
    if (column.type === 'date' || column.type === 'datetime') return dateSortValue(row.properties[column.key] ?? value);
    if (column.type === 'checkbox') return /^(true|checked)$/i.test(value) ? 1 : 0;
    return value.toLowerCase();
  }

  function displayCellValue(row: BaseRow, column: BaseColumn) {
    if (column.type === 'formula') {
      return evaluateFormula(row, column.formula ?? '');
    }

    const value = row.properties[column.key] ?? '';
    if (column.key === 'tags') {
      return parseTagValue(value).join(', ');
    }

    if (column.type === 'list') {
      return parseYamlArrayValue(value).join(', ');
    }

    if (column.type === 'date') {
      return formatDateValue(value);
    }

    if (column.type === 'datetime') {
      return formatDateTimeValue(value);
    }

    return unquoteYamlScalar(value);
  }

  function editableCellValue(row: BaseRow, column: BaseColumn) {
    if (column.key === 'tags') {
      return parseTagValue(row.properties[column.key] ?? '').join(', ');
    }

    if (column.type === 'list') {
      return parseYamlArrayValue(row.properties[column.key] ?? '').join(', ');
    }

    if (column.type === 'date') {
      return formatDateValue(row.properties[column.key] ?? '');
    }

    if (column.type === 'datetime') {
      return formatDateTimeValue(row.properties[column.key] ?? '');
    }

    return unquoteYamlScalar(row.properties[column.key] ?? '');
  }

  async function updateCell(row: BaseRow, column: BaseColumn, value: string | boolean) {
    if (column.type === 'formula') return;

    const sourceValue = sourceValueForColumn(column, value);
    const nextRows = rows.map((candidate) =>
      candidate.id === row.id
        ? {
            ...candidate,
            title: column.key === 'title' ? unquoteYamlScalar(sourceValue) || candidate.title : candidate.title,
            properties: {
              ...candidate.properties,
              [column.key]: sourceValue
            }
          }
        : candidate
    );
    rows = nextRows;

    const updated = nextRows.find((candidate) => candidate.id === row.id);
    if (!updated) return;

    await saveRow(updated);
  }

  function sourceValueForColumn(column: BaseColumn, value: string | boolean) {
    if (column.type === 'checkbox') return value ? 'true' : 'false';
    const text = String(value).trim();
    if (column.key === 'tags') {
      if (!text) return '[]';
      return sourceTagValueFromTokens(text.split(/[\s,]+/));
    }

    if (column.type === 'list') {
      if (!text) return '[]';
      return sourceValueFromTokens(text.split(',').map((item) => item.trim()).filter(Boolean));
    }
    if (column.type === 'number') return text || '0';
    if (column.type === 'date') return sourceDateValue(text);
    if (column.type === 'datetime') return sourceDateTimeValue(text);
    return quoteYamlString(String(value));
  }

  async function saveRow(row: BaseRow) {
    try {
      const source = serializeRow(row);
      const result = await saveNoteSource({ id: row.id, source });
      const nextNotes = await listNotes();
      onNotesChanged(nextNotes);
      rows = rows.map((candidate) =>
        candidate.id === row.id
          ? { ...candidate, path: result.note.path, source }
          : candidate
      );
      setStatus(`Updated ${result.note.title}.`);
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
      await loadBaseRows();
    }
  }

  function serializeRow(row: BaseRow) {
    const keys = [
      ...['id', 'title', 'created_at', 'updated_at', 'tags', 'aliases', 'type'].filter((key) => key in row.properties),
      ...Object.keys(row.properties)
        .filter((key) => !['id', 'title', 'created_at', 'updated_at', 'tags', 'aliases', 'type'].includes(key))
        .sort((a, b) => a.localeCompare(b))
    ];

    const yaml = keys
      .filter((key) => key.trim())
      .map((key) => `${key}: ${row.properties[key] ?? ''}`)
      .join('\n');

    return `---\n${yaml}\n---\n${row.body}`;
  }

  function addPropertyColumn() {
    const key = slugPropertyName(newPropertyName);
    if (!key) return;

    if (!propertyColumns.some((column) => column.key === key)) {
      rows = rows.map((row) => ({
        ...row,
        properties: {
          ...row.properties,
          [key]: ''
        }
      }));
    }

    newPropertyName = '';
    if (!visibleColumnKeys.includes(key)) visibleColumnKeys = [...visibleColumnKeys, key];
    scheduleBaseViewStateSave();
    setStatus(`Added ${propertyLabel(key)} column.`);
  }

  function addFormulaColumn() {
    const key = `formula.${slugPropertyName(formulaName || 'formula')}`;
    const expression = formulaExpression.trim();
    if (!expression || formulaColumns.some((column) => column.key === key)) return;

    formulaColumns = [
      ...formulaColumns,
      {
        key,
        label: formulaName.trim() || 'Formula',
        type: 'formula',
        visible: true,
        formula: expression
      }
    ];
    formulaName = '';
    formulaExpression = '';
    scheduleBaseViewStateSave();
    setStatus('Added formula column.');
  }

  function removeFormulaColumn(key: string) {
    formulaColumns = formulaColumns.filter((column) => column.key !== key);
    visibleColumnKeys = visibleColumnKeys.filter((columnKey) => columnKey !== key);
    scheduleBaseViewStateSave();
  }

  function propertyMenuMatches(column: BaseColumn, needle: string) {
    if (!needle) return true;
    return `${column.label} ${column.key} ${column.type}`.toLowerCase().includes(needle);
  }

  function isColumnShown(column: BaseColumn) {
    if (visibleColumnKeys.length) return visibleColumnKeys.includes(column.key);
    if (column.type === 'formula') return column.visible;
    return column.key !== 'id';
  }

  function showColumn(column: BaseColumn) {
    if (column.type === 'formula') {
      formulaColumns = formulaColumns.map((candidate) =>
        candidate.key === column.key ? { ...candidate, visible: true } : candidate
      );
    } else if (!visibleColumnKeys.length) {
      visibleColumnKeys = propertyColumns
        .filter((candidate) => candidate.key !== 'id')
        .map((candidate) => candidate.key);
    }

    if (!visibleColumnKeys.includes(column.key)) visibleColumnKeys = [...visibleColumnKeys, column.key];
    rows = [...rows];
    scheduleBaseViewStateSave();
  }

  function hideColumn(column: BaseColumn) {
    if (column.type === 'formula') {
      formulaColumns = formulaColumns.map((candidate) =>
        candidate.key === column.key ? { ...candidate, visible: false } : candidate
      );
    } else if (!visibleColumnKeys.length) {
      visibleColumnKeys = propertyColumns
        .filter((candidate) => candidate.key !== 'id')
        .map((candidate) => candidate.key);
    }

    visibleColumnKeys = visibleColumnKeys.filter((key) => key !== column.key);
    rows = [...rows];
    scheduleBaseViewStateSave();
  }

  function exportCsv() {
    const csv = buildCsv(limitedRows, activeColumns);
    const blob = new Blob([csv], { type: 'text/csv;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.href = url;
    link.download = 'BasesPlus.csv';
    link.click();
    URL.revokeObjectURL(url);
    setStatus('Exported Bases+ CSV.');
  }

  async function copyTable() {
    const text = buildCsv(limitedRows, activeColumns, '\t');
    await navigator.clipboard.writeText(text);
    setStatus('Copied Bases+ view.');
  }

  function buildCsv(baseRows: BaseRow[], columns: BaseColumn[], separator = ',') {
    const escapeCell = (value: string) => {
      if (separator === '\t') return value.replace(/\r?\n/g, ' ');
      return `"${value.replace(/"/g, '""')}"`;
    };

    return [
      columns.map((column) => escapeCell(column.label)).join(separator),
      ...baseRows.map((row) => columns.map((column) => escapeCell(displayCellValue(row, column))).join(separator))
    ].join('\n');
  }

  function columnSummary(column: BaseColumn, baseRows: BaseRow[]) {
    const values = baseRows.map((row) => displayCellValue(row, column)).filter((value) => value.trim());
    if (column.type === 'number' || column.type === 'formula') {
      const numbers = values.map(numericValue).filter((value) => Number.isFinite(value));
      if (!numbers.length) return `${values.length} filled`;
      const sum = numbers.reduce((total, value) => total + value, 0);
      return `Sum ${formatNumber(sum)} / Avg ${formatNumber(sum / numbers.length)}`;
    }

    if (column.type === 'checkbox') {
      const checked = values.filter((value) => /^(true|checked)$/i.test(value)).length;
      return `${checked} checked`;
    }

    return `${values.length} filled`;
  }

  function evaluateFormula(row: BaseRow, expression: string) {
    const replaced = expression.replace(/\b[A-Za-z_][A-Za-z0-9_]*\b/g, (key) => {
      if (['true', 'false'].includes(key)) return key;
      return String(numericValue(unquoteYamlScalar(row.properties[key] ?? '0')));
    });

    if (!/^[\d+\-*/%().<>=!&|,\s]+$/.test(replaced)) return '';

    try {
      const result = Function(`"use strict"; return (${replaced});`)();
      return typeof result === 'number' ? formatNumber(result) : String(result);
    } catch {
      return '';
    }
  }

  function splitMarkdownPlusSource(source: string): { frontmatter: string; body: string } {
    if (!source.startsWith('---')) return { frontmatter: '', body: source };
    const delimiter = source.indexOf('\n---', 3);
    if (delimiter === -1) return { frontmatter: '', body: source };
    const frontmatterStart = source.startsWith('---\r\n') ? 5 : 4;
    return {
      frontmatter: source.slice(frontmatterStart, delimiter),
      body: source.slice(delimiter + 4).replace(/^\r?\n+/, '')
    };
  }

  function parseFrontmatter(frontmatter: string) {
    const properties: Record<string, string> = {};
    for (const line of frontmatter.split(/\r?\n/)) {
      const match = line.match(/^([A-Za-z0-9_-]+):\s*(.*)$/);
      if (match) properties[match[1]] = match[2] ?? '';
    }
    return properties;
  }

  function parseYamlArrayValue(value: string) {
    const raw = value.trim();
    if (!raw || raw === '[]') return [];

    if (raw.startsWith('[') && raw.endsWith(']')) {
      try {
        const parsed = JSON.parse(raw);
        if (Array.isArray(parsed)) return parsed.map((item) => String(item)).filter(Boolean);
      } catch {
        return raw
          .slice(1, -1)
          .split(',')
          .map((item) => unquoteYamlScalar(item.trim()))
          .filter(Boolean);
      }
    }

    return raw.split(',').map((item) => unquoteYamlScalar(item.trim())).filter(Boolean);
  }

  function sourceValueFromTokens(tokens: string[]) {
    return `[${tokens.map(quoteYamlString).join(', ')}]`;
  }

  function parseTagValue(value: string) {
    return parseYamlArrayValue(value)
      .flatMap((token) => token.split(/[\s,]+/))
      .map(normalizeTagToken)
      .filter(Boolean);
  }

  function sourceTagValueFromTokens(tokens: string[]) {
    const deduped = Array.from(new Set(tokens.map(normalizeTagToken).filter(Boolean)));
    return deduped.length ? deduped.map((token) => `#${token}`).join(', ') : '[]';
  }

  function normalizeTagToken(value: string) {
    return value.trim().replace(/^#+/, '').trim();
  }

  function unquoteYamlScalar(value: string) {
    const trimmed = value.trim();
    if (trimmed.startsWith('"') && trimmed.endsWith('"')) {
      try {
        return String(JSON.parse(trimmed));
      } catch {
        return trimmed.slice(1, -1);
      }
    }

    if (trimmed.startsWith("'") && trimmed.endsWith("'")) {
      return trimmed.slice(1, -1);
    }

    return trimmed;
  }

  function formatDateParts(year: string, month: string, day: string) {
    return `${day}/${month}/${year}`;
  }

  function formatDateTimeParts(hour: string, minute: string, year: string, month: string, day: string) {
    return `${formatDateParts(year, month, day)} ${hour}:${minute}`;
  }

  function formatDateValue(value: string) {
    const unquoted = unquoteYamlScalar(value);
    const date = unquoted.match(/^(\d{4})-(\d{2})-(\d{2})$/);
    return date ? formatDateParts(date[1], date[2], date[3]) : unquoted;
  }

  function formatDateTimeValue(value: string) {
    const unquoted = unquoteYamlScalar(value);
    const timestamp = unquoted.match(/^(\d{4})-(\d{2})-(\d{2})[T\s](\d{2}):(\d{2})(?::\d{2}(?:\.\d+)?)?(?:Z|[+-]\d{2}:?\d{2})?$/);
    return timestamp
      ? formatDateTimeParts(timestamp[4], timestamp[5], timestamp[1], timestamp[2], timestamp[3])
      : unquoted;
  }

  function sourceDateValue(value: string) {
    const trimmed = value.trim();
    const friendlyDate = trimmed.match(/^(\d{1,2})[/-](\d{1,2})[/-](\d{4})$/);
    if (friendlyDate) {
      const [, day, month, year] = friendlyDate;
      return `${year}-${month.padStart(2, '0')}-${day.padStart(2, '0')}`;
    }

    return trimmed;
  }

  function sourceDateTimeValue(value: string) {
    const trimmed = value.trim();
    const friendlyDateTime = trimmed.match(/^(\d{1,2})[/-](\d{1,2})[/-](\d{4})\s+(\d{1,2}):(\d{2})$/);
    if (friendlyDateTime) {
      const [, day, month, year, hour, minute] = friendlyDateTime;
      return `${year}-${month.padStart(2, '0')}-${day.padStart(2, '0')}T${hour.padStart(2, '0')}:${minute}:00Z`;
    }

    const friendlyReverseDateTime = trimmed.match(/^(\d{1,2}):(\d{2})\s+(\d{4})[/-](\d{1,2})[/-](\d{1,2})$/);
    if (friendlyReverseDateTime) {
      const [, hour, minute, year, month, day] = friendlyReverseDateTime;
      return `${year}-${month.padStart(2, '0')}-${day.padStart(2, '0')}T${hour.padStart(2, '0')}:${minute}:00Z`;
    }

    return trimmed;
  }

  function dateSortValue(value: string) {
    const source = unquoteYamlScalar(value);
    const date = sourceDateValue(source);
    const datetime = sourceDateTimeValue(source);
    return Date.parse(datetime) || Date.parse(date) || 0;
  }

  function quoteYamlString(value: string) {
    return JSON.stringify(value);
  }

  function propertyLabel(key: string) {
    const labels: Record<string, string> = {
      id: 'ID',
      title: 'Title',
      created_at: 'Created',
      updated_at: 'Modified',
      tags: 'Tags',
      aliases: 'Aliases',
      type: 'Type'
    };
    return labels[key] ?? key.replace(/^formula\./, '').replace(/[_-]+/g, ' ');
  }

  function slugPropertyName(value: string) {
    return value.trim().replace(/\s+/g, '_').replace(/[^A-Za-z0-9_-]/g, '').replace(/^_+|_+$/g, '');
  }

  function numericValue(value: string) {
    const number = Number(String(value).replace(/,/g, '').trim());
    return Number.isFinite(number) ? number : 0;
  }

  function formatNumber(value: number) {
    return Number.isInteger(value) ? String(value) : value.toFixed(2).replace(/\.?0+$/, '');
  }
</script>

<div class="bases-view">
  <header class="bases-toolbar">
    <div class="bases-title">
      <Table size={17} />
      <div>
        <input
          class="base-title-input"
          aria-label="Base title"
          bind:value={titleDraft}
          disabled={titleSaving}
          on:blur={saveBaseTitle}
          on:keydown={handleBaseTitleKeydown}
        />
        <p>{limitedRows.length} of {filteredRows.length} rows</p>
      </div>
    </div>

    <div class="bases-actions">
      <label class="view-select" title="View layout">
        <Table size={14} />
        <select bind:value={layout} aria-label="Base view layout" on:change={handleLayoutChange}>
          <option value="table">Table</option>
          <option value="kanban">Kanban</option>
          <option value="list">List</option>
          <option value="cards">Cards</option>
        </select>
      </label>

      <button class:active={showColumns} class="icon-button" title="Properties" on:click={() => (showColumns = !showColumns)}>
        <Columns3 size={14} />
      </button>
      <button class="icon-button" title="Copy" on:click={copyTable}>
        <Copy size={14} />
      </button>
      <button class="icon-button" title="Export CSV" on:click={exportCsv}>
        <Download size={14} />
      </button>
      <button class="icon-button" title="Refresh" disabled={loading} on:click={loadBaseRows}>
        <RefreshCcw size={14} />
      </button>
    </div>
  </header>

  <section class="bases-controls">
    <label class="search-field">
      <Search size={14} />
      <input bind:value={searchText} placeholder="Search displayed properties" on:input={handleSearchInput} />
    </label>

    <label>
      <Filter size={14} />
      <select bind:value={filterKey} on:keydown={handleFilterDraftKeydown}>
        <option value="">No filter</option>
        {#each propertyColumns as column}
          <option value={column.key}>{column.label}</option>
        {/each}
      </select>
    </label>

    <select bind:value={filterOperator} disabled={!filterKey} on:keydown={handleFilterDraftKeydown}>
      <option value="contains">contains</option>
      <option value="equals">equals</option>
      <option value="not-equals">not equals</option>
      <option value="filled">is filled</option>
      <option value="empty">is empty</option>
      <option value="greater">greater than</option>
      <option value="less">less than</option>
    </select>

    <input
      bind:value={filterValue}
      disabled={!filterKey || filterOperator === 'filled' || filterOperator === 'empty'}
      placeholder={activeFilterColumn ? activeFilterColumn.label : 'Value'}
      on:keydown={handleFilterDraftKeydown}
    />

    <button class="add-filter-button" type="button" disabled={!filterKey || (filterNeedsValue(filterOperator) && !filterValue.trim())} on:click={addFilterRule}>
      <Plus size={13} /> Filter
    </button>

    <select bind:value={groupKey} on:change={handleGroupChange}>
      <option value="">No group</option>
      {#each propertyColumns as column}
        <option value={column.key}>Group by {column.label}</option>
      {/each}
    </select>

    <input class="limit-input" type="number" min="1" bind:value={limit} title="Result limit" on:change={handleLimitChange} />
  </section>

  {#if filterRules.length}
    <section class="active-filter-rules" aria-label="Active filters">
      {#each filterRules as rule}
        <span class="filter-rule-chip">
          {filterRuleLabel(rule)}
          <button type="button" aria-label={`Remove ${filterRuleLabel(rule)}`} on:click={() => removeFilterRule(rule.id)}>
            <X size={12} />
          </button>
        </span>
      {/each}
    </section>
  {/if}

  {#if showColumns}
    <section class="properties-menu" aria-label="Properties">
      <header>
        <div>
          <strong>Properties</strong>
          <span>{activeColumns.length} shown</span>
        </div>
        <button type="button" aria-label="Close properties" on:click={() => (showColumns = false)}>
          <X size={13} />
        </button>
      </header>

      <label class="property-menu-search">
        <Search size={13} />
        <input bind:value={propertySearch} placeholder="Find a property" />
      </label>

      <div class="property-menu-sections">
        <section>
          <h3>Shown</h3>
          {#if shownPropertyColumns.length}
            <div class="property-tile-grid">
              {#each shownPropertyColumns as column}
                <button
                  class="property-tile-button"
                  type="button"
                  disabled={activeColumns.length <= 1}
                  title={activeColumns.length <= 1 ? 'At least one property must stay visible' : `Hide ${column.label}`}
                  on:click={() => hideColumn(column)}
                >
                  <X size={12} />
                  <span>{column.label}</span>
                  <em>{column.type}</em>
                </button>
              {/each}
            </div>
          {:else}
            <p>No visible properties match.</p>
          {/if}
        </section>

        <section>
          <h3>Available</h3>
          {#if availablePropertyColumns.length}
            <div class="property-tile-grid">
              {#each availablePropertyColumns as column}
                <button class="property-tile-button" type="button" on:click={() => showColumn(column)}>
                  <Plus size={12} />
                  <span>{column.label}</span>
                  <em>{column.type}</em>
                </button>
              {/each}
            </div>
          {:else}
            <p>No hidden properties match.</p>
          {/if}
        </section>
      </div>

      <footer>
        <div class="add-column-row">
          <input bind:value={newPropertyName} placeholder="New property" />
          <button on:click={addPropertyColumn}><Plus size={13} /> Add</button>
        </div>

        <div class="formula-row">
          <input bind:value={formulaName} placeholder="Formula name" />
          <input bind:value={formulaExpression} placeholder="price * quantity" />
          <button on:click={addFormulaColumn}><Calculator size={13} /> Formula</button>
        </div>
      </footer>
    </section>
  {/if}

  <section class="bases-content" class:is-loading={loading}>
    {#if layout === 'table'}
      <div class="base-table-wrap">
        <table class="base-table">
          <thead>
            <tr>
              {#each activeColumns as column}
                <th>
                  <button
                    class:active={sortKey === column.key}
                    on:click={() => setSort(column)}
                  >
                    {column.label}
                    {#if sortKey === column.key}
                      <span>{sortDirection === 'asc' ? 'A-Z' : 'Z-A'}</span>
                    {/if}
                  </button>
                </th>
              {/each}
            </tr>
          </thead>
          <tbody>
            {#each groupedRows as group}
              {#if group.label}
                <tr class="group-row">
                  <td colspan={activeColumns.length}>{group.label} <span>{group.rows.length}</span></td>
                </tr>
              {/if}
              {#each group.rows as row}
                <tr class:active={row.id === selectedId}>
                  {#each activeColumns as column}
                    <td>
                      {#if column.type === 'formula'}
                        <span class="formula-cell">{displayCellValue(row, column)}</span>
                      {:else if column.type === 'checkbox'}
                        <input
                          type="checkbox"
                          checked={/^(true)$/i.test(editableCellValue(row, column))}
                          on:change={(event) => updateCell(row, column, event.currentTarget.checked)}
                        />
                      {:else}
                        <input
                          class:note-title-cell={column.key === 'title'}
                          value={editableCellValue(row, column)}
                          on:change={(event) => updateCell(row, column, event.currentTarget.value)}
                          on:dblclick={() => onOpenNote(row.id)}
                        />
                      {/if}
                    </td>
                  {/each}
                </tr>
              {/each}
            {/each}
          </tbody>
          <tfoot>
            <tr>
              {#each activeColumns as column}
                <td>{columnSummary(column, limitedRows)}</td>
              {/each}
            </tr>
          </tfoot>
        </table>
      </div>
    {:else if layout === 'kanban'}
      <div class="kanban-shell">
        <div class="kanban-toolbar">
          <label>
            <span>Kanban property</span>
            <select bind:value={kanbanPropertyKey} on:change={handleKanbanPropertyChange}>
              {#each kanbanPropertyColumns as column}
                <option value={column.key}>{column.label}</option>
              {/each}
            </select>
          </label>

          <div class="kanban-add-lane">
            <input
              bind:value={kanbanLaneDraft}
              placeholder="New group"
              on:keydown={handleKanbanLaneKeydown}
            />
            <button type="button" disabled={!kanbanLaneDraft.trim()} on:click={addKanbanLane}>
              <Plus size={13} /> Group
            </button>
          </div>
        </div>

      <div class="base-kanban" aria-label="Kanban view">
        {#each kanbanGroups as group}
          <section
            class:drop-target={Boolean(draggingKanbanRowId)}
            class:lane-drop-target={Boolean(draggingKanbanLane)}
            class="kanban-column"
            aria-label={group.label}
            on:dragover={handleKanbanDragOver}
            on:drop={(event) => handleKanbanLaneDrop(event, group.label)}
          >
            <button
              class="kanban-column-header"
              type="button"
              draggable="true"
              on:dragstart={(event) => handleKanbanLaneDragStart(event, group.label)}
              on:dragend={handleKanbanLaneDragEnd}
              on:drop={(event) => handleKanbanDrop(event, group.value)}
            >
              <strong>{group.label}</strong>
              <span>{group.rows.length}</span>
            </button>
            <div
              class="kanban-cards"
              role="list"
              on:drop={(event) => handleKanbanDrop(event, group.value)}
            >
              {#each group.rows as row}
                <button
                  class:active={row.id === selectedId}
                  class:is-dragging={draggingKanbanRowId === row.id}
                  draggable="true"
                  on:click={() => onOpenNote(row.id)}
                  on:dragstart={(event) => handleKanbanDragStart(event, row)}
                  on:dragend={handleKanbanDragEnd}
                >
                  <strong>{row.title}</strong>
                  {#each activeColumns.filter((column) => column.key !== 'title' && column.key !== kanbanPropertyKey).slice(0, 4) as column}
                    <span><em>{column.label}</em>{displayCellValue(row, column) || 'Empty'}</span>
                  {/each}
                </button>
              {/each}
            </div>
          </section>
        {/each}
      </div>
      </div>
    {:else if layout === 'list'}
      <div class="base-list">
        {#each limitedRows as row}
          <button class:active={row.id === selectedId} on:click={() => onOpenNote(row.id)}>
            <strong>{row.title}</strong>
            <span>{activeColumns.slice(1, 5).map((column) => `${column.label}: ${displayCellValue(row, column) || 'Empty'}`).join(' / ')}</span>
          </button>
        {/each}
      </div>
    {:else}
      <div class="base-cards">
        {#each limitedRows as row}
          <button class:active={row.id === selectedId} on:click={() => onOpenNote(row.id)}>
            <strong>{row.title}</strong>
            {#each activeColumns.filter((column) => column.key !== 'title').slice(0, 6) as column}
              <span><em>{column.label}</em>{displayCellValue(row, column) || 'Empty'}</span>
            {/each}
          </button>
        {/each}
      </div>
    {/if}
  </section>
</div>

<style>
  .bases-view {
    display: grid;
    grid-template-rows: auto auto auto minmax(0, 1fr);
    gap: 0.5rem;
    min-height: 0;
    color: #d7dde4;
  }

  .bases-toolbar,
  .bases-controls {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    min-width: 0;
  }

  .bases-title {
    display: flex;
    align-items: center;
    gap: 0.48rem;
    min-width: 0;
    color: #8bd5bd;
  }

  .bases-title p {
    margin: 0;
  }

  .base-title-input {
    width: min(24rem, 100%);
    min-height: 1.5rem;
    border-color: transparent;
    background: transparent;
    padding: 0;
    color: #8bd5bd;
    font-size: 1rem;
    font-weight: 700;
    line-height: 1.15;
  }

  .base-title-input:focus {
    border-color: #2ea987;
    background: #10161f;
    box-shadow: none;
    padding: 0 0.24rem;
  }

  .bases-title p {
    color: #7d8896;
    font-size: 0.72rem;
  }

  .bases-actions {
    display: flex;
    align-items: center;
    gap: 0.28rem;
  }

  .view-select {
    display: inline-flex;
    align-items: center;
    gap: 0.32rem;
    border: 1px solid #303946;
    border-radius: 5px;
    background: #161b22;
    padding: 0 0.35rem;
    color: #aeb8c4;
  }

  .view-select select {
    width: 6.6rem;
    min-height: 1.55rem;
    border: 0;
    background: transparent;
    padding: 0 0.2rem;
    color: #d7dde4;
    font-size: 0.76rem;
    outline: none;
  }

  .view-select:focus-within {
    border-color: #2ea987;
    box-shadow: 0 0 0 1px rgba(46, 169, 135, 0.18);
  }

  .bases-controls {
    justify-content: start;
    overflow: auto;
    border-top: 1px solid #232b36;
    border-bottom: 1px solid #232b36;
    padding: 0.42rem 0;
  }

  .bases-controls label,
  .search-field {
    display: inline-flex;
    align-items: center;
    gap: 0.28rem;
  }

  .bases-controls input,
  .bases-controls select,
  .properties-menu input {
    min-height: 1.7rem;
    font-size: 0.76rem;
  }

  .search-field input {
    width: 15rem;
  }

  .limit-input {
    width: 4.8rem;
  }

  .add-filter-button {
    display: inline-flex;
    align-items: center;
    gap: 0.28rem;
    min-height: 1.7rem;
    padding: 0 0.52rem;
    font-size: 0.76rem;
  }

  .active-filter-rules {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.35rem;
    min-width: 0;
    margin-top: -0.15rem;
  }

  .filter-rule-chip {
    display: inline-flex;
    align-items: center;
    gap: 0.32rem;
    max-width: 32rem;
    border: 1px solid #245445;
    border-radius: 999px;
    background: #0d211d;
    color: #8bd5bd;
    padding: 0.18rem 0.28rem 0.18rem 0.5rem;
    font-size: 0.72rem;
    line-height: 1.2;
  }

  .filter-rule-chip button {
    display: grid;
    place-items: center;
    width: 1rem;
    height: 1rem;
    border: 0;
    background: transparent;
    color: #8bd5bd;
    padding: 0;
  }

  .properties-menu {
    display: grid;
    gap: 0.5rem;
    border: 1px solid #232b36;
    border-radius: 5px;
    background: #0b0f14;
    padding: 0.5rem;
    font-size: 0.76rem;
  }

  .properties-menu header,
  .properties-menu footer,
  .property-menu-search,
  .add-column-row,
  .formula-row {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    min-width: 0;
  }

  .properties-menu header {
    justify-content: space-between;
  }

  .properties-menu header div {
    display: grid;
    gap: 0.05rem;
  }

  .properties-menu header strong,
  .properties-menu h3 {
    margin: 0;
    color: #e6edf3;
    font-size: 0.78rem;
  }

  .properties-menu header span,
  .properties-menu p,
  .property-tile-button em {
    color: #7d8896;
    font-size: 0.7rem;
    font-style: normal;
  }

  .properties-menu header > button {
    display: grid;
    place-items: center;
    width: 1.45rem;
    height: 1.45rem;
    border: 0;
    background: transparent;
    color: #7d8896;
    padding: 0;
  }

  .property-menu-search input {
    max-width: 20rem;
  }

  .property-menu-sections {
    display: grid;
    gap: 0.62rem;
    min-height: 0;
  }

  .property-menu-sections section {
    display: grid;
    align-content: start;
    gap: 0.28rem;
    min-height: 0;
    max-height: 9.5rem;
    overflow: auto;
  }

  .property-tile-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(8.75rem, 1fr));
    gap: 0.34rem;
  }

  .property-tile-button {
    display: grid;
    grid-template-columns: 1rem minmax(0, 1fr);
    grid-template-rows: auto auto;
    align-items: start;
    column-gap: 0.34rem;
    row-gap: 0.12rem;
    min-height: 3.25rem;
    border-color: #232b36;
    background: #10161f;
    padding: 0.42rem 0.48rem;
    text-align: left;
  }

  .property-tile-button span {
    min-width: 0;
    color: #d7dde4;
    font-size: 0.76rem;
    line-height: 1.15;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .property-tile-button em {
    grid-column: 2;
    min-width: 0;
    line-height: 1.1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .property-tile-button:hover,
  .property-tile-button:focus {
    border-color: #536173;
    background: #1c232d;
  }

  .property-tile-button:disabled {
    opacity: 0.45;
  }

  .properties-menu footer {
    flex-wrap: wrap;
    border-top: 1px solid #1b222c;
    padding-top: 0.5rem;
  }

  .add-column-row,
  .formula-row {
    flex: 1 1 20rem;
  }

  .bases-content {
    min-height: 0;
    overflow: hidden;
  }

  .bases-content.is-loading {
    opacity: 0.68;
  }

  .base-table-wrap {
    height: 100%;
    overflow: auto;
    border: 1px solid #232b36;
    border-radius: 5px;
    background: #0b0f14;
  }

  .base-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.78rem;
  }

  .base-table th,
  .base-table td {
    min-width: 9rem;
    border-right: 1px solid #1b222c;
    border-bottom: 1px solid #1b222c;
    padding: 0;
    text-align: left;
  }

  .base-table th {
    position: sticky;
    top: 0;
    z-index: 2;
    background: #10161f;
  }

  .base-table th button {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    min-height: 1.8rem;
    border: 0;
    border-radius: 0;
    background: transparent;
    color: #aeb8c4;
    padding: 0.28rem 0.42rem;
    text-align: left;
  }

  .base-table th button.active {
    color: #8bd5bd;
  }

  .base-table th span,
  .group-row span {
    color: #687586;
    font-size: 0.68rem;
  }

  .base-table input {
    width: 100%;
    min-height: 1.8rem;
    border: 0;
    border-radius: 0;
    background: transparent;
    color: #d7dde4;
    padding: 0.26rem 0.42rem;
    font-size: 0.78rem;
  }

  .base-table input[type='checkbox'] {
    width: 0.9rem;
    height: 0.9rem;
    min-height: 0;
    margin: 0.42rem;
    accent-color: #2ea987;
  }

  .base-table input:focus {
    outline: 1px solid #2ea987;
    outline-offset: -1px;
    background: #10161f;
  }

  .base-table tr.active input,
  .base-table tr.active .formula-cell {
    background: #10211e;
  }

  .note-title-cell {
    color: #8bd5bd !important;
    font-weight: 700;
  }

  .formula-cell {
    display: block;
    min-height: 1.8rem;
    padding: 0.32rem 0.42rem;
    color: #aeb8c4;
  }

  .group-row td {
    position: sticky;
    top: 1.8rem;
    z-index: 1;
    background: #0f141b;
    color: #8bd5bd;
    padding: 0.3rem 0.42rem;
    font-weight: 700;
  }

  .base-table tfoot td {
    position: sticky;
    bottom: 0;
    background: #10161f;
    color: #7d8896;
    padding: 0.32rem 0.42rem;
    font-size: 0.72rem;
  }

  .kanban-shell {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    gap: 0.45rem;
    height: 100%;
    min-height: 0;
  }

  .kanban-toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.45rem;
    border: 1px solid #232b36;
    border-radius: 5px;
    background: #0b0f14;
    padding: 0.42rem;
  }

  .kanban-toolbar label,
  .kanban-add-lane {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    min-width: 0;
  }

  .kanban-toolbar label span {
    color: #9aa6b2;
    font-size: 0.72rem;
    white-space: nowrap;
  }

  .kanban-toolbar select,
  .kanban-toolbar input {
    width: 12rem;
    min-height: 1.72rem;
    font-size: 0.76rem;
  }

  .kanban-add-lane button {
    display: inline-flex;
    align-items: center;
    gap: 0.24rem;
    min-height: 1.72rem;
    padding: 0.2rem 0.48rem;
    font-size: 0.76rem;
  }

  .base-kanban,
  .base-list,
  .base-cards {
    display: grid;
    align-content: start;
    gap: 0.38rem;
    height: 100%;
    overflow: auto;
  }

  .base-kanban {
    grid-auto-flow: column;
    grid-auto-columns: minmax(14rem, 18rem);
    align-content: stretch;
    align-items: stretch;
  }

  .kanban-column {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    min-height: 0;
    border: 1px solid #232b36;
    border-radius: 5px;
    background: #0b0f14;
  }

  .kanban-column.drop-target {
    border-color: #2a3a48;
  }

  .kanban-column.drop-target:hover {
    border-color: #2ea987;
    background: #0d1419;
  }

  .kanban-column.lane-drop-target {
    outline: 1px dashed rgba(139, 213, 189, 0.26);
    outline-offset: -3px;
  }

  .kanban-column-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    gap: 0.5rem;
    border: 0;
    border-bottom: 1px solid #1b222c;
    border-radius: 0;
    background: transparent;
    padding: 0.5rem 0.55rem;
    cursor: grab;
    user-select: none;
  }

  .kanban-column-header:active {
    cursor: grabbing;
  }

  .kanban-column-header strong {
    overflow: hidden;
    color: #8bd5bd;
    font-size: 0.8rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .kanban-column-header span {
    color: #687586;
    font-size: 0.72rem;
  }

  .kanban-cards {
    display: grid;
    align-content: start;
    gap: 0.36rem;
    min-height: 0;
    overflow: auto;
    padding: 0.45rem;
  }

  .base-list button,
  .base-cards button,
  .kanban-cards button {
    display: grid;
    gap: 0.2rem;
    border-color: #232b36;
    background: #0b0f14;
    color: #d7dde4;
    padding: 0.48rem;
    text-align: left;
  }

  .kanban-cards button {
    cursor: grab;
  }

  .kanban-cards button:active {
    cursor: grabbing;
  }

  .kanban-cards button.is-dragging {
    opacity: 0.46;
  }

  .base-list button.active,
  .base-cards button.active,
  .kanban-cards button.active {
    border-color: #2ea987;
    background: #10211e;
  }

  .base-list strong,
  .base-cards strong,
  .kanban-cards strong {
    color: #8bd5bd;
  }

  .base-list span,
  .base-cards span,
  .kanban-cards span {
    color: #9aa6b2;
    font-size: 0.76rem;
  }

  .base-cards {
    grid-template-columns: repeat(auto-fill, minmax(12rem, 1fr));
  }

  .base-cards span,
  .kanban-cards span {
    display: grid;
    gap: 0.06rem;
  }

  .base-cards em,
  .kanban-cards em {
    color: #687586;
    font-style: normal;
  }
</style>
