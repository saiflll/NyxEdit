<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { getDefaultIconSvg } from "$lib/icon-overrides";

  let {
    filePath = $bindable(""),
    initialContent = "",
    onSave = (_path: string, _content: string) => {},
    onDirtyChange = (_dirty: boolean) => {},
  } = $props();

  let editorEl: HTMLDivElement;
  let editor: any = null;
  let currentContent = $state("");
  let isDirty = $state(false);
  let fileType = $state("");
  let langCompartment: any = null;

  const EXT_MAP: Record<string, string> = {
    ts: "typescript",
    tsx: "typescript",
    js: "javascript",
    jsx: "javascript",
    mjs: "javascript",
    cjs: "javascript",
    rs: "rust",
    py: "python",
    html: "html",
    htm: "html",
    css: "css",
    json: "json",
    md: "markdown",
    svelte: "html",
    vue: "html",
    go: "go",
    java: "java",
    kt: "kotlin",
    swift: "swift",
    c: "c",
    cpp: "cpp",
    cc: "cpp",
    cxx: "cpp",
    h: "c",
    hpp: "cpp",
    hxx: "cpp",
    ino: "cpp", // Arduino sketch = C++ dialect
    sh: "shell",
    bash: "shell",
    zsh: "shell",
    yml: "yaml",
    yaml: "yaml",
    toml: "toml",
    sql: "sql",
    xml: "xml",
    rb: "ruby",
    php: "php",
    dockerfile: "dockerfile",
  };

  function getLangFromExt(path: string): string {
    const ext = path.split(".").pop()?.toLowerCase() || "";
    return EXT_MAP[ext] || "";
  }

  async function saveFile() {
    if (!filePath) return;
    try {
      if (filePath.startsWith("sftp://")) {
        const rest = filePath.slice(7);
        const slashIdx = rest.indexOf("/");
        if (slashIdx > 0) {
          const sessionId = rest.substring(0, slashIdx);
          const remotePath = rest.substring(slashIdx);
          await invoke("sftp_write_file", {
            sessionId,
            remotePath,
            content: currentContent,
          });
          isDirty = false;
          onDirtyChange(false);
          onSave(filePath, currentContent);
          return;
        }
      }

      await invoke("fs_write_file", {
        path: filePath,
        content: currentContent,
      });
      isDirty = false;
      onDirtyChange(false);
      onSave(filePath, currentContent);
    } catch (e: any) {
      console.error("Save failed:", e);
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.key === "s") {
      e.preventDefault();
      saveFile();
    }
  }

  function getFileIconAndColor(path: string) {
    const name = path.split("\\").pop() || path.split("/").pop() || "";
    const ext = name.split(".").pop()?.toLowerCase() || "";
    const { svg, color } = getDefaultIconSvg(ext, name);
    return { name, ext, color, svg };
  }

  onMount(async () => {
    const codemirror = await import("codemirror");
    const {
      EditorView,
      keymap,
      lineNumbers,
      highlightActiveLine,
      highlightSpecialChars,
      drawSelection,
      rectangularSelection,
    } = await import("@codemirror/view");
    const { EditorState, Compartment } = await import("@codemirror/state");
    const { defaultKeymap, history, historyKeymap, indentWithTab } =
      await import("@codemirror/commands");
    const {
      HighlightStyle,
      syntaxHighlighting,
      bracketMatching,
      indentOnInput,
      foldGutter,
      foldKeymap,
    } = await import("@codemirror/language");
    const {
      autocompletion,
      completionKeymap,
      closeBrackets,
      closeBracketsKeymap,
    } = await import("@codemirror/autocomplete");
    const { searchKeymap, highlightSelectionMatches } = await import(
      "@codemirror/search"
    );
    const { lintKeymap } = await import("@codemirror/lint");
    const { tags } = await import("@lezer/highlight");

    langCompartment = new Compartment();

    // Premium theme-aware syntax highlight style using CSS variables for live-switching
    const myHighlightStyle = HighlightStyle.define([
      { tag: tags.keyword, color: "var(--accent-blue)", fontWeight: "bold" },
      {
        tag: tags.controlKeyword,
        color: "var(--accent-blue)",
        fontWeight: "bold",
      },
      { tag: tags.string, color: "var(--accent-green)" },
      { tag: tags.character, color: "var(--accent-green)" },
      { tag: tags.number, color: "var(--accent-yellow)" },
      { tag: tags.integer, color: "var(--accent-yellow)" },
      { tag: tags.float, color: "var(--accent-yellow)" },
      { tag: tags.bool, color: "var(--accent-yellow)" },
      { tag: tags.comment, color: "var(--text-muted)", fontStyle: "italic" },
      { tag: tags.variableName, color: "var(--text-primary)" },
      { tag: tags.propertyName, color: "var(--accent-cyan, #22d3ee)" },
      {
        tag: tags.function(tags.variableName),
        color: "var(--accent-indigo, #818cf8)",
      },
      {
        tag: tags.definition(tags.variableName),
        color: "var(--text-primary)",
        fontWeight: "600",
      },
      { tag: tags.operator, color: "var(--text-secondary)" },
      { tag: tags.className, color: "var(--accent-purple, #a78bfa)" },
      { tag: tags.tagName, color: "var(--accent-blue)" },
      { tag: tags.attributeName, color: "var(--accent-yellow)" },
    ]);

    const state = EditorState.create({
      doc: initialContent,
      extensions: [
        lineNumbers(),
        highlightActiveLine(),
        highlightSpecialChars(),
        drawSelection(),
        rectangularSelection(),
        bracketMatching(),
        indentOnInput(),
        foldGutter(),
        history(),
        autocompletion(),
        closeBrackets(),
        highlightSelectionMatches(),
        syntaxHighlighting(myHighlightStyle, { fallback: true }),
        keymap.of([
          ...defaultKeymap,
          ...historyKeymap,
          ...foldKeymap,
          ...completionKeymap,
          ...closeBracketsKeymap,
          ...searchKeymap,
          ...lintKeymap,
          indentWithTab,
        ]),
        EditorView.updateListener.of((update: any) => {
          if (update.docChanged) {
            currentContent = update.state.doc.toString();
            isDirty = true;
            onDirtyChange(true);
          }
        }),
        langCompartment.of([]),
        EditorView.theme({
          "&": {
            fontSize: "13px",
            backgroundColor: "transparent",
            color: "var(--text-primary)",
          },
          ".cm-scroller": { fontFamily: "var(--font-family, inherit)" },
          ".cm-gutters": {
            backgroundColor: "var(--bg-surface)",
            borderRight: "1px solid var(--border-primary)",
            color: "var(--text-muted)",
          },
          ".cm-activeLineGutter": { backgroundColor: "var(--bg-hover)" },
          ".cm-cursor": { borderLeftColor: "var(--accent-blue)" },
          ".cm-selectionBackground": {
            backgroundColor:
              "color-mix(in srgb, var(--accent-blue) 30%, transparent) !important",
          },
          "&.cm-focused .cm-selectionBackground": {
            backgroundColor:
              "color-mix(in srgb, var(--accent-blue) 40%, transparent) !important",
          },
          ".cm-matchingBracket": {
            backgroundColor:
              "color-mix(in srgb, var(--accent-blue) 20%, transparent)",
            borderBottom: "1px solid var(--accent-blue)",
          },
        }),
      ],
    });

    editor = new EditorView({
      state,
      parent: editorEl,
    });

    // Load initial content and language support
    if (initialContent) {
      editor.dispatch({
        changes: {
          from: 0,
          to: editor.state.doc.length,
          insert: initialContent,
        },
      });
      currentContent = initialContent;
      isDirty = false;
    }
    if (filePath) {
      updateLanguage(filePath);
    }
  });

  $effect(() => {
    // Whenever initialContent changes, sync with editor if needed
    if (editor && initialContent !== undefined) {
      const state = editor.state;
      const currentDoc = state.doc.toString();
      if (currentDoc !== initialContent) {
        editor.dispatch({
          changes: { from: 0, to: state.doc.length, insert: initialContent },
        });
        currentContent = initialContent;
        isDirty = false;
      }
    }
  });

  $effect(() => {
    // Whenever filePath changes, reconfigure language
    if (editor && filePath && langCompartment) {
      updateLanguage(filePath);
    }
  });

  async function updateLanguage(path: string) {
    // Special handling for filename-based detection (e.g. Dockerfile)
    const baseName = path.split(/[\\/]/).pop()?.toLowerCase() || "";
    if (baseName === "dockerfile" || baseName.startsWith("dockerfile.")) {
      fileType = "dockerfile";
    } else {
      fileType = getLangFromExt(path);
    }
    if (!fileType) {
      if (editor && langCompartment) {
        editor.dispatch({
          effects: langCompartment.reconfigure([]),
        });
      }
      return;
    }

    try {
      let extension: any = null;
      switch (fileType) {
        case "typescript":
        case "javascript": {
          const m = await import("@codemirror/lang-javascript");
          extension =
            fileType === "typescript"
              ? m.javascript({ typescript: true })
              : m.javascript();
          break;
        }
        case "rust": {
          const m = await import("@codemirror/lang-rust");
          extension = m.rust();
          break;
        }
        case "python": {
          const m = await import("@codemirror/lang-python");
          extension = m.python();
          break;
        }
        case "html": {
          const m = await import("@codemirror/lang-html");
          extension = m.html();
          break;
        }
        case "css": {
          const m = await import("@codemirror/lang-css");
          extension = m.css();
          break;
        }
        case "json": {
          const m = await import("@codemirror/lang-json");
          extension = m.json();
          break;
        }
        case "markdown": {
          const m = await import("@codemirror/lang-markdown");
          extension = m.markdown();
          break;
        }
        case "go": {
          const m = await import("@codemirror/lang-go");
          extension = m.go();
          break;
        }
        case "cpp":
        case "c": {
          const m = await import("@codemirror/lang-cpp");
          extension = m.cpp();
          break;
        }
        case "java": {
          const m = await import("@codemirror/lang-java");
          extension = m.java();
          break;
        }
        case "php": {
          const m = await import("@codemirror/lang-php");
          extension = m.php({ plain: true });
          break;
        }
        case "sql": {
          const m = await import("@codemirror/lang-sql");
          extension = m.sql();
          break;
        }
        case "xml": {
          const m = await import("@codemirror/lang-xml");
          extension = m.xml();
          break;
        }
        case "yaml": {
          const m = await import("@codemirror/lang-yaml");
          extension = m.yaml();
          break;
        }
        case "shell":
        case "kotlin":
        case "ruby":
        case "dockerfile":
        case "toml": {
          const { StreamLanguage } = await import("@codemirror/language");
          let parser: any;
          if (fileType === "shell") {
            const m = await import("@codemirror/legacy-modes/mode/shell");
            parser = m.shell;
          } else if (fileType === "kotlin") {
            const m = await import("@codemirror/legacy-modes/mode/clike");
            parser = m.kotlin;
          } else if (fileType === "ruby") {
            const m = await import("@codemirror/legacy-modes/mode/ruby");
            parser = m.ruby;
          } else if (fileType === "dockerfile") {
            const m = await import("@codemirror/legacy-modes/mode/dockerfile");
            parser = m.dockerFile;
          } else if (fileType === "toml") {
            const m = await import("@codemirror/legacy-modes/mode/toml");
            parser = m.toml;
          }
          if (parser) extension = StreamLanguage.define(parser);
          break;
        }
      }

      if (extension) {
        editor.dispatch({
          effects: langCompartment.reconfigure(extension),
        });
      }
    } catch (e) {
      console.warn("Language support not available for:", fileType);
    }
  }

  onDestroy(() => {
    if (editor) editor.destroy();
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="code-editor" onkeydown={handleKeydown}>
  <div class="editor-header">
    <div class="editor-title-area">
      {#if filePath}
        {@const fileInfo = getFileIconAndColor(filePath)}
        <span class="editor-file-icon">{@html fileInfo.svg}</span>
        <div class="editor-file-info">
          <span class="editor-file-name">{fileInfo.name}</span>
          <span class="editor-file-dir"
            >{filePath.substring(
              0,
              filePath.length - fileInfo.name.length,
            )}</span
          >
        </div>
      {:else}
        <span class="editor-file-icon">
          <svg
            width="15"
            height="15"
            viewBox="0 0 24 24"
            fill="none"
            stroke="var(--text-muted)"
            stroke-width="1.5"
          >
            <path
              d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5 2z"
            />
          </svg>
        </span>
        <span class="editor-file-name" style="color: var(--text-muted)"
          >Untitled File</span
        >
      {/if}
    </div>

    <div class="editor-actions">
      {#if isDirty}
        <span class="dirty-badge">● Unsaved Changes</span>
      {/if}
      <button
        onclick={saveFile}
        class="save-btn"
        class:dirty={isDirty}
        disabled={!isDirty}
      >
        <svg
          width="12"
          height="12"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <path
            d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z"
          />
          <polyline points="17 21 17 13 7 13 7 21" />
          <polyline points="7 3 7 8 15 8" />
        </svg>
        <span>Save (Ctrl+S)</span>
      </button>
    </div>
  </div>
  <div bind:this={editorEl} class="editor-instance"></div>
</div>

<style>
  .code-editor {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--glass-bg, var(--bg-secondary));
    backdrop-filter: blur(var(--glass-blur, 12px));
    -webkit-backdrop-filter: blur(var(--glass-blur, 12px));
  }
  .editor-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 42px;
    padding: 0 14px;
    background: var(--glass-bg, var(--bg-secondary));
    border-bottom: 1px solid var(--glass-border, var(--border-primary));
    flex-shrink: 0;
    backdrop-filter: blur(var(--glass-blur, 8px));
    -webkit-backdrop-filter: blur(var(--glass-blur, 8px));
  }
  .editor-title-area {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    min-width: 0;
  }
  .editor-file-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }
  .editor-file-info {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .editor-file-name {
    font-size: var(--font-size);
    font-weight: 600;
    color: var(--text-primary);
    line-height: 1.2;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .editor-file-dir {
    font-size: var(--fs-10);
    color: var(--text-muted);
    font-family: monospace;
    line-height: 1.1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    margin-top: 1px;
    opacity: 0.7;
    direction: rtl;
    text-align: left;
  }
  .editor-actions {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-shrink: 0;
  }
  .dirty-badge {
    font-size: var(--fs-9);
    background: rgba(251, 191, 36, 0.08);
    color: var(--accent-yellow);
    border: 1px solid rgba(251, 191, 36, 0.25);
    border-radius: 4px;
    padding: 1.5px 6px;
    font-weight: 500;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    animation: pulse 2s infinite ease-in-out;
  }
  .save-btn {
    display: flex;
    align-items: center;
    gap: 5px;
    background: transparent;
    border: 1.5px solid var(--accent-blue);
    border-radius: 6px;
    padding: 4px 10px;
    font-size: var(--fs-11);
    color: var(--accent-blue);
    font-weight: 600;
    cursor: pointer;
    transition: all 0.15s ease-in-out;
  }
  .save-btn svg {
    flex-shrink: 0;
  }
  .save-btn:hover:not(:disabled) {
    background: var(--accent-blue);
    color: #fff;
    box-shadow: 0 0 8px rgba(129, 140, 248, 0.35);
  }
  .save-btn.dirty {
    background: var(--accent-blue);
    color: var(--bg-primary);
    border-color: var(--accent-blue);
  }
  .save-btn.dirty:hover {
    filter: brightness(1.15);
    box-shadow: 0 0 10px rgba(129, 140, 248, 0.5);
  }
  .save-btn:disabled {
    opacity: 0.35;
    border-color: var(--border-primary);
    color: var(--text-muted);
    cursor: not-allowed;
  }
  .editor-instance {
    flex: 1;
    overflow: hidden;
  }
  .editor-instance :global(.cm-editor) {
    height: 100%;
  }
  .editor-instance :global(.cm-scroller) {
    overflow: auto;
  }

  /* Scrollbar styles for the editor scroller */
  .editor-instance :global(.cm-scroller::-webkit-scrollbar) {
    width: 6px;
    height: 6px;
  }
  .editor-instance :global(.cm-scroller::-webkit-scrollbar-track) {
    background: transparent;
  }
  .editor-instance :global(.cm-scroller::-webkit-scrollbar-thumb) {
    background: var(--border-primary);
    border-radius: 3px;
  }
  .editor-instance :global(.cm-scroller::-webkit-scrollbar-thumb:hover) {
    background: var(--bg-hover);
  }

  @keyframes pulse {
    0%,
    100% {
      opacity: 0.85;
    }
    50% {
      opacity: 0.5;
    }
  }
</style>
