<script lang="ts">
  import {
    Calculator,
    Columns3,
    Copy,
    Download,
    Filter,
    Grid2X2,
    List,
    Plus,
    RefreshCcw,
    Search,
    Table,
    X
  } from '@lucide/svelte';
  import { getNoteSource, listNotes, saveNoteSource } from '$lib/api';
  import type { NoteSummary } from '$lib/types';

  type BaseLayout = 'table' | 'list' | 'cards';
  type SortDirection = 'asc' | 'desc';
  type ColumnType = 'text' | 'number' | 'date' | 'datetime' | 'checkbox' | 'list' | 'formula';
  type FilterOperator = 'contains' | 'equals' | 'not-equals' | 'filled' | 'empty' | 'greater' | 'less';
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
  let sortKey = 'updated_at';
  let sortDirection: SortDirection = 'desc';
  let groupKey = '';
  let limit = 100;
  let showColumns = false;
  let newPropertyName = '';
  let formulaName = '';
  let formulaExpression = '';
  let formulaColumns: BaseColumn[] = [];

  $: baseNoteRows = notes.filter((note) => note.note_type !== 'base');
  $: noteSignature = baseNoteRows.map((note) => `${note.id}:${note.updated_at}`).join('|');
  $: if (noteSignature && noteSignature !== loadedSignature && !loading) {
    void loadBaseRows();
  }
  $: propertyColumns = buildPropertyColumns(rows, formulaColumns);
  $: visibleColumns = propertyColumns.filter((column) => column.visible);
  $: activeColumns = visibleColumns.filter((column) => isColumnVisible(column.key));
  $: filteredRows = applyBaseFilters(rows, activeColumns);
  $: sortedRows = sortBaseRows(filteredRows, propertyColumns);
  $: limitedRows = sortedRows.slice(0, Math.max(1, limit || sortedRows.length || 1));
  $: groupedRows = groupBaseRows(limitedRows, groupKey);
  $: activeFilterColumn = propertyColumns.find((column) => column.key === filterKey);

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
        visible: key !== 'id'
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

  function applyBaseFilters(baseRows: BaseRow[], columns: BaseColumn[]) {
    const search = searchText.trim().toLowerCase();
    return baseRows.filter((row) => {
      if (search) {
        const haystack = columns
          .filter((column) => column.visible)
          .map((column) => displayCellValue(row, column))
          .join(' ')
          .toLowerCase();
        if (!haystack.includes(search)) return false;
      }

      if (!filterKey) return true;

      const column = columns.find((candidate) => candidate.key === filterKey);
      if (!column) return true;

      const raw = displayCellValue(row, column);
      const value = raw.toLowerCase();
      const needle = filterValue.trim().toLowerCase();

      if (filterOperator === 'filled') return Boolean(raw.trim());
      if (filterOperator === 'empty') return !raw.trim();
      if (filterOperator === 'equals') return value === needle;
      if (filterOperator === 'not-equals') return value !== needle;
      if (filterOperator === 'greater') return numericValue(raw) > numericValue(filterValue);
      if (filterOperator === 'less') return numericValue(raw) < numericValue(filterValue);
      return value.includes(needle);
    });
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
    setStatus('Added formula column.');
  }

  function removeFormulaColumn(key: string) {
    formulaColumns = formulaColumns.filter((column) => column.key !== key);
  }

  function toggleColumn(key: string) {
    if (key.startsWith('formula.')) {
      formulaColumns = formulaColumns.map((column) =>
        column.key === key ? { ...column, visible: !column.visible } : column
      );
      return;
    }

    const hiddenKey = `bases-plus-hidden-${key}`;
    const current = localStorage.getItem(hiddenKey) === 'true';
    localStorage.setItem(hiddenKey, String(!current));
    rows = [...rows];
  }

  function isColumnVisible(key: string, fallback = true) {
    return localStorage.getItem(`bases-plus-hidden-${key}`) === 'true' ? false : fallback;
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
        <h2>{baseTitle}</h2>
        <p>{limitedRows.length} of {filteredRows.length} rows</p>
      </div>
    </div>

    <div class="bases-actions">
      <div class="segmented-control" aria-label="Base layout">
        <button class:active={layout === 'table'} title="Table" on:click={() => (layout = 'table')}>
          <Table size={14} />
        </button>
        <button class:active={layout === 'list'} title="List" on:click={() => (layout = 'list')}>
          <List size={14} />
        </button>
        <button class:active={layout === 'cards'} title="Cards" on:click={() => (layout = 'cards')}>
          <Grid2X2 size={14} />
        </button>
      </div>

      <button class="icon-button" title="Columns" on:click={() => (showColumns = !showColumns)}>
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
      <input bind:value={searchText} placeholder="Search displayed properties" />
    </label>

    <label>
      <Filter size={14} />
      <select bind:value={filterKey}>
        <option value="">No filter</option>
        {#each propertyColumns as column}
          <option value={column.key}>{column.label}</option>
        {/each}
      </select>
    </label>

    <select bind:value={filterOperator} disabled={!filterKey}>
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
    />

    <select bind:value={groupKey}>
      <option value="">No group</option>
      {#each propertyColumns as column}
        <option value={column.key}>Group by {column.label}</option>
      {/each}
    </select>

    <input class="limit-input" type="number" min="1" bind:value={limit} title="Result limit" />
  </section>

  {#if showColumns}
    <section class="bases-columns" aria-label="Columns">
      {#each propertyColumns as column}
        <label>
          <input
            type="checkbox"
            checked={column.visible && isColumnVisible(column.key)}
            on:change={() => toggleColumn(column.key)}
          />
          <span>{column.label}</span>
          {#if column.type === 'formula'}
            <button type="button" aria-label="Remove formula" on:click={() => removeFormulaColumn(column.key)}>
              <X size={12} />
            </button>
          {/if}
        </label>
      {/each}

      <div class="add-column-row">
        <input bind:value={newPropertyName} placeholder="New property" />
        <button on:click={addPropertyColumn}><Plus size={13} /> Property</button>
      </div>

      <div class="formula-row">
        <input bind:value={formulaName} placeholder="Formula name" />
        <input bind:value={formulaExpression} placeholder="price * quantity" />
        <button on:click={addFormulaColumn}><Calculator size={13} /> Formula</button>
      </div>
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
                    on:click={() => {
                      sortDirection = sortKey === column.key && sortDirection === 'asc' ? 'desc' : 'asc';
                      sortKey = column.key;
                    }}
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

  .bases-title h2,
  .bases-title p {
    margin: 0;
  }

  .bases-title h2 {
    color: #8bd5bd;
    font-size: 1rem;
    line-height: 1.15;
  }

  .bases-title p {
    color: #7d8896;
    font-size: 0.72rem;
  }

  .bases-actions {
    display: flex;
    gap: 0.28rem;
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
  .bases-columns input {
    min-height: 1.7rem;
    font-size: 0.76rem;
  }

  .search-field input {
    width: 15rem;
  }

  .limit-input {
    width: 4.8rem;
  }

  .bases-columns {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.35rem;
    border-bottom: 1px solid #1b222c;
    padding-bottom: 0.45rem;
  }

  .bases-columns label,
  .add-column-row,
  .formula-row {
    display: inline-flex;
    align-items: center;
    gap: 0.24rem;
    border: 1px solid #232b36;
    border-radius: 5px;
    background: #0b0f14;
    padding: 0.18rem 0.28rem;
    font-size: 0.76rem;
  }

  .bases-columns label button {
    display: grid;
    place-items: center;
    width: 1.2rem;
    height: 1.2rem;
    border: 0;
    background: transparent;
    color: #7d8896;
    padding: 0;
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

  .base-list,
  .base-cards {
    display: grid;
    align-content: start;
    gap: 0.38rem;
    height: 100%;
    overflow: auto;
  }

  .base-list button,
  .base-cards button {
    display: grid;
    gap: 0.2rem;
    border-color: #232b36;
    background: #0b0f14;
    color: #d7dde4;
    padding: 0.48rem;
    text-align: left;
  }

  .base-list button.active,
  .base-cards button.active {
    border-color: #2ea987;
    background: #10211e;
  }

  .base-list strong,
  .base-cards strong {
    color: #8bd5bd;
  }

  .base-list span,
  .base-cards span {
    color: #9aa6b2;
    font-size: 0.76rem;
  }

  .base-cards {
    grid-template-columns: repeat(auto-fill, minmax(12rem, 1fr));
  }

  .base-cards span {
    display: grid;
    gap: 0.06rem;
  }

  .base-cards em {
    color: #687586;
    font-style: normal;
  }
</style>
