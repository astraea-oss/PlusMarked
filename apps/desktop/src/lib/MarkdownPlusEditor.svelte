<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands';
  import { markdown } from '@codemirror/lang-markdown';
  import { HighlightStyle, syntaxHighlighting } from '@codemirror/language';
  import { EditorState } from '@codemirror/state';
  import { tags } from '@lezer/highlight';
  import {
    Decoration,
    EditorView,
    ViewPlugin,
    keymap,
    placeholder,
    type DecorationSet,
    type ViewUpdate
  } from '@codemirror/view';

  export let value = '';
  export let ariaLabel = 'MarkdownPlus editor';
  export let onChange: (value: string) => void = () => {};

  let host: HTMLDivElement;
  let view: EditorView | null = null;
  let editorValue = '';

  const markdownPlusHighlight = HighlightStyle.define([
    { tag: tags.heading1, color: '#f0f4f8', fontSize: '1.22em', fontWeight: '760' },
    { tag: tags.heading2, color: '#f0f4f8', fontSize: '1.12em', fontWeight: '740' },
    { tag: tags.heading3, color: '#e6edf3', fontSize: '1.04em', fontWeight: '720' },
    { tag: tags.heading, color: '#e6edf3', fontWeight: '700' },
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
      '.cm-mdp-heading-1': {
        color: '#f0f4f8',
        fontSize: '1.22em',
        fontWeight: '760'
      },
      '.cm-mdp-heading-2': {
        color: '#f0f4f8',
        fontSize: '1.12em',
        fontWeight: '740'
      },
      '.cm-mdp-heading-3': {
        color: '#e6edf3',
        fontSize: '1.04em',
        fontWeight: '720'
      },
      '.cm-mdp-divider-line::after': {
        content: '""',
        display: 'inline-block',
        width: 'min(22rem, 64%)',
        marginLeft: '0.7rem',
        verticalAlign: 'middle',
        borderTop: '2px solid #5d6b7c'
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
        if (update.docChanged || update.viewportChanged) {
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
          EditorView.lineWrapping,
          markdownPlusTheme,
          syntaxHighlighting(markdownPlusHighlight),
          markdownPlusLinePlugin,
          keymap.of([indentWithTab, ...defaultKeymap, ...historyKeymap]),
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

  onDestroy(() => {
    view?.destroy();
  });

  function lineDecorations(editorView: EditorView): DecorationSet {
    const decorations = [];

    for (const { from, to } of editorView.visibleRanges) {
      let position = from;
      while (position <= to) {
        const line = editorView.state.doc.lineAt(position);
        if (/^[ \t]*-{3,}[ \t]*$/.test(line.text)) {
          decorations.push(Decoration.line({ class: 'cm-mdp-divider-line' }).range(line.from));
        } else {
          const heading = line.text.match(/^\s*(#{1,3})\s+\S/);
          if (heading) {
            decorations.push(Decoration.line({ class: `cm-mdp-heading-${heading[1].length}` }).range(line.from));
          }
        }
        position = line.to + 1;
      }
    }

    return Decoration.set(decorations, true);
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
