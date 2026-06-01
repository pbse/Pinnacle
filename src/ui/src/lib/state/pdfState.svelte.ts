import { invoke } from "@tauri-apps/api/core";
import { appState } from "./appState.svelte";
import { historyState } from "./historyState.svelte";
import { db, type BookmarkRecord, type VersionRecord, type NoteRecord } from "./db";

export type ToolId = "merge" | "split" | "extract" | "annotate" | "signature" | "security" | "organize" | "compare" | "library" | "forms" | "versions" | "watermark" | "notepad" | "peek" | "settings" | "insights" | "compress";
export type SelectionTarget = "parse" | "split" | "rotate" | "delete" | "annotate" | "signature" | "security" | "extract" | "crypto" | "organize";
export type LoaderStage = "converting" | "scanning" | "indexing" | "complete" | "error";
export type ActiveLoader = {
  filename: string;
  stage: LoaderStage;
  progress: number;
  detail?: string;
  outputPath?: string;
};

const PDF_EXTENSIONS = [".pdf"];
const IMAGE_EXTENSIONS = [".png", ".jpg", ".jpeg"];
const OFFICE_EXTENSIONS = [".docx", ".xlsx"];
const SUPPORTED_DROP_EXTENSIONS = [...PDF_EXTENSIONS, ...IMAGE_EXTENSIONS, ...OFFICE_EXTENSIONS];

function filenameOf(path: string) {
  return path.split(/[/\\]/).pop() || path;
}

function extensionOf(path: string) {
  const name = filenameOf(path).toLowerCase();
  const index = name.lastIndexOf(".");
  return index >= 0 ? name.slice(index) : "";
}

function convertedPdfPath(path: string) {
  const separatorIndex = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  const dir = separatorIndex >= 0 ? path.slice(0, separatorIndex + 1) : "";
  const base = filenameOf(path).replace(/\.[^.]+$/, "");
  const safeBase = base.replace(/[^\w.-]+/g, "_") || "document";
  return `${dir}${safeBase}.pinnacle-${Date.now()}.pdf`;
}

function parseColorHex(hex: string): [number, number, number] | null {
  const match = /^#?([a-fA-F0-9]{6})$/.exec(hex.trim());
  if (!match) return null;
  const intVal = parseInt(match[1], 16);
  return [((intVal >> 16) & 255) / 255, ((intVal >> 8) & 255) / 255, (intVal & 255) / 255];
}

function layoutToFullText(layout: any, fallback: string) {
  if (!layout?.pages || !Array.isArray(layout.pages)) return fallback;
  return layout.pages
    .map((page: any) => {
      if (typeof page.text === "string" && page.text.trim()) return page.text;
      const items = [...(page.blocks || []), ...(page.lines || []), ...(page.words || [])];
      return items
        .map((item: any) => item.text || item.content || item.value || "")
        .filter(Boolean)
        .join(" ");
    })
    .filter(Boolean)
    .join("\n\n");
}

async function convertDroppedFileToPdf(path: string) {
  const ext = extensionOf(path);
  const outputPath = convertedPdfPath(path);

  if (IMAGE_EXTENSIONS.includes(ext)) {
    await invoke("images_to_pdf", { imagePaths: [path], outputPath });
    return outputPath;
  }

  if (OFFICE_EXTENSIONS.includes(ext)) {
    await invoke("office_to_pdf", { path, outputPath });
    return outputPath;
  }

  return path;
}

const state = $state({
  activeTool: "insights" as ToolId,
  highlightedSnippet: null as string | null,
  bookmarks: [] as BookmarkRecord[],
  versions: [] as VersionRecord[],
  notes: [] as NoteRecord[],
  openTabs: [] as string[],
  selectedParseFile: null as string | null,
  selectedSplitFile: null as string | null,
  selectedRotateFile: null as string | null,
  selectedDeleteFile: null as string | null,
  selectedAnnotateFile: null as string | null,
  selectedSignatureFile: null as string | null,
  selectedCryptoFile: null as string | null,
  selectedExtractFile: null as string | null,
  selectedMergeFiles: [] as string[],
  viewerFilePath: "",
  viewerPageNumber: 1,
  viewerMode: "view" as "rect" | "points" | "view",
  viewerTarget: null as SelectionTarget | null,
  ocrTrigger: 0,
  currentLayoutData: null as any,
  ocrSplitMode: false,
  ocrTextEdited: "",
  activeLoader: null as ActiveLoader | null,
  splitPagesInput: "",
  rotatePagesInput: "",
  deletePagesInput: "",
  annotationRectInput: "",
  annotationStrokes: [] as [number, number][][],
  annotationType: "highlight" as "highlight" | "underline" | "strikeout" | "note" | "square" | "circle" | "ink",
  annotationText: "",
  annotationColor: "#facc15",
  signatureRectInput: "",
  signatureColor: "#0f172a",
  signatureWidth: 2 as number | null,
  signatureStrokes: [] as [number, number][][],
  signCertPath: "",
  signCertPassword: "",
  rememberPassword: false,
  signRectInput: "",
  signReason: "",
  signLocation: "",
  signContact: "",
  history: [] as any[],
  redoStack: [] as any[],
  comparisonFile2: null as string | null,
  scannedFields: [] as { name: string, field_type: string, value: string, page: number, rect: number[] }[],
  formFieldsToCreate: [] as { name: string, field_type: string, page: number, rect: number[] }[],
  pendingChanges: [] as {
    id: string;
    target: "annotate" | "signature";
    page: number;
    rect: number[] | null;
    strokes: [number, number][][] | null;
    type: string;
    text?: string;
    color: string;
    width?: number;
  }[],
  activeStamp: null as {
    strokes: [number, number][][];
    aspectRatio: number;
    width: number;
    height: number;
  } | null,
  showSignPad: false,

  addPendingChange(change: any) {
    state.pendingChanges = [...state.pendingChanges, change];
  },

  removePendingChange(id: string) {
    state.pendingChanges = state.pendingChanges.filter(c => c.id !== id);
  },

  clearPendingChanges() {
    state.pendingChanges = [];
  },

  async commitAllPending(makePermanent: boolean) {
    if (state.pendingChanges.length === 0) {
      appState.showStatus("No pending changes to apply.", true);
      return;
    }
    
    const file = state.viewerFilePath;
    if (!file) {
      appState.showStatus("No document open.", true);
      return;
    }
    
    const filename = file.split(/[/\\]/).pop() || "document.pdf";
    const defaultPath = filename.replace(/\.[^.]+$/, "") + "_edited.pdf";
    const outputPath = await invoke<string | null>("save_file_dialog", { defaultPath });
    if (!outputPath) return;
    
    appState.startLoading("Applying all changes...");
    
    // Generate a unique session key for temp files to prevent any name collisions
    const tempSessionId = Date.now() + Math.random().toString(36).substring(2, 6);
    const tempFiles: string[] = [];
    
    try {
      let currentInput = file;
      
      for (let i = 0; i < state.pendingChanges.length; i++) {
        const change = state.pendingChanges[i];
        const isLast = i === state.pendingChanges.length - 1;
        const currentOutput = isLast 
          ? (makePermanent ? `${outputPath}.${tempSessionId}.unflattened.pdf` : outputPath) 
          : `${outputPath}.${tempSessionId}.tmp_${i}.pdf`;
        
        if (!isLast || makePermanent) {
          tempFiles.push(currentOutput);
        }
        
        const colorArray = parseColorHex(change.color);
        
        if (change.target === "signature") {
          await invoke("add_signature_visual", {
            path: currentInput,
            page: change.page,
            rect: change.rect,
            strokes: change.strokes,
            color: colorArray,
            width: change.width ?? 2.0,
            outputPath: currentOutput
          });
        } else {
          if (change.type === "ink") {
            await invoke("add_ink_annotation", {
              path: currentInput,
              page: change.page,
              gestures: change.strokes,
              color: colorArray,
              width: change.width ?? 2.0,
              outputPath: currentOutput
            });
          } else {
            await invoke("add_annotation", {
              path: currentInput,
              page: change.page,
              rect: change.rect,
              kind: change.type,
              contents: change.text || null,
              color: colorArray,
              outputPath: currentOutput
            });
          }
        }
        
        currentInput = currentOutput;
      }
      
      if (makePermanent) {
        appState.startLoading("Flattening and locking changes...");
        const unflattened = `${outputPath}.${tempSessionId}.unflattened.pdf`;
        await invoke("flatten_annotations", {
          path: unflattened,
          outputPath: outputPath
        });
      }
      
      state.clearPendingChanges();
      
      appState.showStatus(`Successfully applied all changes ${makePermanent ? '(Flattened)' : ''}.`, false, outputPath);
      await invoke("shell_open", { filePath: outputPath });
      
      state.viewerFilePath = outputPath;
      
    } catch (err) {
      appState.showStatus(`Error applying batch changes: ${err}`, true);
      console.error(err);
    } finally {
      // Clean up all temporary files created during this session
      for (const tempPath of tempFiles) {
        try {
          await invoke("delete_file", { path: tempPath });
        } catch (e) {
          console.warn(`Could not delete temp file at ${tempPath}:`, e);
        }
      }
    }
  },

  switchTool(id: ToolId) {
    if (id === state.activeTool) return;
    state.activeTool = id;
    
    if (state.viewerFilePath) {
      if (id === 'split') state.selectedSplitFile = state.viewerFilePath;
      if (id === 'annotate') {
        state.selectedAnnotateFile = state.viewerFilePath;
        state.viewerMode = 'rect';
        state.viewerTarget = 'annotate';
      }
      if (id === 'signature') {
        state.selectedSignatureFile = state.viewerFilePath;
        state.viewerMode = 'view';
        state.viewerTarget = 'signature';
      }
      if (id === 'security') {
        state.selectedCryptoFile = state.viewerFilePath;
        state.viewerMode = 'rect';
        state.viewerTarget = 'security';
      }
      if (id === 'extract') state.selectedExtractFile = state.viewerFilePath;
      if (id === 'compress') state.selectedCryptoFile = state.viewerFilePath;
      if (id === 'organize') {
        state.selectedRotateFile = state.viewerFilePath;
        state.selectedDeleteFile = state.viewerFilePath;
      }
    } else {
      if (id === 'split' && state.selectedSplitFile) state.viewerFilePath = state.selectedSplitFile;
      if (id === 'annotate' && state.selectedAnnotateFile) {
        state.viewerFilePath = state.selectedAnnotateFile;
        state.viewerMode = 'rect';
        state.viewerTarget = 'annotate';
      }
      if (id === 'signature' && state.selectedSignatureFile) {
        state.viewerFilePath = state.selectedSignatureFile;
        state.viewerMode = 'view';
        state.viewerTarget = 'signature';
      }
      if (id === 'security' && state.selectedCryptoFile) {
        state.viewerFilePath = state.selectedCryptoFile;
        state.viewerMode = 'rect';
        state.viewerTarget = 'security';
      }
      if (id === 'extract' && state.selectedExtractFile) state.viewerFilePath = state.selectedExtractFile;
      if (id === 'organize' && state.selectedRotateFile) state.viewerFilePath = state.selectedRotateFile;
    }

    state.signatureStrokes = [];
    state.annotationStrokes = [];
    state.annotationRectInput = "";
    state.signatureRectInput = "";
    state.signRectInput = "";
    
    if (id === 'annotate' && state.annotationType === 'ink') {
      state.viewerMode = 'points';
    } else if (!['annotate', 'security'].includes(id)) {
      state.viewerMode = "view";
    }
  },

  openTab(path: string) {
    if (!path) return;
    if (state.viewerFilePath !== path) {
      state.scannedFields = [];
      state.formFieldsToCreate = [];
    }
    if (!state.openTabs.includes(path)) {
      state.openTabs = [...state.openTabs, path];
    }
    state.viewerFilePath = path;
    historyState.addFile(path);
  },

  closeTab(path: string) {
    state.openTabs = state.openTabs.filter(t => t !== path);
    if (state.viewerFilePath === path) {
      state.viewerFilePath = state.openTabs[state.openTabs.length - 1] || "";
    }
  },

  async saveReadingProgress(path: string, page: number, total: number) {
    if (!path) return;
    const item = await db.documents.where('path').equals(path).first();
    if (item?.id) {
      await db.documents.update(item.id, { lastPage: page, totalPages: total });
    }
  },

  async getReadingProgress(path: string): Promise<number> {
    if (!path) return 1;
    const item = await db.documents.where('path').equals(path).first();
    return item?.lastPage || 1;
  },

  async addBookmark(path: string, page: number, label: string) {
    if (!path) return;
    await db.bookmarks.add({
      docPath: path,
      pageNumber: page,
      label,
      timestamp: Date.now()
    });
    await state.loadBookmarks(path);
  },

  async deleteBookmark(id: number) {
    await db.bookmarks.delete(id);
    if (state.viewerFilePath) await state.loadBookmarks(state.viewerFilePath);
  },

  async loadNotes() {
    state.notes = await db.notes.orderBy('timestamp').reverse().toArray();
  },

  async addNote(content: string, citation?: { docPath: string, pageNumber: number, text: string }) {
    await db.notes.add({
      content,
      timestamp: Date.now(),
      tags: [],
      citations: citation ? [citation] : []
    });
    await state.loadNotes();
    appState.showStatus("Note saved to Research Notepad.", false);
  },

  async deleteNote(id: number) {
    await db.notes.delete(id);
    await state.loadNotes();
  },

  async loadBookmarks(path: string) {
    state.bookmarks = await db.bookmarks.where('docPath').equals(path).sortBy('timestamp');
  },

  async loadVersions(path: string) {
    state.versions = await db.versions.where('docPath').equals(path).sortBy('timestamp');
  },

  async saveSnapshot(path: string, label: string) {
    const bytes = await invoke<Uint8Array>("read_file_bytes", { path });
    await db.versions.add({
      docPath: path,
      label,
      timestamp: Date.now(),
      data: bytes
    });
    await state.loadVersions(path);
  },

  async restoreSnapshot(version: VersionRecord, outputPath: string) {
    await invoke("write_file_bytes", { path: outputPath, bytes: version.data });
    appState.showStatus(`Restored to: ${version.label}`, false, outputPath);
  },

  async saveSession(name: string) {
    const session = {
      name,
      tabs: state.openTabs,
      activeTab: state.viewerFilePath,
      timestamp: Date.now()
    };
    localStorage.setItem(`pdf_session_${name}`, JSON.stringify(session));
    appState.showStatus(`Session "${name}" saved.`, false);
  },

  async loadSession(name: string) {
    const saved = localStorage.getItem(`pdf_session_${name}`);
    if (saved) {
      const session = JSON.parse(saved);
      state.openTabs = session.tabs;
      state.viewerFilePath = session.activeTab;
      appState.showStatus(`Restored session: ${name}`, false);
    }
  },
  
  pushHistory(hState: any) {
    state.history.push(JSON.parse(JSON.stringify(hState)));
    state.redoStack = []; 
  },

  undo() {
    if (state.history.length > 0) {
      const lastState = state.history.pop();
      state.redoStack.push(JSON.parse(JSON.stringify({
        signatureStrokes: state.signatureStrokes,
        annotationStrokes: state.annotationStrokes,
        annotationRectInput: state.annotationRectInput,
        signatureRectInput: state.signatureRectInput
      })));
      state.signatureStrokes = lastState.signatureStrokes;
      state.annotationStrokes = lastState.annotationStrokes;
      state.annotationRectInput = lastState.annotationRectInput;
      state.signatureRectInput = lastState.signatureRectInput;
    }
  },

  redo() {
    if (state.redoStack.length > 0) {
      const nextState = state.redoStack.pop();
      state.history.push(JSON.parse(JSON.stringify({
        signatureStrokes: state.signatureStrokes,
        annotationStrokes: state.annotationStrokes,
        annotationRectInput: state.annotationRectInput,
        signatureRectInput: state.signatureRectInput
      })));
      state.signatureStrokes = nextState.signatureStrokes;
      state.annotationStrokes = nextState.annotationStrokes;
      state.annotationRectInput = nextState.annotationRectInput;
      state.signatureRectInput = nextState.signatureRectInput;
    }
  },

  setFileForTarget(target: SelectionTarget, path: string) {
    if (!path) return;
    
    switch (target) {
      case "split": state.selectedSplitFile = path; break;
      case "rotate": state.selectedRotateFile = path; break;
      case "delete": state.selectedDeleteFile = path; break;
      case "organize": 
        state.selectedRotateFile = path;
        state.selectedDeleteFile = path;
        break;
      case "annotate": state.selectedAnnotateFile = path; break;
      case "signature": state.selectedSignatureFile = path; break;
      case "security": 
      case "crypto": state.selectedCryptoFile = path; break;
      case "extract": state.selectedExtractFile = path; break;
    }

    state.openTab(path);
  },

  async selectFile(target: SelectionTarget) {
    const result = await invoke<string[]>("open_file_dialog", { multiple: false });
    if (result && result.length > 0) {
      state.setFileForTarget(target, result[0]);
    }
  },

  async openNewDocument() {
    const result = await invoke<string[]>("open_file_dialog", { multiple: false });
    if (result && result.length > 0) {
      state.openTab(result[0]);
      state.activeTool = 'insights';
    }
  },

  async handleDroppedFiles(paths: string[]) {
    const validPaths = paths.filter(p => SUPPORTED_DROP_EXTENSIONS.includes(extensionOf(p)));
    
    if (validPaths.length === 0) {
      appState.showStatus("Unsupported file format.", true);
      return;
    }
    
    for (const [index, path] of validPaths.entries()) {
      const filename = filenameOf(path);
      const ext = extensionOf(path);
      const isPdf = PDF_EXTENSIONS.includes(ext);
      
      state.activeLoader = {
        filename,
        stage: isPdf ? "scanning" : "converting",
        progress: 10,
        detail: `${index + 1} of ${validPaths.length}`
      };
      
      try {
        const isTest = typeof window === "undefined" || !(window as any).__TAURI_INTERNALS__;
        
        if (!isTest) {
          let importPath = path;
          let convertedFrom: string | null = null;

          if (!isPdf) {
            state.activeLoader.progress = 28;
            state.activeLoader.detail = IMAGE_EXTENSIONS.includes(ext)
              ? "Converting image to PDF"
              : "Converting office document to PDF";
            importPath = await convertDroppedFileToPdf(path);
            convertedFrom = filename;
            state.activeLoader.filename = filenameOf(importPath);
            state.activeLoader.outputPath = importPath;
            state.activeLoader.detail = `Converted from ${convertedFrom}`;
          }
          
          state.activeLoader.stage = "scanning";
          state.activeLoader.progress = 60;
          state.activeLoader.detail = "Scanning spatial layout with LiteParse";
          
          const existingDoc = await db.documents.where('path').equals(importPath).first();
          let jsonStr = existingDoc?.layoutJson || "";
          if (!jsonStr) {
            jsonStr = await invoke<string>("pdf_to_layout_json", { path: importPath });
          }
          const parsedLayout = JSON.parse(jsonStr);
          state.currentLayoutData = parsedLayout;
          
          state.activeLoader.stage = "indexing";
          state.activeLoader.progress = 85;
          state.activeLoader.detail = "Writing layout and text to local library";
          
          await historyState.addFile(importPath, {
            layoutJson: jsonStr,
            fullText: layoutToFullText(parsedLayout, jsonStr)
          });

          if (state.viewerFilePath !== importPath) {
            state.scannedFields = [];
            state.formFieldsToCreate = [];
          }
          if (!state.openTabs.includes(importPath)) {
            state.openTabs = [...state.openTabs, importPath];
          }
          state.viewerFilePath = importPath;
          state.selectedExtractFile = importPath;
          state.activeTool = "insights";
          
          state.activeLoader.stage = "complete";
          state.activeLoader.progress = 100;
          state.activeLoader.detail = convertedFrom
            ? `Ready: ${filenameOf(importPath)}`
            : "Indexed and ready";
          
          appState.showStatus(`Imported & Indexed: ${filenameOf(importPath)}`, false);
          
          setTimeout(() => {
            if (state.activeLoader?.outputPath === importPath || state.activeLoader?.filename === filenameOf(importPath)) {
              state.activeLoader = null;
            }
          }, 2000);
        } else {
          state.setFileForTarget('split', path);
          state.activeLoader = null;
        }
      } catch (e: any) {
        console.error("Import failed:", e);
        if (state.activeLoader) {
          state.activeLoader.stage = "error";
          state.activeLoader.progress = 100;
          state.activeLoader.detail = e?.message || e?.toString?.() || "Import failed";
        }
        appState.showStatus(`Failed to import ${filename}: ${e.toString()}`, true);
        setTimeout(() => {
          state.activeLoader = null;
        }, 4000);
      }
    }
  }
});

export const pdfState = state;

if (typeof window !== "undefined") {
  (window as any).__PINNACLE_PDF_STATE__ = pdfState;
}
