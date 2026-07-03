<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands';
  import { markdown } from '@codemirror/lang-markdown';
  import { HighlightStyle, syntaxHighlighting } from '@codemirror/language';
  import { EditorSelection, EditorState, type Range } from '@codemirror/state';
  import { tags } from '@lezer/highlight';
  import {
    Decoration,
    EditorView,
    WidgetType,
    ViewPlugin,
    keymap,
    lineNumbers,
    placeholder,
    type DecorationSet,
    type ViewUpdate
  } from '@codemirror/view';

  export let value = '';
  export let ariaLabel = 'MarkdownPlus editor';
  export let onChange: (value: string) => void = () => {};
  export let onInternalLink: (target: string) => void = () => {};
  export let onExternalLink: (target: string) => void = () => {};
  export let internalLinkExists: (target: string) => boolean = () => true;
  export let internalLinkSignature = '';

  let host: HTMLDivElement;
  let view: EditorView | null = null;
  let editorValue = '';
  let lastInternalLinkSignature = '';
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
  const enclosingPairKeymap = Object.entries(enclosingPairs).map(([open, close]) => ({
    key: open,
    run: (editorView: EditorView) => insertEnclosingPair(editorView, open, close)
  }));
  const closingPairKeymap = Array.from(closingPairs).map((close) => ({
    key: close,
    run: (editorView: EditorView) => skipClosingPair(editorView, close)
  }));

  const markdownPlusHighlight = HighlightStyle.define([
    { tag: tags.strong, color: '#f0f4f8', fontWeight: '760' },
    { tag: tags.emphasis, color: '#b9c7d5', fontStyle: 'italic' },
    { tag: tags.link, color: '#4fbda0', textDecoration: 'underline' },
    { tag: tags.monospace, color: '#8bd5bd' },
    { tag: tags.quote, color: '#9aa6b2' },
    { tag: tags.processingInstruction, color: '#687586' }
  ]);

  const markdownPlusTheme = EditorView.theme(
    {
      '&': {
        height: '100%',
        backgroundColor: 'transparent',
        color: '#d7dde4',
        fontSize: '0.88rem'
      },
      '&.cm-focused': {
        outline: 'none'
      },
      '.cm-scroller': {
        overflow: 'auto',
        fontFamily: '"SFMono-Regular", Consolas, "Liberation Mono", Menlo, ui-monospace, monospace',
        lineHeight: '1.35'
      },
      '.cm-gutters': {
        border: '0',
        backgroundColor: 'transparent',
        color: '#687586'
      },
      '.cm-lineNumbers .cm-gutterElement': {
        minWidth: '2.4rem',
        padding: '0 0.55rem 0 0.2rem'
      },
      '.cm-activeLineGutter': {
        backgroundColor: 'rgba(79, 189, 160, 0.06)',
        color: '#9aa6b2'
      },
      '.cm-content': {
        minHeight: '100%',
        padding: '0.12rem 0 0',
        caretColor: '#e6edf3'
      },
      '.cm-line': {
        padding: '0'
      },
      '.cm-activeLine': {
        backgroundColor: 'rgba(79, 189, 160, 0.06)'
      },
      '.cm-selectionBackground, &.cm-focused .cm-selectionBackground': {
        backgroundColor: '#1f473f'
      },
      '.cm-cursor': {
        borderLeftColor: '#e6edf3'
      },
      '.cm-mdp-divider-line': {
        color: '#7d8896'
      },
      '.cm-mdp-before-divider, .cm-mdp-before-divider *': {
        color: '#d7dde4 !important',
        fontSize: '0.88rem !important',
        fontStyle: 'normal',
        fontWeight: '400 !important'
      },
      '.cm-mdp-heading-1': {
        color: '#8bd5bd',
        fontSize: '1.22em',
        fontWeight: '760'
      },
      '.cm-mdp-heading-2': {
        color: '#8bd5bd',
        fontSize: '1.12em',
        fontWeight: '740'
      },
      '.cm-mdp-heading-3': {
        color: '#8bd5bd',
        fontSize: '1.04em',
        fontWeight: '720'
      },
      '.cm-mdp-divider-line::after': {
        content: '""',
        display: 'inline-block',
        width: 'min(22rem, 64%)',
        verticalAlign: 'middle',
        borderTop: '2px solid #5d6b7c'
      },
      '.cm-mdp-link': {
        color: '#8bd5bd',
        textDecoration: 'underline',
        textUnderlineOffset: '0.12em',
        cursor: 'pointer'
      },
      '.cm-mdp-internal-link': {
        borderRadius: '4px',
        padding: '0 0.12rem',
        color: '#8bd5bd',
        textDecoration: 'none'
      },
      '.cm-mdp-missing-internal-link': {
        color: '#7fa497'
      },
      '.cm-mdp-internal-link:hover, .cm-mdp-external-link:hover': {
        color: '#c4f5e5'
      },
      '.cm-mdp-missing-internal-link:hover': {
        color: '#a8c9bd'
      },
      '.cm-mdp-editing-link': {
        color: '#4fbda0',
        textDecoration: 'underline',
        textUnderlineOffset: '0.12em'
      },
      '.cm-mdp-inline-tag': {
        border: '1px solid #245c50',
        borderRadius: '999px',
        backgroundColor: '#10211e',
        color: '#8bd5bd',
        padding: '0 0.22rem',
        fontWeight: '650'
      },
      '.cm-mdp-list-bullet': {
        display: 'inline-grid',
        placeItems: 'center',
        width: '1.1rem',
        color: '#8bd5bd',
        fontWeight: '760'
      },
      '.cm-mdp-task-marker': {
        display: 'inline-grid',
        placeItems: 'center',
        width: '1.1rem',
        color: '#8bd5bd',
        cursor: 'pointer',
        verticalAlign: '-0.12rem'
      },
      '.cm-mdp-task-checkbox': {
        display: 'block',
        width: '0.82rem',
        height: '0.82rem',
        border: '1px solid #245c50',
        borderRadius: '3px',
        backgroundColor: '#0d1117',
        color: '#8bd5bd'
      },
      '.cm-mdp-task-marker[data-mdp-task-checked="true"] .cm-mdp-task-checkbox': {
        backgroundColor: '#10211e'
      },
      '.cm-mdp-task-marker[data-mdp-task-checked="true"] .cm-mdp-task-checkbox::after': {
        content: '"✓"',
        display: 'block',
        fontSize: '0.68rem',
        lineHeight: '1'
      }
    },
    { dark: true }
  );

  const markdownPlusLinePlugin = ViewPlugin.fromClass(
    class {
      decorations: DecorationSet;

      constructor(view: EditorView) {
        this.decorations = lineDecorations(view);
      }

      update(update: ViewUpdate) {
        if (update.docChanged || update.viewportChanged || update.selectionSet) {
          this.decorations = lineDecorations(update.view);
        }
      }
    },
    {
      decorations: (plugin) => plugin.decorations
    }
  );

  onMount(() => {
    editorValue = value;
    view = new EditorView({
      parent: host,
      state: EditorState.create({
        doc: value,
        extensions: [
          history(),
          markdown(),
          placeholder(''),
          lineNumbers(),
          EditorView.lineWrapping,
          markdownPlusTheme,
          syntaxHighlighting(markdownPlusHighlight),
          markdownPlusLinePlugin,
          EditorView.domEventHandlers({
            mousedown: (event, editorView) => handleEditorMouseDown(event, editorView)
          }),
          keymap.of([indentWithTab, ...enclosingPairKeymap, ...closingPairKeymap, ...defaultKeymap, ...historyKeymap]),
          EditorView.contentAttributes.of({
            'aria-label': ariaLabel,
            spellcheck: 'false'
          }),
          EditorView.updateListener.of((update) => {
            if (!update.docChanged) return;

            const nextValue = update.state.doc.toString();
            editorValue = nextValue;
            onChange(nextValue);
          })
        ]
      })
    });
  });

  $: if (view && value !== editorValue && value !== view.state.doc.toString()) {
    editorValue = value;
    view.dispatch({
      changes: {
        from: 0,
        to: view.state.doc.length,
        insert: value
      }
    });
  }

  $: if (view && internalLinkSignature !== lastInternalLinkSignature) {
    lastInternalLinkSignature = internalLinkSignature;
    view.dispatch({ selection: view.state.selection });
  }

  onDestroy(() => {
    view?.destroy();
  });

  function lineDecorations(editorView: EditorView): DecorationSet {
    const decorations = [];

    for (const { from, to } of editorView.visibleRanges) {
      let position = from;
      while (position <= to) {
        const line = editorView.state.doc.lineAt(position);
        const divider = line.text.match(/^([ \t]*)-{3,}[ \t]*$/);
        if (divider) {
          decorations.push(Decoration.line({ class: 'cm-mdp-divider-line' }).range(line.from));
          addDividerMarkerDecorations(editorView, decorations, line.text, line.from, divider[1].length);
          if (line.number > 1) {
            const previousLine = editorView.state.doc.line(line.number - 1);
            if (previousLine.text.trim() && !/^\s*#{1,3}\s+\S/.test(previousLine.text)) {
              decorations.push(Decoration.line({ class: 'cm-mdp-before-divider' }).range(previousLine.from));
            }
          }
        } else {
          const heading = line.text.match(/^(\s*)(#{1,3})(\s+)\S/);
          if (heading) {
            decorations.push(Decoration.line({ class: `cm-mdp-heading-${heading[2].length}` }).range(line.from));
            addHeadingMarkerDecorations(editorView, decorations, line.from, heading);
          }
        }
        addListDecorations(editorView, decorations, line.text, line.from);
        addEmphasisDecorations(editorView, decorations, line.text, line.from);
        addLinkDecorations(editorView, decorations, line.text, line.from);
        addInlineTagDecorations(editorView, decorations, line.text, line.from);
        position = line.to + 1;
      }
    }

    return Decoration.set(decorations, true);
  }

  function addDividerMarkerDecorations(
    editorView: EditorView,
    decorations: Range<Decoration>[],
    text: string,
    lineFrom: number,
    indentLength: number
  ) {
    const markerStart = lineFrom + indentLength;
    const markerEnd = lineFrom + text.length;
    if (isRangeBeingEdited(editorView, markerStart, markerEnd)) return;

    decorations.push(Decoration.replace({}).range(markerStart, markerEnd));
  }

  function addHeadingMarkerDecorations(
    editorView: EditorView,
    decorations: Range<Decoration>[],
    lineFrom: number,
    heading: RegExpMatchArray
  ) {
    const markerStart = lineFrom + heading[1].length;
    const markerEnd = markerStart + heading[2].length + heading[3].length;
    if (isRangeBeingEdited(editorView, markerStart, markerEnd)) return;

    decorations.push(Decoration.replace({}).range(markerStart, markerEnd));
  }

  function addEmphasisDecorations(
    editorView: EditorView,
    decorations: Range<Decoration>[],
    text: string,
    lineFrom: number
  ) {
    addDelimitedTextDecorations(editorView, decorations, text, lineFrom, /(^|[^*])\*\*([^\s*](?:[^*\n]*?[^\s*])?)\*\*(?!\*)/g, 2);
    addDelimitedTextDecorations(editorView, decorations, text, lineFrom, /(^|[^*])\*([^\s*](?:[^*\n]*?[^\s*])?)\*(?!\*)/g, 1);
  }

  function addDelimitedTextDecorations(
    editorView: EditorView,
    decorations: Range<Decoration>[],
    text: string,
    lineFrom: number,
    pattern: RegExp,
    delimiterLength: number
  ) {
    for (const match of text.matchAll(pattern)) {
      const prefixLength = match[1].length;
      const content = match[2];
      const openingStart = lineFrom + (match.index ?? 0) + prefixLength;
      const openingEnd = openingStart + delimiterLength;
      const closingStart = openingEnd + content.length;
      const closingEnd = closingStart + delimiterLength;

      if (isRangeBeingEdited(editorView, openingStart, closingEnd)) continue;

      decorations.push(Decoration.replace({}).range(openingStart, openingEnd));
      decorations.push(Decoration.replace({}).range(closingStart, closingEnd));
    }
  }

  function addListDecorations(
    editorView: EditorView,
    decorations: Range<Decoration>[],
    text: string,
    lineFrom: number
  ) {
    const task = text.match(/^(\s*)([-*+])(\s+)\[([ xX])\](\s+)/);
    if (task) {
      const markerStart = lineFrom + task[1].length;
      const statusPosition = markerStart + task[2].length + task[3].length + 1;
      const markerEnd = lineFrom + task[0].length;
      if (!isRangeBeingEdited(editorView, markerStart, markerEnd)) {
        decorations.push(
          Decoration.replace({
            widget: new TaskCheckboxWidget(statusPosition, task[4].toLowerCase() === 'x')
          }).range(markerStart, markerEnd)
        );
      }
      return;
    }

    const bullet = text.match(/^(\s*)[-*+]\s+/);
    if (!bullet) return;

    const markerStart = lineFrom + bullet[1].length;
    const markerEnd = lineFrom + bullet[0].length;
    if (isRangeBeingEdited(editorView, markerStart, markerEnd)) return;

    decorations.push(
      Decoration.replace({
        widget: new BulletWidget()
      }).range(markerStart, markerEnd)
    );
  }

  function addLinkDecorations(
    editorView: EditorView,
    decorations: Range<Decoration>[],
    text: string,
    lineFrom: number
  ) {
    const wikiPattern = /\[\[([^\]\n]+)\]\]/g;
    for (const match of text.matchAll(wikiPattern)) {
      const matchText = match[0];
      const rawLink = match[1];
      const matchStart = lineFrom + (match.index ?? 0);
      const matchEnd = matchStart + matchText.length;
      const pipeIndex = rawLink.indexOf('|');
      const target = (pipeIndex === -1 ? rawLink : rawLink.slice(0, pipeIndex)).trim();
      if (!target) continue;

      if (isRangeBeingEdited(editorView, matchStart, matchEnd)) {
        decorations.push(
          Decoration.mark({ class: 'cm-mdp-editing-link' }).range(matchStart, matchEnd)
        );
        continue;
      }

      const contentStart = matchStart + 2;
      const contentEnd = matchEnd - 2;
      const labelStart = pipeIndex === -1 ? contentStart : contentStart + pipeIndex + 1;
      const visibleStart = labelStart < contentEnd ? labelStart : contentStart;
      const visibleEnd = labelStart < contentEnd ? contentEnd : contentStart + target.length;

      decorations.push(Decoration.replace({}).range(matchStart, visibleStart));
      decorations.push(
        Decoration.mark({
          class: `cm-mdp-link cm-mdp-internal-link${internalLinkExists(target) ? '' : ' cm-mdp-missing-internal-link'}`,
          attributes: {
            'data-mdp-internal-link': target,
            title: target
          }
        }).range(visibleStart, visibleEnd)
      );
      decorations.push(Decoration.replace({}).range(visibleEnd, matchEnd));
    }

    const externalPattern = /\[([^\]\n]+)\]\(([^)\n]+)\)/g;
    for (const match of text.matchAll(externalPattern)) {
      const label = match[1];
      const href = match[2].trim();
      const matchStart = lineFrom + (match.index ?? 0);
      const matchEnd = matchStart + match[0].length;
      if (!label || !href || isRangeBeingEdited(editorView, matchStart, matchEnd)) {
        decorations.push(Decoration.mark({ class: 'cm-mdp-editing-link' }).range(matchStart, matchEnd));
        continue;
      }

      const labelStart = matchStart + 1;
      const labelEnd = labelStart + label.length;
      decorations.push(Decoration.replace({}).range(matchStart, labelStart));
      decorations.push(
        Decoration.mark({
          class: 'cm-mdp-link cm-mdp-external-link',
          attributes: {
            'data-mdp-external-link': href,
            title: href
          }
        }).range(labelStart, labelEnd)
      );
      decorations.push(Decoration.replace({}).range(labelEnd, matchEnd));
    }
  }

  function addInlineTagDecorations(
    editorView: EditorView,
    decorations: Range<Decoration>[],
    text: string,
    lineFrom: number
  ) {
    const tagPattern = /(^|[\s([{>])#([A-Za-z0-9_/-]+)/g;
    for (const match of text.matchAll(tagPattern)) {
      const prefix = match[1] ?? '';
      const tag = match[2] ?? '';
      if (!tag) continue;

      const prefixStart = match.index ?? 0;
      const tagStart = lineFrom + prefixStart + prefix.length;
      const tagEnd = tagStart + tag.length + 1;
      if (isMarkdownHeadingTag(text, prefixStart, prefix)) continue;

      if (!isRangeBeingEdited(editorView, tagStart, tagEnd)) {
        decorations.push(Decoration.replace({}).range(tagStart, tagStart + 1));
      }
      decorations.push(Decoration.mark({ class: 'cm-mdp-inline-tag' }).range(tagStart + 1, tagEnd));
    }
  }

  function isMarkdownHeadingTag(text: string, prefixStart: number, prefix: string) {
    return prefixStart === 0 && prefix === '' && /^#{1,6}\s/.test(text);
  }

  function isRangeBeingEdited(editorView: EditorView, from: number, to: number) {
    return editorView.state.selection.ranges.some((range) => {
      if (range.empty) {
        return range.from > from && range.from < to;
      }

      return range.from < to && range.to > from;
    });
  }

  function handleEditorMouseDown(event: MouseEvent, editorView: EditorView) {
    if (handleTaskCheckboxMouseDown(event, editorView)) return true;
    return handleEditorLinkMouseDown(event);
  }

  function handleTaskCheckboxMouseDown(event: MouseEvent, editorView: EditorView) {
    if (event.button !== 0 || !(event.target instanceof Element)) return false;

    const checkbox = event.target.closest<HTMLElement>('[data-mdp-task-position]');
    if (!checkbox?.dataset.mdpTaskPosition) return false;

    event.preventDefault();
    const position = Number(checkbox.dataset.mdpTaskPosition);
    const checked = checkbox.dataset.mdpTaskChecked === 'true';
    editorView.dispatch({
      changes: {
        from: position,
        to: position + 1,
        insert: checked ? ' ' : 'x'
      }
    });
    return true;
  }

  function handleEditorLinkMouseDown(event: MouseEvent) {
    if (event.button !== 0 || !(event.target instanceof Element)) return false;

    const internalLink = event.target.closest<HTMLElement>('[data-mdp-internal-link]');
    if (internalLink?.dataset.mdpInternalLink) {
      event.preventDefault();
      onInternalLink(internalLink.dataset.mdpInternalLink);
      return true;
    }

    const externalLink = event.target.closest<HTMLElement>('[data-mdp-external-link]');
    if (externalLink?.dataset.mdpExternalLink) {
      event.preventDefault();
      onExternalLink(externalLink.dataset.mdpExternalLink);
      return true;
    }

    return false;
  }

  class BulletWidget extends WidgetType {
    toDOM() {
      const bullet = document.createElement('span');
      bullet.className = 'cm-mdp-list-bullet';
      bullet.textContent = '•';
      return bullet;
    }

    ignoreEvent() {
      return false;
    }
  }

  class TaskCheckboxWidget extends WidgetType {
    position: number;
    checked: boolean;

    constructor(position: number, checked: boolean) {
      super();
      this.position = position;
      this.checked = checked;
    }

    eq(widget: TaskCheckboxWidget) {
      return widget.position === this.position && widget.checked === this.checked;
    }

    toDOM() {
      const marker = document.createElement('span');
      marker.className = 'cm-mdp-task-marker';
      marker.dataset.mdpTaskPosition = String(this.position);
      marker.dataset.mdpTaskChecked = String(this.checked);
      marker.setAttribute('role', 'checkbox');
      marker.setAttribute('aria-checked', String(this.checked));

      const checkbox = document.createElement('span');
      checkbox.className = 'cm-mdp-task-checkbox';
      marker.appendChild(checkbox);

      return marker;
    }

    ignoreEvent() {
      return false;
    }
  }

  function insertEnclosingPair(editorView: EditorView, open: string, close: string) {
    const transaction = editorView.state.changeByRange((range) => {
      const selected = editorView.state.sliceDoc(range.from, range.to);
      if (!selected && open === close && editorView.state.sliceDoc(range.from, range.from + 1) === close) {
        return {
          changes: [],
          range: EditorSelection.cursor(range.from + 1)
        };
      }

      const insert = `${open}${selected}${close}`;
      const cursor = selected ? range.to + open.length + close.length : range.from + open.length;
      return {
        changes: {
          from: range.from,
          to: range.to,
          insert
        },
        range: EditorSelection.cursor(cursor)
      };
    });

    editorView.dispatch(transaction);
    return true;
  }

  function skipClosingPair(editorView: EditorView, close: string) {
    const transaction = editorView.state.changeByRange((range) => {
      if (!range.empty || editorView.state.sliceDoc(range.from, range.from + 1) !== close) {
        return {
          changes: {
            from: range.from,
            to: range.to,
            insert: close
          },
          range: EditorSelection.cursor(range.from + close.length)
        };
      }

      return {
        changes: [],
        range: EditorSelection.cursor(range.from + close.length)
      };
    });

    editorView.dispatch(transaction);
    return true;
  }
</script>

<div class="markdown-plus-editor" bind:this={host}></div>

<style>
  .markdown-plus-editor {
    min-height: 0;
    height: 100%;
    tab-size: 2;
  }
</style>
