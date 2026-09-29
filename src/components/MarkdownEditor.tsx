// SPDX-License-Identifier: MIT
// Portions adapted from CC Switch (c) 2025 Jason Young
// https://github.com/farion1231/cc-switch
// Added search keymap (Mod-f), a taller editing surface, and a theme drawn
// from the app's CSS tokens so light, dark and the agent accent all follow.

import { useEffect, useRef } from "react";
import { markdown } from "@codemirror/lang-markdown";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { searchKeymap, highlightSelectionMatches } from "@codemirror/search";
import { EditorState } from "@codemirror/state";
import { EditorView, placeholder as placeholderExt, keymap } from "@codemirror/view";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { lineNumbers, highlightActiveLine, highlightActiveLineGutter } from "@codemirror/view";
import { tags } from "@lezer/highlight";

const tokenTheme = EditorView.theme({
  "&": { backgroundColor: "transparent", color: "hsl(var(--foreground))" },
  ".cm-content": { caretColor: "rgb(var(--primary))" },
  ".cm-cursor, .cm-dropCursor": { borderLeftColor: "rgb(var(--primary))", borderLeftWidth: "2px" },
  ".cm-gutters": {
    backgroundColor: "transparent",
    color: "hsl(var(--muted-foreground) / 0.55)",
    border: "none",
  },
  ".cm-lineNumbers .cm-gutterElement": { padding: "0 10px 0 14px", minWidth: "36px" },
  ".cm-activeLine": { backgroundColor: "rgb(var(--primary) / 0.05)" },
  ".cm-activeLineGutter": { backgroundColor: "transparent", color: "rgb(var(--primary))" },
  "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection": {
    backgroundColor: "rgb(var(--primary) / 0.2) !important",
  },
  ".cm-selectionMatch": { backgroundColor: "rgb(var(--primary) / 0.12)" },
  ".cm-placeholder": { color: "hsl(var(--muted-foreground) / 0.8)" },
  ".cm-panels": {
    backgroundColor: "hsl(var(--card))",
    color: "hsl(var(--foreground))",
    borderColor: "hsl(var(--border))",
  },
  ".cm-panels.cm-panels-top": { borderBottom: "1px solid hsl(var(--border))" },
  ".cm-panel.cm-search input, .cm-panel.cm-search button": {
    fontFamily: "inherit",
    fontSize: "12px",
    borderRadius: "6px",
  },
  ".cm-textfield": {
    backgroundColor: "hsl(var(--background))",
    border: "1px solid hsl(var(--input))",
    color: "hsl(var(--foreground))",
  },
  ".cm-button": {
    backgroundImage: "none",
    backgroundColor: "hsl(var(--muted))",
    border: "1px solid hsl(var(--border))",
    color: "hsl(var(--foreground))",
  },
});

const markdownHighlight = HighlightStyle.define([
  { tag: tags.heading1, fontWeight: "700", fontSize: "1.18em", color: "rgb(var(--primary))" },
  { tag: tags.heading2, fontWeight: "700", fontSize: "1.08em", color: "rgb(var(--primary))" },
  { tag: [tags.heading3, tags.heading4, tags.heading5, tags.heading6], fontWeight: "650", color: "rgb(var(--primary))" },
  { tag: tags.strong, fontWeight: "700" },
  { tag: tags.emphasis, fontStyle: "italic" },
  { tag: tags.strikethrough, textDecoration: "line-through" },
  { tag: [tags.link, tags.url], color: "rgb(var(--primary))", textDecoration: "underline" },
  { tag: tags.monospace, color: "hsl(var(--foreground))", backgroundColor: "hsl(var(--muted) / 0.8)", borderRadius: "4px" },
  { tag: tags.quote, color: "hsl(var(--muted-foreground))", fontStyle: "italic" },
  { tag: [tags.list, tags.processingInstruction, tags.meta, tags.contentSeparator], color: "hsl(var(--muted-foreground))" },
]);

export function MarkdownEditor({
  value,
  onChange,
  placeholder = "",
  readOnly = false,
  minHeight = "360px",
  className = "",
  ariaLabel,
}: {
  value: string;
  onChange?: (value: string) => void;
  placeholder?: string;
  readOnly?: boolean;
  minHeight?: string;
  className?: string;
  ariaLabel?: string;
}) {
  const editorRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;

  useEffect(() => {
    if (!editorRef.current) return;
    const baseTheme = EditorView.baseTheme({
      "&": { height: "100%", minHeight, fontSize: "13.5px", lineHeight: "1.65" },
      ".cm-scroller": {
        overflow: "auto",
        fontFamily:
          "ui-monospace, 'SF Mono', SFMono-Regular, 'JetBrains Mono', Menlo, Consolas, 'Liberation Mono', monospace",
      },
      ".cm-content": { padding: "14px 0" },
      ".cm-line": { padding: "0 16px 0 4px" },
      "&.cm-focused": { outline: "none" },
    });
    const extensions = [
      lineNumbers(),
      highlightActiveLine(),
      highlightActiveLineGutter(),
      history(),
      highlightSelectionMatches(),
      keymap.of([...defaultKeymap, ...historyKeymap, ...searchKeymap, indentWithTab]),
      markdown(),
      syntaxHighlighting(markdownHighlight),
      baseTheme,
      tokenTheme,
      EditorView.lineWrapping,
      EditorState.readOnly.of(readOnly),
    ];
    if (ariaLabel) {
      extensions.push(EditorView.contentAttributes.of({ "aria-label": ariaLabel }));
    }
    if (!readOnly) {
      extensions.push(
        placeholderExt(placeholder),
        EditorView.updateListener.of((update) => {
          if (update.docChanged && onChangeRef.current) {
            onChangeRef.current(update.state.doc.toString());
          }
        }),
      );
    }
    const view = new EditorView({
      state: EditorState.create({ doc: value, extensions }),
      parent: editorRef.current,
    });
    viewRef.current = view;
    return () => {
      view.destroy();
      viewRef.current = null;
    };
  }, [ariaLabel, readOnly, minHeight, placeholder]);

  useEffect(() => {
    const view = viewRef.current;
    if (view && view.state.doc.toString() !== value) {
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: value },
      });
    }
  }, [value]);

  return (
    <div
      ref={editorRef}
      data-testid="markdown-editor"
      className={`overflow-hidden rounded-xl border border-border bg-card transition-[border-color,box-shadow] focus-within:border-primary/50 focus-within:ring-[3px] focus-within:ring-primary/10 ${className}`}
    />
  );
}
