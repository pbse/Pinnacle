<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { fly, fade, scale } from "svelte/transition";

  // State & Runes
  import { appState } from "$lib/state/appState.svelte";
  import { pdfState } from "$lib/state/pdfState.svelte";
  import { chatState } from "$lib/state/chatState.svelte";
  import { historyState } from "$lib/state/historyState.svelte";
  import { db } from "$lib/state/db";

  // Components
  import SidebarNav from "$lib/components/SidebarNav.svelte";
  import StatusDisplay from "$lib/components/StatusDisplay.svelte";
  import PdfViewer from "$lib/components/PdfViewer.svelte";
  import MergePane from "$lib/components/MergePane.svelte";
  import SplitPane from "$lib/components/SplitPane.svelte";
  import ExtractPane from "$lib/components/ExtractPane.svelte";
  import AnnotatePane from "$lib/components/AnnotatePane.svelte";
  import SignaturePane from "$lib/components/SignaturePane.svelte";
  import SecurityPane from "$lib/components/SecurityPane.svelte";
  import OrganizePane from "$lib/components/OrganizePane.svelte";
  import SettingsPane from "$lib/components/SettingsPane.svelte";
  import ComparePane from "$lib/components/ComparePane.svelte";
  import LibraryPane from "$lib/components/LibraryPane.svelte";
  import FormsPane from "$lib/components/FormsPane.svelte";
  import CompressionPane from "$lib/components/CompressionPane.svelte";
  import VersionsPane from "$lib/components/VersionsPane.svelte";
  import WatermarkPane from "$lib/components/WatermarkPane.svelte";
  import NotepadPane from "$lib/components/NotepadPane.svelte";
  import InsightPane from "$lib/components/InsightPane.svelte";
  import CommandPalette from "$lib/components/CommandPalette.svelte";
  import ShortcutsModal from "$lib/components/ShortcutsModal.svelte";
  import OnboardingTour from "$lib/components/OnboardingTour.svelte";
  import SignaturePadModal from "$lib/components/SignaturePadModal.svelte";

  let aiSummary = $state("");
  let aiInsights = $state<{ dates: string[], amounts: string[], orgs: string[] }>({ dates: [], amounts: [], orgs: [] });
  let isShortcutsOpen = $state(false);
  let isZenMode = $state(false);
  let isDragOver = $state(false);
  let documentIntelRequest = 0;
  
  // Comparison Side-by-Side State
  let compPage1 = $state(1);
  let compPage2 = $state(1);
  let isSyncScrolling = $state(true);
  let compareMode = $state<"side-by-side" | "overlay">("side-by-side");
  let overlaySliderPos = $state(50); // percentage
  
  const intelligenceTools = ['extract', 'compare', 'notepad', 'library', 'versions', 'settings', 'insights'];
  const operationTools = ['merge', 'split', 'annotate', 'signature', 'security', 'organize', 'forms', 'watermark', 'compress'];

  const toolPanes: Record<string, any> = {
    extract: ExtractPane,
    insights: InsightPane,
    settings: SettingsPane,
    compare: ComparePane,
    library: LibraryPane,
    versions: VersionsPane,
    notepad: NotepadPane,
    merge: MergePane,
    split: SplitPane,
    annotate: AnnotatePane,
    signature: SignaturePane,
    security: SecurityPane,
    organize: OrganizePane,
    forms: FormsPane,
    watermark: WatermarkPane,
    compress: CompressionPane
  };

  // --- Derived State (Directly from pdfState for maximum reactivity) ---
  let currentPreviewRect = $derived(
    pdfState.activeTool === "annotate"
      ? parseRect(pdfState.annotationRectInput)
      : pdfState.activeTool === "security"
        ? parseRect(pdfState.signRectInput)
        : pdfState.activeTool === "signature"
          ? parseRect(pdfState.signatureRectInput)
          : null
  );

  let currentPreviewStrokes = $derived(
    pdfState.activeTool === "signature" 
      ? pdfState.signatureStrokes 
      : pdfState.activeTool === "annotate" 
        ? pdfState.annotationStrokes 
        : []
  );

  let currentPreviewColor = $derived(
    pdfState.activeTool === "annotate"
      ? pdfState.annotationColor
      : pdfState.activeTool === "signature"
        ? pdfState.signatureColor
        : pdfState.activeTool === "security"
          ? "#3b82f6"
          : "#6366f1"
  );

  function parseRect(rectStr: string): number[] | null {
    if (!rectStr || typeof rectStr !== "string") return null;
    const parts = rectStr.split(",").map((p) => p.trim()).filter((p) => p.length > 0);
    if (parts.length !== 4) return null;
    const nums = parts.map((p) => Number(p));
    if (nums.some((n) => Number.isNaN(n))) return null;
    return nums;
  }

  function handleViewerSelect(event: any) {
    const { rect, strokes: selectedStrokes } = event;

    if (rect) {
      const rectStr = rect.map((n: number) => Math.round(n)).join(", ");
      switch (pdfState.viewerTarget) {
        case "annotate": pdfState.annotationRectInput = rectStr; break;
        case "signature": pdfState.signatureRectInput = rectStr; break;
        case "security": pdfState.signRectInput = rectStr; break;
      }
    }

    if (selectedStrokes) {
      if (pdfState.viewerTarget === "signature") {
        pdfState.signatureStrokes = selectedStrokes;
        if (pdfState.signatureStrokes.length > 0) {
          const allPoints = pdfState.signatureStrokes.flat();
          const xs = allPoints.map((p: [number, number]) => p[0]);
          const ys = allPoints.map((p: [number, number]) => p[1]);
          const minX = Math.min(...xs) - 5;
          const minY = Math.min(...ys) - 5;
          const maxX = Math.max(...xs) + 5;
          const maxY = Math.max(...ys) + 5;
          pdfState.signatureRectInput = `${Math.round(minX)}, ${Math.round(minY)}, ${Math.round(maxX)}, ${Math.round(maxY)}`;
        }
      } else if (pdfState.viewerTarget === "annotate") {
        pdfState.annotationStrokes = selectedStrokes;
      }
    }
  }

  function handleViewerClear() {
    switch (pdfState.viewerTarget) {
      case "annotate": 
        pdfState.annotationRectInput = ""; 
        pdfState.annotationStrokes = [];
        break;
      case "signature": 
        pdfState.signatureStrokes = []; 
        pdfState.signatureRectInput = ""; 
        break;
      case "security": pdfState.signRectInput = ""; break;
    }
  }

  function normalizeInsights(insights: any) {
    return {
      dates: Array.isArray(insights?.dates) ? insights.dates : [],
      amounts: Array.isArray(insights?.amounts) ? insights.amounts : [],
      orgs: Array.isArray(insights?.orgs) ? insights.orgs : []
    };
  }

  function isCurrentIntelRequest(path: string, requestId: number) {
    return requestId === documentIntelRequest && pdfState.viewerFilePath === path;
  }

  const loaderStageOrder = ["converting", "scanning", "indexing", "complete"];
  const loaderSteps = [
    { id: "converting", label: "Converting" },
    { id: "scanning", label: "Scanning Layout" },
    { id: "indexing", label: "Indexing" }
  ];

  function loaderStepClass(step: string, current = "") {
    if (current === "error") return "border-red-500 bg-red-500/10 text-red-600";
    if (current === step) return "border-blue-600 bg-blue-500/10 text-blue-700 dark:text-blue-300 animate-pulse shadow-[3px_3px_0px_0px_rgba(37,99,235,0.45)]";

    const stepIndex = loaderStageOrder.indexOf(step);
    const currentIndex = loaderStageOrder.indexOf(current);
    if (currentIndex > stepIndex) return "border-emerald-500 bg-emerald-500/10 text-emerald-600";

    return "border-slate-300 dark:border-slate-700 text-slate-400 bg-white/50 dark:bg-slate-950/30";
  }

  async function hydrateDocumentIntel(path: string, requestId: number) {
    chatState.loadHistory(path);

    const doc = await db.documents.where('path').equals(path).first();
    if (!isCurrentIntelRequest(path, requestId)) return;

    aiSummary = doc?.summary || "";

    const cachedEntities = await db.entities.where('docPaths').equals(path).toArray();
    if (!isCurrentIntelRequest(path, requestId)) return;

    if (cachedEntities.length > 0) {
      aiInsights = {
        dates: cachedEntities.filter(entity => entity.type === 'date').map(entity => entity.name).slice(0, 3),
        amounts: [],
        orgs: cachedEntities.filter(entity => entity.type === 'org').map(entity => entity.name).slice(0, 3)
      };
    } else {
      aiInsights = { dates: [], amounts: [], orgs: [] };
    }

    if (!doc?.summary) {
      try {
        const summary = await chatState.nameDocument(path);
        if (!isCurrentIntelRequest(path, requestId)) return;
        aiSummary = summary;
        const latest = await db.documents.where('path').equals(path).first();
        if (latest?.id) await db.documents.update(latest.id, { summary });
        await historyState.loadHistory();
      } catch (e) {
        console.error(e);
      }
    }

    if (cachedEntities.length === 0) {
      try {
        const insights = normalizeInsights(await chatState.getDocumentInsights(path));
        if (!isCurrentIntelRequest(path, requestId)) return;
        aiInsights = insights;
        await historyState.indexEntities(path, insights);
      } catch (e) {
        console.error(e);
      }
    }
  }

  $effect(() => {
    const activePath = pdfState.viewerFilePath;
    const requestId = ++documentIntelRequest;

    if (activePath) {
      if (pdfState.activeTool === 'peek' || pdfState.activeTool === 'library') {
        pdfState.activeTool = 'insights';
      }
      hydrateDocumentIntel(activePath, requestId);
    } else {
      aiSummary = "";
      aiInsights = { dates: [], amounts: [], orgs: [] };
    }
  });

  // Multi-Tab Context Menu
  let tabContextMenu = $state<{ x: number, y: number, tab: string } | null>(null);

  function handleTabContextMenu(e: MouseEvent, tab: string) {
    e.preventDefault();
    tabContextMenu = { x: e.clientX, y: e.clientY, tab };
  }

  onMount(() => {
    const handleGlobalKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        if (operationTools.includes(pdfState.activeTool)) pdfState.activeTool = 'peek';
      }
      
      if ((e.metaKey || e.ctrlKey) && e.key === "z") {
        if (e.shiftKey) pdfState.redo();
        else pdfState.undo();
      }
      if ((e.metaKey || e.ctrlKey) && e.key === "/") {
        e.preventDefault();
        isShortcutsOpen = !isShortcutsOpen;
      }
      if ((e.metaKey || e.ctrlKey) && e.altKey && e.key === "z") {
        e.preventDefault();
        isZenMode = !isZenMode;
      }
    };
    window.addEventListener("keydown", handleGlobalKey);
    
    let unlisten: (() => void) | undefined;
    getCurrentWebviewWindow().onDragDropEvent((event) => {
      if (event.payload.type === "enter" || event.payload.type === "over") {
        isDragOver = true;
      } else if (event.payload.type === "leave") {
        isDragOver = false;
      } else if (event.payload.type === "drop") {
        isDragOver = false;
        pdfState.handleDroppedFiles(event.payload.paths);
      }
    }).then(fn => { unlisten = fn; });

    let unlistenPdf: any;
    listen("pdf-created", (event) => {
      const path = event.payload as string;
      historyState.addFile(path);
      appState.showStatus(`Auto-imported: ${path.split(/[/\\]/).pop()}`, false);
    }).then(fn => unlistenPdf = fn);

    return () => {
      window.removeEventListener("keydown", handleGlobalKey);
      if (unlisten) unlisten();
      if (unlistenPdf) unlistenPdf();
    };
  });
</script>

<div class="flex h-screen w-screen overflow-hidden font-sans transition-colors duration-300 bg-white dark:bg-slate-950 text-slate-900 dark:text-slate-100">
  {#if !isZenMode}
    <div class="shrink-0 h-full">
      <SidebarNav />
    </div>

    <!-- Secondary Sidebar Pane -->
    <div 
      class="relative h-full overflow-hidden border-r border-slate-200 dark:border-slate-800 transition-all duration-500 shrink-0 {(intelligenceTools.includes(pdfState.activeTool) || operationTools.includes(pdfState.activeTool)) ? 'w-80 opacity-100' : 'w-0 opacity-0 pointer-events-none'}" 
      data-testid="sidebar-pane"
    >
      {#key pdfState.activeTool}
        {@const ActivePane = toolPanes[pdfState.activeTool]}
        {#if ActivePane}
          <div class="absolute inset-0" data-testid="active-pane-{pdfState.activeTool}">
            <ActivePane />
          </div>
        {/if}
      {/key}
    </div>
  {/if}

  <!-- Main Area -->
  <main class="flex-1 flex flex-col min-w-0 bg-slate-50 dark:bg-slate-950 relative h-full">
    
    {#if pdfState.viewerFilePath}
      <!-- PDF Viewer Layer -->
      <div class="absolute inset-0 z-10 flex flex-col bg-white dark:bg-slate-950 transition-all duration-300">
        <!-- Tab Bar -->
        <div class="flex items-center px-4 bg-white dark:bg-slate-900 border-b border-slate-200 dark:border-slate-800 overflow-x-auto no-scrollbar shrink-0">
          {#each pdfState.openTabs as tab}
            <div class="flex items-center group">
              <button 
                onclick={() => pdfState.openTab(tab)}
                oncontextmenu={(e) => handleTabContextMenu(e, tab)}
                class="flex items-center gap-2 px-4 py-2 text-[10px] font-bold transition-all border-b-2 {pdfState.viewerFilePath === tab ? 'text-blue-600 border-blue-600 bg-blue-50/50 dark:bg-blue-900/10' : 'text-slate-400 border-transparent hover:text-slate-600 hover:bg-slate-50 dark:hover:bg-slate-800/50'}"
              >
                <span class="truncate max-w-[120px]">{tab.split(/[/\\]/).pop()}</span>
              </button>
              <button 
                onclick={(e) => { e.stopPropagation(); pdfState.closeTab(tab); }}
                class="px-2 py-2 text-slate-300 hover:text-red-500 transition-colors border-b-2 border-transparent {pdfState.viewerFilePath === tab ? 'bg-blue-50/50 dark:bg-blue-900/10' : 'hover:bg-slate-50'}"
              >✕</button>
            </div>
          {/each}
        </div>

        <div class="flex items-center justify-between px-8 py-3 bg-white dark:bg-slate-900 border-b border-slate-200 dark:border-slate-800 shadow-sm transition-colors duration-300 shrink-0">
          <div class="flex items-center gap-3 overflow-hidden">
            <span class="p-1 bg-red-100 dark:bg-red-900/30 text-red-600 dark:text-red-400 rounded-md text-[9px] font-black uppercase tracking-tighter transition-colors">PDF</span>
            <div class="flex flex-col min-w-0">
              <h1 class="text-xs font-bold truncate text-slate-700 dark:text-slate-300 max-w-md tracking-tight transition-colors">{pdfState.viewerFilePath.split(/[/\\]/).pop()}</h1>
              {#if aiSummary}
                <p class="text-[9px] text-slate-400 dark:text-slate-500 font-medium truncate italic leading-none mt-0.5 tracking-tight">{aiSummary}</p>
              {/if}
              <div class="flex gap-2 mt-1.5 overflow-hidden">
                 {#each (aiInsights.dates || []) as date}
                   <span class="px-1.5 py-0.5 bg-green-50 dark:bg-green-900/20 text-green-600 dark:text-green-400 border border-green-100 dark:border-green-800 rounded text-[7px] font-black uppercase tracking-tighter shadow-sm">{date}</span>
                 {/each}
                 {#each (aiInsights.amounts || []) as amt}
                   <span class="px-1.5 py-0.5 bg-amber-50 dark:bg-amber-900/20 text-amber-600 dark:text-amber-400 border border-amber-100 dark:border-amber-800 rounded text-[7px] font-black uppercase tracking-tighter shadow-sm">{amt}</span>
                 {/each}
                 {#each (aiInsights.orgs || []) as org}
                   <span class="px-1.5 py-0.5 bg-blue-50 dark:bg-blue-900/20 text-blue-600 dark:text-blue-400 border border-blue-100 dark:border-blue-800 rounded text-[7px] font-black uppercase tracking-tighter shadow-sm">{org}</span>
                 {/each}
              </div>
              </div>

          </div>
          <div class="flex items-center gap-4 text-xs font-medium text-slate-500 transition-colors">
            <button onclick={() => pdfState.viewerFilePath = ""} class="px-3 py-1 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 rounded-lg transition-all text-[9px] font-black uppercase tracking-widest text-slate-600 dark:text-slate-400">Dashboard</button>
            <span class="bg-blue-50 dark:bg-blue-900/20 text-blue-600 dark:text-blue-400 px-2 py-0.5 rounded-full uppercase tracking-widest text-[9px] font-black transition-colors">Live Preview</span>
            <button onclick={() => pdfState.viewerFilePath = ""} class="hover:text-red-500 transition-colors">✕</button>
          </div>
        </div>
        
        <div class="flex-1 overflow-hidden p-8 flex justify-center items-start transition-colors duration-300 bg-slate-100 dark:bg-slate-900/40 relative">
          <!-- @ts-ignore -->
          {#if pdfState.activeTool === 'compare' && pdfState.comparisonFile2}
            <div class="flex flex-col w-full h-full max-h-[85vh] gap-4">
              <div class="flex justify-center gap-4 bg-white dark:bg-slate-900 p-2 rounded-xl border border-slate-200 dark:border-slate-800 shadow-sm self-center">
                 <button onclick={() => compareMode = 'side-by-side'} class="px-4 py-1 text-[9px] font-black uppercase rounded-lg transition-all {compareMode === 'side-by-side' ? 'bg-blue-600 text-white shadow-md' : 'text-slate-400 hover:text-slate-600'}">Side-by-Side</button>
                 <button onclick={() => compareMode = 'overlay'} class="px-4 py-1 text-[9px] font-black uppercase rounded-lg transition-all {compareMode === 'overlay' ? 'bg-blue-600 text-white shadow-md' : 'text-slate-400 hover:text-slate-600'}">Visual Overlay</button>
              </div>

              {#if compareMode === 'side-by-side'}
                <div class="flex gap-4 w-full h-full">
                  <div class="flex-1 flex flex-col min-w-0">
                    <div class="text-[9px] font-black uppercase text-slate-400 mb-2 px-1">Original</div>
                    <PdfViewer
                      filePath={pdfState.viewerFilePath}
                      pageNumber={compPage1}
                      mode="view"
                      onnext={() => { compPage1++; if(isSyncScrolling) compPage2++; }}
                      onprev={() => { compPage1 = Math.max(1, compPage1 - 1); if(isSyncScrolling) compPage2 = Math.max(1, compPage2 - 1); }}
                      onclose={() => (pdfState.viewerFilePath = "")}
                    />
                  </div>
                  <div class="flex-1 flex flex-col min-w-0">
                    <div class="flex justify-between items-center mb-2 px-1">
                      <div class="text-[9px] font-black uppercase text-slate-400">Revised</div>
                      <label class="flex items-center gap-1 cursor-pointer">
                          <input type="checkbox" bind:checked={isSyncScrolling} class="w-2.5 h-3 rounded" />
                          <span class="text-[8px] font-black uppercase text-blue-500">Sync Scroll</span>
                      </label>
                    </div>
                    <PdfViewer
                      filePath={pdfState.comparisonFile2}
                      pageNumber={compPage2}
                      mode="view"
                      onnext={() => { compPage2++; if(isSyncScrolling) compPage1++; }}
                      onprev={() => { compPage2 = Math.max(1, compPage2 - 1); if(isSyncScrolling) compPage1 = Math.max(1, compPage1 - 1); }}
                      onclose={() => { 
                        // @ts-ignore
                        pdfState.comparisonFile2 = null; 
                      }}
                    />
                  </div>
                </div>
              {:else}
                <!-- Visual Overlay Mode with Slider -->
                <div class="relative w-full h-full flex justify-center items-center overflow-hidden rounded-2xl shadow-2xl border border-slate-300 dark:border-slate-800 bg-white dark:bg-slate-900">
                   <div class="absolute inset-0 flex justify-center items-start pt-8">
                      <!-- Layer 1: Revised -->
                      <div class="absolute inset-0 opacity-50 contrast-125 brightness-110 grayscale-50">
                        <PdfViewer
                          filePath={pdfState.comparisonFile2}
                          pageNumber={compPage2}
                          mode="view"
                        />
                      </div>
                      <!-- Layer 2: Original (Clipped) -->
                      <div class="absolute inset-0 z-10 pointer-events-none" style="clip-path: inset(0 {100 - overlaySliderPos}% 0 0)">
                        <PdfViewer
                          filePath={pdfState.viewerFilePath}
                          pageNumber={compPage1}
                          mode="view"
                        />
                      </div>

                      <!-- Draggable Slider Handle -->
                      <div 
                        class="absolute top-0 bottom-0 z-20 w-1 bg-blue-600 cursor-ew-resize flex items-center justify-center group"
                        style="left: {overlaySliderPos}%"
                      >
                         <div class="w-8 h-8 rounded-full bg-blue-600 text-white flex items-center justify-center shadow-2xl transition-transform group-active:scale-90 border-2 border-white">
                           ↔️
                         </div>
                         <input 
                          type="range" 
                          bind:value={overlaySliderPos} 
                          class="absolute inset-0 opacity-0 w-full h-full cursor-ew-resize" 
                          min="0" max="100" 
                         />
                      </div>
                   </div>
                </div>
              {/if}
            </div>
          {:else}
            <div class="w-full h-full min-h-0 flex justify-center items-start overflow-hidden">
              <PdfViewer
                filePath={pdfState.viewerFilePath}
                bind:pageNumber={pdfState.viewerPageNumber}
                mode={pdfState.viewerMode}
                previewRect={currentPreviewRect}
                previewStrokes={currentPreviewStrokes}
                previewColor={currentPreviewColor}
                ocrTrigger={pdfState.ocrTrigger}
                entityMappingTrigger={aiInsights}
                highlightedSnippet={pdfState.highlightedSnippet}
                formFields={[...pdfState.scannedFields, ...pdfState.formFieldsToCreate]}
                onselect={handleViewerSelect}
                onclear={handleViewerClear}
                ondone={() => (pdfState.viewerMode = "view")}
                onprev={() => (pdfState.viewerPageNumber = Math.max(1, pdfState.viewerPageNumber - 1))}
                onnext={() => pdfState.viewerPageNumber++}
                onclose={() => (pdfState.viewerFilePath = "")}
                onreorder={(newOrder: number[]) => {
                  if (pdfState.viewerTarget === 'rotate') {
                   invoke("save_file_dialog", { defaultPath: "reordered.pdf" }).then(outputPath => {
                     if (!outputPath) return;
                     appState.startLoading("Reordering pages...");
                     invoke("reorder_pages", { path: pdfState.viewerFilePath, newOrder, outputPath }).then(() => {
                        appState.showStatus("Pages reordered successfully.", false, outputPath as string);
                        invoke("shell_open", { filePath: outputPath as string });
                     }).catch(e => appState.showStatus(`Reorder failed: ${e}`, true));
                   });
                  }
                }}
              />
            </div>
          {/if}
        </div>
      </div>
    {:else}
      <!-- Welcome Screen Layer -->
      <div class="absolute inset-0 z-0 flex flex-col items-center justify-center p-12 bg-slate-50 dark:bg-slate-950 overflow-y-auto no-scrollbar h-full">
        <div class="max-w-4xl w-full">
          <div class="text-center mb-16">
             <div class="w-24 h-24 bg-gradient-to-br from-blue-500 to-indigo-600 rounded-3xl flex items-center justify-center mx-auto mb-8 text-4xl shadow-2xl shadow-blue-500/20 text-white animate-pulse">📄</div>
             <h2 class="text-4xl font-black text-slate-900 dark:text-white mb-4 tracking-tighter transition-colors">Pinnacle Intelligence</h2>
             <p class="text-sm text-slate-500 dark:text-slate-400 mb-8 leading-relaxed font-medium transition-colors tracking-tight max-w-lg mx-auto italic">
                Securely analyze, edit, and organize your documents with local-first intelligence. No data ever leaves your device.
             </p>
             
             <div class="flex flex-wrap justify-center gap-4">
               <button onclick={() => pdfState.openNewDocument()} class="px-8 py-4 bg-blue-600 hover:bg-blue-700 text-white rounded-2xl shadow-lg shadow-blue-500/20 hover:shadow-blue-500/40 hover:-translate-y-1 transition-all text-sm font-black uppercase tracking-widest">Open a Document</button>
               <button onclick={() => pdfState.switchTool('extract')} class="px-6 py-4 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-2xl shadow-sm hover:shadow-xl hover:-translate-y-1 transition-all text-xs font-black uppercase tracking-widest text-slate-600 dark:text-slate-300">Start with Assistant</button>
               <button onclick={() => pdfState.switchTool('library')} class="px-6 py-4 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-2xl shadow-sm hover:shadow-xl hover:-translate-y-1 transition-all text-xs font-black uppercase tracking-widest text-slate-600 dark:text-slate-300">Browse Library</button>
             </div>
          </div>
          
          {#if historyState.recentFiles.length > 0}
            <div class="mt-12" in:fly={{ y: 20, duration: 600, delay: 200 }}>
              <div class="flex items-center justify-between mb-6 px-4">
                <h3 class="text-xs font-black text-slate-400 uppercase tracking-[0.2em]">Recent Documents</h3>
                <button onclick={() => historyState.clear()} class="text-[10px] font-black text-slate-400 hover:text-red-500 transition-colors uppercase tracking-widest">Wipe History</button>
              </div>
              <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                {#each historyState.recentFiles as file}
                  <button 
                    onclick={() => { 
                      pdfState.viewerFilePath = file.path; 
                      // @ts-ignore
                      pdfState.setFileForTarget(pdfState.activeTool, file.path); 
                    }}
                    class="flex items-center gap-4 p-4 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-2xl hover:border-blue-500 dark:hover:border-blue-400 transition-all text-left group shadow-sm hover:shadow-xl"
                  >
                    <div class="w-10 h-10 flex items-center justify-center rounded-xl bg-slate-50 dark:bg-slate-800 text-slate-400 group-hover:text-blue-500 group-hover:bg-blue-50 dark:group-hover:bg-blue-900/20 transition-all">📄</div>
                    <div class="min-w-0">
                      <div class="text-xs font-black text-slate-700 dark:text-slate-200 truncate">{file.name}</div>
                      <div class="text-[9px] text-slate-400 truncate font-mono">{file.path.split(/[/\\]/).slice(-3).join('/')}</div>
                    </div>
                  </button>
                {/each}
              </div>
            </div>
          {/if}

          <div class="mt-20 flex justify-center opacity-50 grayscale hover:grayscale-0 transition-all">
            <div class="inline-flex gap-4 p-2 bg-white dark:bg-slate-900 rounded-2xl border border-slate-200 dark:border-slate-800 transition-colors shadow-sm items-center">
              <div class="flex items-center gap-2 px-3 py-1 bg-green-50 dark:bg-green-900/20 rounded-xl">
                 <div class="w-1.5 h-1.5 rounded-full bg-green-500 animate-pulse"></div>
                 <span class="text-[9px] font-black text-green-700 dark:text-green-400 uppercase tracking-widest">End-to-End Encrypted</span>
              </div>
              <span class="text-[9px] font-black text-slate-500 dark:text-slate-400 uppercase tracking-widest">Local Intel</span>
              <span class="text-[9px] font-black text-slate-500 dark:text-slate-400 uppercase tracking-widest">Privacy First</span>
            </div>
          </div>
        </div>
      </div>
    {/if}
  </main>
</div>

{#if isDragOver || pdfState.activeLoader}
  {@const loader = pdfState.activeLoader}
  <div class="fixed inset-0 z-[700] flex items-center justify-center bg-slate-950/35 backdrop-blur-sm p-6" in:fade={{ duration: 120 }}>
    <div
      class="w-full max-w-xl bg-white/85 dark:bg-slate-950/85 backdrop-blur-2xl border-4 border-slate-950 dark:border-white rounded-[8px] shadow-[14px_14px_0px_0px_rgba(15,23,42,1)] dark:shadow-[14px_14px_0px_0px_rgba(255,255,255,0.9)] p-6"
      in:scale={{ start: 0.97, duration: 140 }}
    >
      <div class="flex items-start justify-between gap-4 mb-5">
        <div class="min-w-0">
          <div class="text-[10px] font-black uppercase tracking-[0.24em] text-blue-600 dark:text-blue-300">
            {loader ? "Importing Document" : "Drop to Import"}
          </div>
          <div class="mt-2 text-sm font-black text-slate-950 dark:text-white truncate">
            {loader?.filename || "PDF, DOCX, XLSX, PNG, JPG"}
          </div>
          <div class="mt-1 text-[10px] font-bold text-slate-500 dark:text-slate-400 truncate">
            {loader?.detail || "Pinnacle will convert, scan layout, and index locally."}
          </div>
        </div>
        <div class="h-12 w-12 shrink-0 rounded-[8px] border-2 border-slate-950 dark:border-white bg-amber-300 text-slate-950 shadow-[5px_5px_0px_0px_rgba(15,23,42,1)] flex items-center justify-center text-xl">
          ⬇
        </div>
      </div>

      <div class="grid grid-cols-3 gap-2 mb-5">
        {#each loaderSteps as step}
          <div class="rounded-[6px] border-2 px-2 py-2 text-center text-[8px] font-black uppercase tracking-wider transition-all {loaderStepClass(step.id, loader?.stage)}">
            {step.label}
          </div>
        {/each}
      </div>

      <div class="h-3 rounded-full border-2 border-slate-950 dark:border-white bg-white dark:bg-slate-900 overflow-hidden">
        <div
          class="h-full transition-all duration-300 {loader?.stage === 'error' ? 'bg-red-500' : 'bg-blue-600'}"
          style="width: {loader?.progress || (isDragOver ? 6 : 0)}%"
        ></div>
      </div>

      <div class="mt-4 flex items-center justify-between text-[9px] font-black uppercase tracking-[0.18em] text-slate-400">
        <span>{loader?.stage || "Ready"}</span>
        <span>{loader?.progress || 0}%</span>
      </div>
    </div>
  </div>
{/if}

<CommandPalette />
<ShortcutsModal isOpen={isShortcutsOpen} onclose={() => isShortcutsOpen = false} />
<OnboardingTour />
<StatusDisplay />

{#if pdfState.showSignPad}
  <SignaturePadModal />
{/if}

{#if tabContextMenu}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="fixed inset-0 z-[600]" onclick={() => tabContextMenu = null}>
    <div 
      class="absolute bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl shadow-2xl py-1.5 min-w-[160px] overflow-hidden"
      style="left: {tabContextMenu.x}px; top: {tabContextMenu.y}px"
      in:fly={{ y: -5, duration: 150 }}
    >
      <button onclick={() => { pdfState.closeTab(tabContextMenu?.tab || ""); tabContextMenu = null; }} class="w-full text-left px-3 py-1.5 text-[10px] font-bold text-slate-700 dark:text-slate-300 hover:bg-blue-50 dark:hover:bg-blue-900/30 transition-colors uppercase tracking-tight">Close Tab</button>
      <button onclick={() => { pdfState.openTabs = [tabContextMenu?.tab || ""]; tabContextMenu = null; }} class="w-full text-left px-3 py-1.5 text-[10px] font-bold text-slate-700 dark:text-slate-300 hover:bg-blue-50 dark:hover:bg-blue-900/30 transition-colors uppercase tracking-tight">Close Others</button>
      <div class="h-[1px] bg-slate-100 dark:bg-slate-800 my-1"></div>
      <button onclick={() => { pdfState.saveSession(prompt("Session Name:") || "Default"); tabContextMenu = null; }} class="w-full text-left px-3 py-1.5 text-[10px] font-bold text-blue-600 hover:bg-blue-50 dark:hover:bg-blue-900/30 transition-colors uppercase tracking-tight">Save as Session</button>
    </div>
  </div>
{/if}

<style lang="postcss">
  @reference "tailwindcss";

  :global(.viewer-container) {
    @apply shadow-[0_32px_64px_-12px_rgba(0,0,0,0.1)] rounded-3xl overflow-hidden border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 transition-colors duration-300;
  }
  
  :global(.canvas-wrapper) {
    @apply rounded-2xl;
  }

  :global(.viewer-controls) {
    @apply bg-slate-100 dark:bg-slate-900 border-t border-slate-300 dark:border-slate-800 p-6 transition-colors duration-300;
  }
</style>
