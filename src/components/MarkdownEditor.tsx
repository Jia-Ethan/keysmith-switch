// SPDX-License-Identifier: MIT
// Portions adapted from CC Switch (c) 2025 Jason Young
// https://github.com/farion1231/cc-switch
// Added search keymap (Mod-f) and a taller editing surface.

import { useEffect, useRef } from "react";
import { markdown } from "@codemirror/lang-markdown";
import { searchKeymap, highlightSelectionMatches } from "@codemirror/search";
import { EditorState } from "@codemirror/state";
import { oneDark } from "@codemirror/theme-one-dark";
import { EditorView, placeholder as placeholderExt, keymap } from "@codemirror/view";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { lineNumbers, highlightActiveLine, highlightActiveLineGutter } from "@codemirror/view";

export function MarkdownEditor({
  value,
  onChange,
  placeholder = "",
  darkMode = false,
  readOnly = false,
  minHeight = "360px",
  className = "",
  ariaLabel,
}: {
  value: string;
  onChange?: (value: string) => void;
  placeholder?: string;
  darkMode?: boolean;
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
      "&": { height: "100%", minHeight, fontSize: "15px", lineHeight: "1.55" },
      ".cm-scroller": {
        overflow: "auto",
        fontFamily:
          "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, 'Liberation Mono', monospace",
      },
      ".cm-content": { padding: "12px 0", caretColor: "hsl(var(--foreground))" },
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
      baseTheme,
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
    if (darkMode) {
      extensions.push(oneDark);
    } else {
      extensions.push(
        EditorView.theme(
          {
            "&": { backgroundColor: "transparent" },
            ".cm-content": { color: "#1f2937" },
            ".cm-gutters": {
              backgroundColor: "#f9fafb",
              color: "#9ca3af",
              borderRight: "1px solid #e5e7eb",
            },
          },
          { dark: false },
        ),
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
  }, [ariaLabel, darkMode, readOnly, minHeight, placeholder]);

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
      className={`overflow-hidden rounded-xl border border-border ${className}`}
    />
  );
}
