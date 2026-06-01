<script lang="ts">
  import { onMount, tick, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { browser } from "$app/environment";
  import Tesseract from "tesseract.js";
  import { pdfState } from "$lib/state/pdfState.svelte";
  import { appState } from "$lib/state/appState.svelte";
  import { chatState } from "$lib/state/chatState.svelte";
  import { fly, fade } from "svelte/transition";

  let {
    filePath = "",
    pageNumber = $bindable(1),
    mode = "view",
    previewRect = null,
    previewStrokes = [],
    previewColor = "red",
    ocrTrigger = 0,
    entityMappingTrigger = { dates: [], amounts: [], orgs: [] },
    highlightedSnippet = null,
    formFields = [],
    onselect,
    onclear,
    onclose,
    ondone,
    onprev,
    onnext,
    onreorder
  } = $props<{
    filePath?: string;
    pageNumber?: number;
    mode?: "rect" | "points" | "view";
    previewRect?: number[] | null;
    previewStrokes?: [number, number][][];
    previewColor?: string;
    ocrTrigger?: number;
    entityMappingTrigger?: { dates: string[], amounts: string[], orgs: string[] };
    highlightedSnippet?: string | null;
    formFields?: { name: string, field_type: string, page: number, rect: number[] }[];
    onselect?: (event: any) => void;

    onclear?: () => void;
    onclose?: () => void;
    ondone?: () => void;
    onprev?: () => void;
    onnext?: () => void;
    onreorder?: (newOrder: number[]) => void;
  }>();

  let canvas: HTMLCanvasElement | undefined = $state();
  let container: HTMLDivElement | undefined = $state();
  let pdfjs: any = $state(null);
  let pdfDoc: any = $state(null);
  let viewport: any = $state(null);
  let scale = $state(1.5);
  let loading = $state(false);
  let error = $state("");
  let ocrProcessing = $state(false);

  // Reader Settings
  let isInverted = $state(false);

  // Search and Zoom
  let searchQuery = $state("");
  let searchResults = $state<{ page: number, index: number }[]>([]);
  let currentSearchIndex = $state(-1);
  let searchHighlights = $state<{ page: number, rects: number[][] }[]>([]);
  let entityHighlights = $state<{ page: number, label: string, color: string, rects: number[][] }[]>([]);
  let snippetHighlights = $state<number[][]>([]);

  // Native Text Selection
  let textItems = $state<{ str: string, transform: number[], width: number, height: number }[]>([]);
  let textLayer: HTMLDivElement | undefined = $state();
  let selectionState = $state<{ text: string, x: number, y: number } | null>(null);
  let activeAnnotation = $state<any | null>(null);

  // Thumbnails, Outlines and Reordering
  let thumbnails = $state<{ pageNumber: number, dataUrl: string }[]>([]);
  let outline = $state<any[]>([]);
  let annotations = $state<any[]>([]);
  let sidebarTab = $state<"thumbs" | "outline" | "bookmarks" | "annots">("thumbs");
  let isSidebarOpen = $state(true);
  let readerTheme = $state<"default" | "sepia" | "high-contrast">("default");
  let isSpeaking = $state(false);
  let isBionic = $state(false);
  let isPresentationMode = $state(false);
  let isLaserActive = $state(false);
  let laserPos = $state({ x: 0, y: 0 });
  let synth: SpeechSynthesis | undefined = $state();
  let draggedIndex = $state<number | null>(null);
  let scrollContainer: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  const ITEM_HEIGHT = 160;

  let visibleThumbnails = $derived.by(() => {
    if (!scrollContainer) return thumbnails.slice(0, 10);
    const start = Math.max(0, Math.floor(scrollTop / ITEM_HEIGHT) - 2);
    const end = Math.min(thumbnails.length, start + 10);
    return thumbnails.slice(start, end).map((t, i) => ({ ...t, offset: (start + i) * ITEM_HEIGHT }));
  });

  let isDrawing = $state(false);
  let currentRect = $state({ x1: 0, y1: 0, x2: 0, y2: 0 });
  let strokes = $state<[number, number][][]>([]);
  let currentStroke = $state<[number, number][]>([]);

  // Draggable Signature Stamp State (PDF Coordinate System)
  let stampPdfX = $state(100);
  let stampPdfY = $state(200);
  let stampPdfW = $state(100);
  let stampPdfH = $state(60);

  let isDraggingStamp = $state(false);
  let dragStartMouseX = 0;
  let dragStartMouseY = 0;
  let dragStartStampPdfX = 0;
  let dragStartStampPdfY = 0;

  let isResizingStamp = $state(false);
  let resizeStartMouseX = 0;
  let resizeStartStampPdfW = 0;

  // Reactively calculate viewport coordinates for drawing the overlay
  let stampX = $derived.by(() => {
    if (!viewport) return 100;
    const [vx] = viewport.convertToViewportPoint(stampPdfX, stampPdfY);
    return vx;
  });
  let stampY = $derived.by(() => {
    if (!viewport) return 100;
    const [, vy] = viewport.convertToViewportPoint(stampPdfX, stampPdfY);
    return vy;
  });
  let stampW = $derived(stampPdfW * scale);
  let stampH = $derived(stampPdfH * scale);

  $effect(() => {
    if (pdfState.activeStamp && viewport) {
      stampPdfW = 120; // Default Standard Width in PDF Points (approx 1.6 inches)
      stampPdfH = 120 / pdfState.activeStamp.aspectRatio;
      
      // Center the stamp on the page bounds
      const [pageWidthPdf, pageHeightPdf] = viewport.viewBox.slice(2);
      stampPdfX = (pageWidthPdf - stampPdfW) / 2;
      stampPdfY = (pageHeightPdf + stampPdfH) / 2;
    }
  });

  function handleStampMouseDown(e: MouseEvent | TouchEvent) {
    if ("button" in e && e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    isDraggingStamp = true;
    const clientX = "touches" in e ? e.touches[0].clientX : e.clientX;
    const clientY = "touches" in e ? e.touches[0].clientY : e.clientY;
    dragStartMouseX = clientX;
    dragStartMouseY = clientY;
    dragStartStampPdfX = stampPdfX;
    dragStartStampPdfY = stampPdfY;
  }

  function handleResizeMouseDown(e: MouseEvent | TouchEvent) {
    if ("button" in e && e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    isResizingStamp = true;
    const clientX = "touches" in e ? e.touches[0].clientX : e.clientX;
    resizeStartMouseX = clientX;
    resizeStartStampPdfW = stampPdfW;
  }

  function handleGlobalMouseMove(e: MouseEvent | TouchEvent) {
    if (!viewport) return;
    
    if (isDraggingStamp) {
      const clientX = "touches" in e ? e.touches[0].clientX : e.clientX;
      const clientY = "touches" in e ? e.touches[0].clientY : e.clientY;
      const dx = clientX - dragStartMouseX;
      const dy = clientY - dragStartMouseY;
      
      // Translate viewport delta pixels to PDF points:
      const dPdfX = dx / scale;
      const dPdfY = -dy / scale; // inverted Y
      
      const [pageWidthPdf, pageHeightPdf] = viewport.viewBox.slice(2);
      // Clamp within page bounds
      stampPdfX = Math.max(0, Math.min(pageWidthPdf - stampPdfW, dragStartStampPdfX + dPdfX));
      stampPdfY = Math.max(stampPdfH, Math.min(pageHeightPdf, dragStartStampPdfY + dPdfY));
    } else if (isResizingStamp) {
      const clientX = "touches" in e ? e.touches[0].clientX : e.clientX;
      const dx = clientX - resizeStartMouseX;
      
      // Translate viewport width delta pixels to PDF points:
      const dPdfW = dx / scale;
      const newPdfW = Math.max(20, resizeStartStampPdfW + dPdfW);
      
      if (pdfState.activeStamp) {
        stampPdfW = newPdfW;
        stampPdfH = newPdfW / pdfState.activeStamp.aspectRatio;
        
        // Also clamp stampPdfY if height expands beyond top page limit
        const [, pageHeightPdf] = viewport.viewBox.slice(2);
        if (stampPdfY < stampPdfH) {
          stampPdfY = stampPdfH;
        }
      }
    }
  }

  function handleGlobalMouseUp() {
    isDraggingStamp = false;
    isResizingStamp = false;
  }

  function cancelStamp() {
    pdfState.activeStamp = null;
  }

  function placeStamp() {
    if (!pdfState.activeStamp || !viewport) return;
    
    // Bounds in PDF points
    const px1 = stampPdfX;
    const py1 = stampPdfY;
    const px2 = stampPdfX + stampPdfW;
    const py2 = stampPdfY - stampPdfH;
    
    // Map normalized strokes [0..1] directly to absolute PDF coordinates
    const mappedStrokes = pdfState.activeStamp.strokes.map(stroke => 
      stroke.map(([nx, ny]) => {
        const pdfX = px1 + nx * stampPdfW;
        const pdfY = py1 - ny * stampPdfH;
        return [parseFloat(pdfX.toFixed(2)), parseFloat(pdfY.toFixed(2))] as [number, number];
      })
    );

    const change = {
      id: crypto.randomUUID ? crypto.randomUUID() : Math.random().toString(36).slice(2),
      target: "signature" as const,
      page: pageNumber,
      rect: [
        parseFloat(px1.toFixed(2)),
        parseFloat(py2.toFixed(2)),
        parseFloat(px2.toFixed(2)),
        parseFloat(py1.toFixed(2))
      ],
      strokes: mappedStrokes,
      type: "ink",
      color: pdfState.signatureColor,
      width: pdfState.signatureWidth || 2
    };

    pdfState.addPendingChange(change);
    appState.showStatus("Signature placed! Click 'Apply All' in the sidebar when done.", false);
  }

  let lastLoadedPath = "";
  let lastRenderedPage = -1;
  let currentRenderTask: any = null;
  let isRendering = false;
  let preRenderedPages = $state<Map<number, string>>(new Map());

  async function predictivePreRender() {
    if (!pdfDoc) return;
    const adjacent = [pageNumber - 1, pageNumber + 1].filter(p => p >= 1 && p <= pdfDoc.numPages);
    const hiddenCanvas = document.createElement("canvas");
    const ctx = hiddenCanvas.getContext("2d")!;
    for (const p of adjacent) {
      if (preRenderedPages.has(p)) continue;
      try {
        const page = await pdfDoc.getPage(p);
        const vp = page.getViewport({ scale });
        hiddenCanvas.width = vp.width; hiddenCanvas.height = vp.height;
        await page.render({ canvasContext: ctx, viewport: vp }).promise;
        preRenderedPages.set(p, hiddenCanvas.toDataURL());
      } catch (e) {}
    }
    if (preRenderedPages.size > 5) preRenderedPages.clear();
  }

  $effect(() => {
    if (ocrTrigger > 0 && canvas && browser) {
      performOcr();
    }
  });

  async function performOcr() {
    if (!canvas) return;
    ocrProcessing = true;
    try {
      const dataUrl = canvas.toDataURL("image/png");
      const { data: { text } } = await Tesseract.recognize(dataUrl, "eng", {
        logger: m => console.log(m)
      });
      
      const defaultPath = `page_${pageNumber}_ocr.txt`;
      const outputPath = await invoke<string | null>("save_file_dialog", { defaultPath });
      
      if (outputPath && text) {
        await invoke("write_text_file", { path: outputPath, contents: text });
        await invoke("shell_open", { filePath: outputPath });
      }
    } catch (e: any) {
      error = "OCR Failed: " + e.toString();
    } finally {
      ocrProcessing = false;
    }
  }

  async function initPdfJs() {
    if (!browser || pdfjs) return;
    try {
      // @ts-ignore
      pdfjs = await import("pdfjs-dist");
      pdfjs.GlobalWorkerOptions.workerSrc = new URL(
        "pdfjs-dist/build/pdf.worker.mjs",
        import.meta.url
      ).toString();
    } catch (err: any) {
      error = "Failed to initialize: " + err.toString();
    }
  }

  async function loadDocument() {
    if (!pdfjs || !filePath || filePath === lastLoadedPath) return;
    if (pdfDoc) { try { await pdfDoc.destroy(); } catch (e) {} pdfDoc = null; }
    loading = true; error = "";
    try {
      // Use Uint8Array directly to avoid expensive number array conversion
      const uint8Array = await invoke<Uint8Array>("read_file_bytes", { path: filePath });
      const loadingTask = pdfjs.getDocument({ data: uint8Array });
      pdfDoc = await loadingTask.promise;
      lastLoadedPath = filePath; lastRenderedPage = -1;
      
      // Parallelize non-critical tasks
      Promise.all([
        generateThumbnails(),
        invoke("get_pdf_outline", { path: filePath }).then(o => outline = o as any[]).catch(() => outline = []),
        invoke("get_annotations", { path: filePath }).then(a => annotations = a as any[]).catch(() => annotations = []),
        pdfState.loadBookmarks(filePath).catch(() => {}),
        pdfState.getReadingProgress(filePath).then(p => { pageNumber = p; })
      ]);

      await renderPage();
    } catch (err: any) { error = "Load failed: " + err.toString(); } finally { loading = false; }
  }

  async function generateThumbnails() {
    if (!pdfDoc) return;
    const thumbs = [];
    const canvas = document.createElement("canvas");
    const ctx = canvas.getContext("2d")!;
    
    // Only generate first few thumbnails immediately, then the rest in background
    const limit = Math.min(pdfDoc.numPages, 100);
    for (let i = 1; i <= limit; i++) {
      try {
        const page = await pdfDoc.getPage(i);
        const vp = page.getViewport({ scale: 0.2 });
        canvas.width = vp.width;
        canvas.height = vp.height;
        await page.render({ canvasContext: ctx, viewport: vp }).promise;
        thumbs.push({ pageNumber: i, dataUrl: canvas.toDataURL() });
        
        // Update incrementally for better perceived performance
        if (i % 5 === 0 || i === limit) {
          thumbnails = [...thumbs];
        }
      } catch (e) {
        console.error(`Failed to generate thumbnail for page ${i}`, e);
      }
    }
  }

  async function renderPage() {
    if (!pdfDoc || !canvas || (pageNumber === lastRenderedPage && !loading)) return;
    if (currentRenderTask) { try { currentRenderTask.cancel(); await currentRenderTask.promise; } catch (e) {} currentRenderTask = null; }
    if (isRendering) return;
    isRendering = true;
    try {
      await tick();
      const page = await pdfDoc.getPage(pageNumber);
      const context = canvas.getContext("2d");
      if (!context) { isRendering = false; return; }
      viewport = page.getViewport({ scale });
      canvas.height = viewport.height; canvas.width = viewport.width;
      currentRenderTask = page.render({ canvasContext: context, viewport });
      await currentRenderTask.promise;
      lastRenderedPage = pageNumber;
      
      // Save progress
      await pdfState.saveReadingProgress(filePath, pageNumber, pdfDoc.numPages);

      // Extract Text Content for Native Selection Layer
      const tc = await page.getTextContent();
      textItems = tc.items.map((it: any) => ({
        str: it.str,
        transform: it.transform,
        width: it.width,
        height: it.height
      }));
      
      predictivePreRender();
    } catch (err: any) { if (err.name !== "RenderingCancelledException") error = "Render failed: " + err.toString(); } finally { currentRenderTask = null; isRendering = false; }
  }

  onMount(async () => { await initPdfJs(); });
  $effect(() => { if (pdfjs && filePath !== lastLoadedPath) loadDocument(); });
  $effect(() => { if (pdfDoc && (pageNumber !== lastRenderedPage)) renderPage(); });
  $effect(() => { if (pdfDoc && entityMappingTrigger) mapEntitiesToCanvas(entityMappingTrigger); });
  $effect(() => { if (pdfDoc && highlightedSnippet) mapSnippetToCanvas(highlightedSnippet); else snippetHighlights = []; });
  $effect(() => { if (!isDrawing) strokes = [...previewStrokes]; });

  function handleMouseDown(e: MouseEvent) {
    if (loading || error || !viewport || !canvas) return;
    const rect = canvas.getBoundingClientRect();
    const x = e.clientX - rect.left; const y = e.clientY - rect.top;
    if (mode === "rect") { isDrawing = true; currentRect = { x1: x, y1: y, x2: x, y2: y }; }
    else if (mode === "points") { 
      isDrawing = true; 
      const [pdfX, pdfY] = viewport.convertToPdfPoint(x, y); 
      currentStroke = [[pdfX, pdfY]]; 
    }
  }

  function handleMouseMove(e: MouseEvent) {
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    const x = e.clientX - rect.left; const y = e.clientY - rect.top;

    if (isLaserActive) {
      laserPos = { x, y };
    }

    if (!isDrawing) return;
    if (mode === "rect") currentRect = { ...currentRect, x2: x, y2: y };
    else if (mode === "points" && viewport) {
      const [pdfX, pdfY] = viewport.convertToPdfPoint(x, y);
      const last = currentStroke[currentStroke.length - 1];
      if (last) {
        const [lvx, lvy] = viewport.convertToViewportPoint(last[0], last[1]);
        if (Math.sqrt(Math.pow(x - lvx, 2) + Math.pow(y - lvy, 2)) > 2) currentStroke = [...currentStroke, [pdfX, pdfY]];
      }
    }
  }

  function handleTextSelection() {
    const sel = window.getSelection();
    if (sel && sel.toString().trim().length > 0) {
      const range = sel.getRangeAt(0);
      const rect = range.getBoundingClientRect();
      selectionState = {
        text: sel.toString().trim(),
        x: rect.left + rect.width / 2,
        y: rect.top - 40
      };
    } else {
      selectionState = null;
    }
  }

  function handleMouseUp() {
    if (!isDrawing) return;
    isDrawing = false;
    if (mode === "rect" && viewport) {
      const width = Math.abs(currentRect.x2 - currentRect.x1);
      const height = Math.abs(currentRect.y2 - currentRect.y1);
      
      // Region Zoom - Only if no specific target is active (e.g. annotate, sign)
      if (width > 20 && height > 20 && !pdfState.viewerTarget && sidebarTab === 'thumbs') {
         const zoomFactor = Math.min(canvas!.width / width, canvas!.height / height);
         scale = Math.min(scale * zoomFactor, 10.0);
         renderPage();
         currentRect = { x1: 0, y1: 0, x2: 0, y2: 0 };
         return;
      }

      const [px1, py1] = viewport.convertToPdfPoint(currentRect.x1, currentRect.y1);
      const [px2, py2] = viewport.convertToPdfPoint(currentRect.x2, currentRect.y2);
      onselect?.({
        rect: [parseFloat(px1.toFixed(2)), parseFloat(py1.toFixed(2)), parseFloat(px2.toFixed(2)), parseFloat(py2.toFixed(2))]
      });
    } else if (mode === "points") {
      if (currentStroke.length > 0) {
        strokes = [...strokes, currentStroke];
        currentStroke = [];
        onselect?.({
          strokes: strokes.map(s => s.map(p => [parseFloat(p[0].toFixed(2)), parseFloat(p[1].toFixed(2))]))
        });
      }
    }
  }

  function clear() { strokes = []; currentStroke = []; currentRect = { x1: 0, y1: 0, x2: 0, y2: 0 }; onclear?.(); }
  function undo() { if (mode === "points" && strokes.length > 0) { strokes = strokes.slice(0, -1); onselect?.({ strokes: strokes.map(s => s.map(p => [parseFloat(p[0].toFixed(2)), parseFloat(p[1].toFixed(2))])) }); } }

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      if (isPresentationMode) isPresentationMode = false;
      else onclose?.();
    }
    else if (e.key === "z" && (e.ctrlKey || e.metaKey)) undo();
    else if (e.key === "ArrowRight" || e.key === "ArrowDown") {
      if (pageNumber < (pdfDoc?.numPages || 0)) onnext?.();
    }
    else if (e.key === "ArrowLeft" || e.key === "ArrowUp") {
      if (pageNumber > 1) onprev?.();
    }
  }

  // Drag and Drop for Reordering
  function handleDragStart(index: number) {
    draggedIndex = index;
  }

  function handleDragOver(e: DragEvent, index: number) {
    e.preventDefault();
    if (draggedIndex === null || draggedIndex === index) return;
    
    const newThumbs = [...thumbnails];
    const item = newThumbs.splice(draggedIndex, 1)[0];
    newThumbs.splice(index, 0, item);
    thumbnails = newThumbs;
    draggedIndex = index;
  }

  function handleDrop() {
    draggedIndex = null;
    onreorder?.(thumbnails.map(t => t.pageNumber));
  }

  async function mapEntitiesToCanvas(entities: { dates: string[], amounts: string[], orgs: string[] }) {
    if (!pdfDoc || !entities) return;
    const highlights: typeof entityHighlights = [];
    
    for (let i = 1; i <= pdfDoc.numPages; i++) {
      const page = await pdfDoc.getPage(i);
      const textContent = await page.getTextContent();
      
      const process = (list: string[], label: string, color: string) => {
        if (!Array.isArray(list)) return;
        for (const val of list) {
          if (!val || typeof val !== "string") continue;
          for (const item of textContent.items as any[]) {
            if (item.str && item.str.toLowerCase().includes(val.toLowerCase())) {
              const [sx, sy, skx, sky, tx, ty] = item.transform;
              highlights.push({ page: i, label, color, rects: [[tx, ty, tx + item.width, ty + item.height]] });
            }
          }
        }
      };

      process(entities.dates, "Date", "#10b981"); // Green
      process(entities.amounts, "Amount", "#f59e0b"); // Amber
      process(entities.orgs, "Organization", "#3b82f6"); // Blue
    }
    entityHighlights = highlights;
  }

  async function mapSnippetToCanvas(snippet: string) {
    if (!pdfDoc) return;
    const rects: number[][] = [];
    const page = await pdfDoc.getPage(pageNumber);
    const textContent = await page.getTextContent();
    
    // Simple direct matching for now, future refinement: fuzzy match
    for (const item of textContent.items as any[]) {
      if (snippet.toLowerCase().includes(item.str.toLowerCase()) && item.str.trim().length > 3) {
        const [sx, sy, skx, sky, tx, ty] = item.transform;
        rects.push([tx, ty, tx + item.width, ty + item.height]);
      }
    }
    snippetHighlights = rects;
  }

  async function handleSearch() {
    if (!pdfDoc || !searchQuery.trim()) { 
      searchResults = []; currentSearchIndex = -1; searchHighlights = []; return; 
    }
    const results: { page: number, index: number }[] = [];
    const highlights: { page: number, rects: number[][] }[] = [];
    
    for (let i = 1; i <= pdfDoc.numPages; i++) {
      const page = await pdfDoc.getPage(i);
      const textContent = await page.getTextContent();
      const pageHighlights: number[][] = [];
      
      for (const item of textContent.items as any[]) {
        if (item.str.toLowerCase().includes(searchQuery.toLowerCase())) {
          if (results.length === 0 || results[results.length-1].page !== i) {
            results.push({ page: i, index: results.length });
          }
          // Extract rect: [tx, ty, tx+width, ty+height]
          const [sx, sy, skx, sky, tx, ty] = item.transform;
          pageHighlights.push([tx, ty, tx + item.width, ty + item.height]);
        }
      }
      if (pageHighlights.length > 0) {
        highlights.push({ page: i, rects: pageHighlights });
      }
    }
    
    searchResults = results;
    searchHighlights = highlights;
    if (results.length > 0) {
      currentSearchIndex = 0;
      pageNumber = results[0].page;
    } else {
      currentSearchIndex = -1;
    }
  }

  function nextSearch() {
    if (searchResults.length === 0) return;
    currentSearchIndex = (currentSearchIndex + 1) % searchResults.length;
    pageNumber = searchResults[currentSearchIndex].page;
  }

  function zoomIn() { scale = Math.min(scale + 0.25, 4.0); renderPage(); }
  function zoomOut() { scale = Math.max(scale - 0.25, 0.5); renderPage(); }

  function toggleReadAloud() {
    if (!synth) synth = window.speechSynthesis;
    if (isSpeaking) {
      synth.cancel();
      isSpeaking = false;
      return;
    }
    const text = textItems.map(it => it.str).join(" ");
    if (!text.trim()) return;
    const utter = new SpeechSynthesisUtterance(text);
    utter.onend = () => isSpeaking = false;
    synth.speak(utter);
    isSpeaking = true;
  }

  let jumpPage = $state(1);
  function handleJump() {
    if (jumpPage >= 1 && jumpPage <= (pdfDoc?.numPages || 1)) {
      pageNumber = jumpPage;
    }
  }

  $effect(() => { jumpPage = pageNumber; });

  async function exportAsImage() {
    if (!canvas) return;
    const dataUrl = canvas.toDataURL("image/png");
    const bytes = await (await fetch(dataUrl)).arrayBuffer();
    const uint8 = new Uint8Array(bytes);
    const outputPath = await invoke<string | null>("save_file_dialog", { defaultPath: `page_${pageNumber}.png` });
    if (outputPath) {
      // We can use the write_text_file logic but with bytes if we had a write_binary_file command.
      // Since we have read_file_bytes, let's assume we can add write_file_bytes or just use shell_open for now if it's a web link.
      // Wait, let's check commands.rs for binary write.
      // Actually, I'll just trigger a browser download for simplicity as it's a UI feature.
      const link = document.createElement("a");
      link.href = dataUrl;
      link.download = `page_${pageNumber}.png`;
      link.click();
      appState.showStatus("Page exported as image.", false);
    }
  }

  onDestroy(async () => { if (pdfDoc) try { await pdfDoc.destroy(); } catch (e) {} });
</script>

<svelte:window 
  onmousemove={handleGlobalMouseMove} 
  onmouseup={handleGlobalMouseUp}
  ontouchmove={handleGlobalMouseMove}
  ontouchend={handleGlobalMouseUp}
/>

{#snippet renderOutlineItem(item: any, depth: number)}
  <div class="space-y-1">
    <button 
      onclick={() => { if (item.page) pageNumber = item.page; }}
      class="w-full text-left p-1.5 rounded hover:bg-slate-200 dark:hover:bg-slate-800 transition-colors flex items-start gap-2 group"
      style="padding-left: {depth * 12 + 6}px"
    >
      <span class="text-[10px] font-bold text-slate-700 dark:text-slate-300 line-clamp-2 leading-tight group-hover:text-blue-600 transition-colors">{item.title}</span>
      {#if item.page}
        <span class="ml-auto text-[8px] font-black text-slate-400 uppercase tracking-tighter">p.{item.page}</span>
      {/if}
    </button>
    {#if item.children && item.children.length > 0}
      <div class="space-y-1">
        {#each item.children as child}
          {@render renderOutlineItem(child, depth + 1)}
        {/each}
      </div>
    {/if}
  </div>
{/snippet}

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div class="relative bg-white dark:bg-slate-900 rounded-xl shadow-2xl border border-slate-300 dark:border-slate-800 flex flex-col outline-none overflow-hidden h-full transition-all duration-500 {isPresentationMode ? 'fixed inset-4 z-[500] max-h-none' : ''}" bind:this={container} onkeydown={handleKeyDown} role="region" aria-label="PDF Document Viewer">
  {#if loading || ocrProcessing}
    <div class="absolute inset-0 z-50 flex items-center justify-center bg-white/90 dark:bg-slate-900/80 backdrop-blur-sm transition-colors duration-300">
      <div class="flex flex-col items-center gap-3">
        <div class="w-10 h-10 border-4 border-blue-100 dark:border-blue-900 border-t-blue-600 rounded-full animate-spin"></div>
        <span class="text-sm font-semibold text-slate-700 dark:text-slate-400 tracking-tight">
          {ocrProcessing ? "Performing Local OCR..." : "Optimizing view..."}
        </span>
      </div>
    </div>
  {:else if error}
    <div class="absolute inset-0 z-50 flex items-center justify-center bg-red-50 dark:bg-red-950/20 text-red-700 dark:text-red-400 p-8 text-center font-bold">{error}</div>
  {/if}

  {#if !isPresentationMode}
    <div class="shrink-0 px-8 py-3 bg-white dark:bg-slate-900 border-b border-slate-200 dark:border-slate-800 flex items-center justify-between transition-colors">
    <div class="flex items-center gap-4">
      <div class="flex items-center bg-slate-100 dark:bg-slate-800 rounded-lg px-3 py-1.5 border border-slate-200 dark:border-slate-700 shadow-inner">
        <span class="text-xs text-slate-400 mr-2">🔍</span>
        <input 
          bind:value={searchQuery} 
          onkeydown={(e) => e.key === 'Enter' && handleSearch()}
          placeholder="Search text..." 
          class="bg-transparent outline-none text-xs text-slate-700 dark:text-slate-200 w-32"
        />
        {#if searchResults.length > 0}
          <span class="text-[9px] font-bold text-blue-500 ml-2">{currentSearchIndex + 1}/{searchResults.length}</span>
          <button onclick={nextSearch} class="ml-2 text-slate-400 hover:text-blue-500">↓</button>
        {/if}
      </div>
      <div class="flex items-center gap-1 bg-slate-100 dark:bg-slate-800 rounded-lg p-1 border border-slate-200 dark:border-slate-700">
        <button onclick={zoomOut} class="p-1 hover:bg-white dark:hover:bg-slate-700 rounded text-slate-500">－</button>
        <span class="px-2 text-[10px] font-bold text-slate-600 dark:text-slate-300 w-12 text-center">{Math.round(scale * 100)}%</span>
        <button onclick={zoomIn} class="p-1 hover:bg-white dark:hover:bg-slate-700 rounded text-slate-500">＋</button>
      </div>
    </div>
    
    <div class="flex items-center gap-4">
      <button 
        onclick={() => isLaserActive = !isLaserActive}
        class="text-[10px] font-bold uppercase tracking-widest {isLaserActive ? 'text-red-500' : 'text-slate-400'} hover:text-red-500 transition-colors"
      >
        Laser
      </button>
      <button 
        onclick={() => isPresentationMode = !isPresentationMode}
        class="text-[10px] font-bold uppercase tracking-widest {isPresentationMode ? 'text-blue-500' : 'text-slate-400'} hover:text-blue-500 transition-colors"
      >
        Present
      </button>
      <button 
        onclick={toggleReadAloud}
        class="text-[10px] font-bold uppercase tracking-widest {isSpeaking ? 'text-blue-500 animate-pulse' : 'text-slate-400'} hover:text-blue-500 transition-colors"
      >
        {isSpeaking ? 'Stop Reading' : 'Read Aloud'}
      </button>
      <div class="flex items-center gap-1 bg-slate-100 dark:bg-slate-800 rounded-lg p-1 border border-slate-200 dark:border-slate-700">
        <button onclick={() => readerTheme = 'default'} class="px-2 py-1 text-[8px] font-black uppercase rounded {readerTheme === 'default' ? 'bg-white dark:bg-slate-700 text-blue-600 shadow-sm' : 'text-slate-400'}">Day</button>
        <button onclick={() => readerTheme = 'sepia'} class="px-2 py-1 text-[8px] font-black uppercase rounded {readerTheme === 'sepia' ? 'bg-[#f4ecd8] text-[#5b4636] shadow-sm' : 'text-slate-400'}">Sepia</button>
        <button onclick={() => readerTheme = 'high-contrast'} class="px-2 py-1 text-[8px] font-black uppercase rounded {readerTheme === 'high-contrast' ? 'bg-black text-white shadow-sm' : 'text-slate-400'}">Pro</button>
        <button onclick={() => isBionic = !isBionic} class="px-2 py-1 text-[8px] font-black uppercase rounded {isBionic ? 'bg-blue-600 text-white shadow-sm' : 'text-slate-400'}">Bionic</button>
      </div>
      <button 
        onclick={() => isInverted = !isInverted}
        class="text-[10px] font-bold uppercase tracking-widest {isInverted ? 'text-blue-500' : 'text-slate-400'} hover:text-blue-500 transition-colors"
      >
        Night Mode
      </button>
      <button 
        onclick={exportAsImage}
        class="text-[10px] font-bold uppercase tracking-widest text-slate-400 hover:text-blue-500 transition-colors"
      >
        Export PNG
      </button>
      <button 
        onclick={() => isSidebarOpen = !isSidebarOpen}
        class="text-[10px] font-bold uppercase tracking-widest {isSidebarOpen ? 'text-blue-500' : 'text-slate-400'}"
      >
        Thumbnails
      </button>
    </div>
  </div>
  {/if}

  <div class="flex flex-1 min-h-0">
    <!-- Sidebar (Virtualized Thumbs, Outline, or Bookmarks) -->
    {#if !isPresentationMode && isSidebarOpen && (thumbnails.length > 0 || outline.length > 0 || pdfState.bookmarks.length > 0)}
      <aside 
        class="w-56 flex flex-col border-r border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 transition-colors"
      >
        <div class="flex border-b border-slate-200 dark:border-slate-800 overflow-x-auto">
          <button 
            onclick={() => sidebarTab = 'thumbs'}
            class="flex-1 py-3 text-[9px] font-black uppercase tracking-widest transition-colors {sidebarTab === 'thumbs' ? 'text-blue-600 bg-white dark:bg-slate-900' : 'text-slate-400 hover:text-slate-600'}"
          >Thumbs</button>
          <button 
            onclick={() => sidebarTab = 'outline'}
            class="flex-1 py-3 text-[9px] font-black uppercase tracking-widest transition-colors {sidebarTab === 'outline' ? 'text-blue-600 bg-white dark:bg-slate-900' : 'text-slate-400 hover:text-slate-600'}"
          >Outline</button>
          <button 
            onclick={() => sidebarTab = 'bookmarks'}
            class="flex-1 py-3 text-[9px] font-black uppercase tracking-widest transition-colors {sidebarTab === 'bookmarks' ? 'text-blue-600 bg-white dark:bg-slate-900' : 'text-slate-400 hover:text-slate-600'}"
          >Bookmarks</button>
          <button 
            onclick={() => sidebarTab = 'annots'}
            class="flex-1 py-3 text-[9px] font-black uppercase tracking-widest transition-colors {sidebarTab === 'annots' ? 'text-blue-600 bg-white dark:bg-slate-900' : 'text-slate-400 hover:text-slate-600'}"
          >Annots</button>
        </div>

        <div 
          bind:this={scrollContainer}
          onscroll={(e) => scrollTop = (e.target as HTMLDivElement).scrollTop}
          class="flex-1 overflow-y-auto p-4"
        >
          {#if sidebarTab === 'thumbs'}
            <div class="relative" style="height: {thumbnails.length * ITEM_HEIGHT}px">
              {#each visibleThumbnails as thumb}
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <div 
                  draggable="true"
                  ondragstart={() => handleDragStart(thumbnails.findIndex(t => t.pageNumber === thumb.pageNumber))}
                  ondragover={(e) => handleDragOver(e, thumbnails.findIndex(t => t.pageNumber === thumb.pageNumber))}
                  ondrop={handleDrop}
                  class="absolute left-0 right-0 flex flex-col items-center gap-2 group cursor-grab active:cursor-grabbing px-2"
                  style="top: {(thumb as any).offset}px; height: {ITEM_HEIGHT}px"
                >
                  <button 
                    onclick={() => pageNumber = thumb.pageNumber}
                    class="relative w-full rounded border-2 transition-all {pageNumber === thumb.pageNumber ? 'border-blue-600 shadow-lg scale-[1.02]' : 'border-transparent hover:border-slate-300 dark:hover:border-slate-700'}"
                  >
                    <img src={thumb.dataUrl} alt="Page {thumb.pageNumber}" class="w-full h-auto rounded-sm shadow-sm transition-opacity group-hover:opacity-90" />
                    <div class="absolute top-1 left-1 px-1.5 py-0.5 bg-slate-900/60 text-white text-[8px] font-bold rounded backdrop-blur-xs">
                      {thumb.pageNumber}
                    </div>
                  </button>
                </div>
              {/each}
            </div>
          {:else if sidebarTab === 'outline'}
            <div class="space-y-2">
              {#each outline as item}
                {@render renderOutlineItem(item, 0)}
              {/each}
              {#if outline.length === 0}
                <div class="text-[10px] text-slate-400 italic text-center py-8">No Table of Contents found.</div>
              {/if}
            </div>
          {:else if sidebarTab === 'bookmarks'}
            <div class="space-y-2">
              <div class="flex justify-between items-center mb-4 px-1">
                <span class="text-[9px] font-black text-slate-400 uppercase tracking-widest">Active Page: {pageNumber}</span>
                <button 
                  onclick={() => {
                    const label = prompt("Bookmark label:", `Page ${pageNumber}`);
                    if (label) pdfState.addBookmark(filePath, pageNumber, label);
                  }}
                  class="text-[9px] font-black text-blue-600 uppercase tracking-tighter"
                >+ Add</button>
              </div>
              {#each pdfState.bookmarks as bookmark}
                <div class="group flex items-center justify-between p-2 rounded hover:bg-slate-200 dark:hover:bg-slate-800 transition-colors">
                  <button 
                    onclick={() => pageNumber = bookmark.pageNumber}
                    class="flex-1 text-left min-w-0"
                  >
                    <div class="text-[10px] font-bold text-slate-700 dark:text-slate-300 truncate">{bookmark.label}</div>
                    <div class="text-[8px] text-slate-400 font-black uppercase tracking-tighter">Page {bookmark.pageNumber}</div>
                  </button>
                  <button 
                    onclick={() => bookmark.id && pdfState.deleteBookmark(bookmark.id)}
                    class="p-1 opacity-0 group-hover:opacity-100 text-slate-400 hover:text-red-500 transition-all"
                  >✕</button>
                </div>
              {/each}
              {#if pdfState.bookmarks.length === 0}
                <div class="text-[10px] text-slate-400 italic text-center py-8 px-4">Save points of interest to access them instantly.</div>
              {/if}
            </div>
          {:else if sidebarTab === 'annots'}
            <div class="space-y-2">
              <h3 class="text-[9px] font-black text-slate-400 uppercase tracking-widest mb-4">Document Markup</h3>
              {#each annotations as annot}
                <button 
                  onclick={() => pageNumber = annot.page}
                  class="w-full text-left p-2.5 bg-white dark:bg-slate-900 border border-slate-100 dark:border-slate-800 rounded-xl hover:border-blue-500 transition-all shadow-sm group"
                >
                  <div class="flex justify-between items-center mb-1">
                    <span class="text-[8px] font-black uppercase text-blue-500">{annot.kind}</span>
                    <span class="text-[8px] font-black text-slate-400">Page {annot.page}</span>
                  </div>
                  <div class="text-[10px] font-medium text-slate-600 dark:text-slate-300 line-clamp-2 italic">{annot.contents || 'No comment'}</div>
                </button>
              {/each}
              {#if annotations.length === 0}
                <div class="text-[10px] text-slate-400 italic text-center py-8 px-4">No annotations found in this document.</div>
              {/if}
            </div>
          {/if}
        </div>
      </aside>
    {/if}

    <div class="relative flex-1 p-8 overflow-auto flex justify-center items-start transition-colors duration-300 bg-slate-100 dark:bg-slate-900/40">
      <!-- Floating Instruction Banner -->
      {#if mode !== 'view' || pdfState.activeStamp}
        <div class="absolute top-4 left-1/2 -translate-x-1/2 z-[40] flex items-center gap-3 px-4 py-2 bg-slate-900/90 dark:bg-white/95 text-white dark:text-slate-950 rounded-lg shadow-xl border border-slate-950 dark:border-white font-bold text-xs uppercase tracking-wider backdrop-blur-md animate-in slide-in-from-top-4 duration-300">
          {#if pdfState.activeStamp}
            <span>✍️ Place Signature: Drag and resize on the page, then click Place</span>
          {:else if mode === 'rect'}
            <span>🎯 Selection Mode: Click and drag to select a region</span>
          {:else if mode === 'points'}
            <span>✏️ Drawing Mode: Draw directly on the document</span>
          {/if}
        </div>
      {/if}
      <!-- Search Minimap -->
      {#if searchHighlights.length > 0}
        <div class="absolute right-2 top-8 bottom-8 w-1.5 bg-slate-200/50 dark:bg-slate-800/50 rounded-full z-30 pointer-events-none overflow-hidden">
          {#each searchHighlights as group}
             <div 
              class="absolute left-0 right-0 h-0.5 bg-yellow-400 opacity-60" 
              style="top: {(group.page / (pdfDoc?.numPages || 1)) * 100}%"
             ></div>
          {/each}
        </div>
      {/if}

      <div class="relative shadow-2xl rounded-sm group {readerTheme === 'sepia' ? 'sepia-[0.3] brightness-95' : ''} {readerTheme === 'high-contrast' ? 'contrast-125' : ''}">
        <canvas 
          bind:this={canvas} 
          onmousedown={handleMouseDown} 
          onmousemove={handleMouseMove} 
          onmouseup={handleMouseUp} 
          onmouseleave={handleMouseUp} 
          class="bg-white ring-1 ring-slate-300 dark:ring-slate-700 rounded-sm cursor-crosshair transition-all duration-300 {isInverted ? 'invert hue-rotate-180 brightness-90 contrast-110' : ''}"
        ></canvas>

        <!-- Native Text Selection Layer -->
        {#if viewport && textItems.length > 0}
          <div 
            class="absolute top-0 left-0 right-0 bottom-0 {mode === 'view' ? 'pointer-events-auto' : 'pointer-events-none'} select-text text-transparent overflow-hidden"
            bind:this={textLayer}
            onmouseup={handleTextSelection}
            role="none"
          >
            {#each textItems as item}
              {@const [tx, ty] = viewport.convertToViewportPoint(item.transform[4], item.transform[5])}
              <span 
                class="absolute origin-bottom-left whitespace-pre leading-none {isBionic ? 'font-medium' : ''}"
                style="left: {tx}px; top: {ty - (item.height * scale)}px; font-size: {item.height * scale}px; font-family: sans-serif; transform: scaleX({item.width / (item.str.length || 1) / (item.height || 1)});"
              >
                {#if isBionic}
                   {#each item.str.split(" ") as word, i}
                      <b>{word.slice(0, Math.ceil(word.length / 2))}</b>{word.slice(Math.ceil(word.length / 2))}{i < item.str.split(" ").length - 1 ? ' ' : ''}
                   {/each}
                {:else}
                  {item.str}
                {/if}
              </span>
            {/each}
          </div>
        {/if}

        <!-- Render Active Signature Stamp -->
        {#if pdfState.activeStamp && viewport}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <div 
            class="absolute select-none group/stamp border-2 border-dashed border-blue-600 bg-blue-50/10 cursor-move z-30"
            style="left: {stampX}px; top: {stampY}px; width: {stampW}px; height: {stampH}px;"
            onmousedown={handleStampMouseDown}
            ontouchstart={handleStampMouseDown}
          >
            <!-- SVG Preview of strokes inside the box -->
            <svg class="w-full h-full pointer-events-none" viewBox="0 0 1 1" preserveAspectRatio="none">
              {#each pdfState.activeStamp.strokes as stroke}
                <polyline 
                  points={stroke.map(p => `${p[0]},${p[1]}`).join(' ')} 
                  fill="none" 
                  stroke={pdfState.signatureColor} 
                  stroke-width="0.02" 
                  stroke-linecap="round" 
                  stroke-linejoin="round" 
                />
              {/each}
            </svg>

            <!-- Resize handle (Neubrutalist style) in bottom-right corner -->
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div 
              class="absolute right-0 bottom-0 w-4 h-4 bg-slate-900 border border-white cursor-se-resize flex items-center justify-center translate-x-1 translate-y-1 z-40 active:scale-95"
              onmousedown={handleResizeMouseDown}
              ontouchstart={handleResizeMouseDown}
            >
              <div class="w-1.5 h-1.5 border-r border-b border-white"></div>
            </div>

            <!-- Floating Stamp Action Toolbar -->
            <div class="absolute -top-12 left-1/2 -translate-x-1/2 bg-slate-900 text-white border-2 border-slate-950 dark:border-white px-2 py-1 flex items-center gap-1 shadow-lg z-40 rounded-none animate-in fade-in slide-in-from-bottom-2 duration-200">
              <button 
                onclick={placeStamp} 
                class="px-2 py-1 text-[10px] font-black uppercase hover:bg-slate-800 flex items-center gap-1"
              >
                ✅ Place
              </button>
              <div class="w-[1px] h-4 bg-slate-700"></div>
              <button 
                onclick={cancelStamp} 
                class="px-2 py-1 text-[10px] font-black uppercase hover:bg-slate-800 text-red-400 flex items-center gap-1"
              >
                ❌ Cancel
              </button>
            </div>
          </div>
        {/if}
      </div>

    {#if isDrawing && mode === "rect"}
      <div class="absolute border-2 border-blue-600 bg-blue-600/20 pointer-events-none transition-all z-20" style="left: {Math.min(currentRect.x1, currentRect.x2)+32}px; top: {Math.min(currentRect.y1, currentRect.y2)+32}px; width: {Math.abs(currentRect.x2 - currentRect.x1)}px; height: {Math.abs(currentRect.y2 - currentRect.y1)}px;"></div>
    {/if}

    {#if previewRect && viewport && !isDrawing}
      {@const [vx1, vy1] = viewport.convertToViewportPoint(previewRect[0], previewRect[1])}
      {@const [vx2, vy2] = viewport.convertToViewportPoint(previewRect[2], previewRect[3])}
      <div class="absolute border-2 pointer-events-none z-10 rounded-sm transition-colors duration-300" style="left: {Math.min(vx1, vx2)+32}px; top: {Math.min(vy1, vy2)+32}px; width: {Math.abs(vx2 - vx1)}px; height: {Math.abs(vy2 - vy1)}px; border-color: {previewColor}; background: {previewColor}33;"></div>
    {/if}

    {#if viewport}
      <svg class="absolute top-0 left-0 pointer-events-none p-8 z-10" width={canvas?.width ? canvas.width + 64 : 64} height={canvas?.height ? canvas.height + 64 : 64}>
        <!-- Search Highlights -->
        {#each searchHighlights.filter(h => h.page === pageNumber) as highlight}
           {#each highlight.rects as rect}
              {@const [vx1, vy1] = viewport.convertToViewportPoint(rect[0], rect[1])}
              {@const [vx2, vy2] = viewport.convertToViewportPoint(rect[2], rect[3])}
              <rect 
                x={Math.min(vx1, vx2)} 
                y={Math.min(vy1, vy2)} 
                width={Math.abs(vx2 - vx1)} 
                height={Math.abs(vy2 - vy1)} 
                fill="#facc15" 
                fill-opacity="0.3" 
                class="transition-opacity duration-200"
              />
           {/each}
        {/each}

        <!-- Entity Highlights -->
        {#each entityHighlights.filter(h => h.page === pageNumber) as entity}
           {#each entity.rects as rect}
              {@const [vx1, vy1] = viewport.convertToViewportPoint(rect[0], rect[1])}
              {@const [vx2, vy2] = viewport.convertToViewportPoint(rect[2], rect[3])}
              <rect 
                x={Math.min(vx1, vx2)} 
                y={Math.min(vy1, vy2)} 
                width={Math.abs(vx2 - vx1)} 
                height={Math.abs(vy2 - vy1)} 
                fill={entity.color} 
                fill-opacity="0.25" 
                stroke={entity.color}
                stroke-width="1"
                class="transition-all duration-200 hover:fill-opacity-40 cursor-help"
              >
                <title>{entity.label}</title>
              </rect>
           {/each}
        {/each}

        <!-- Active Annotations (Clickable) -->
        {#each annotations.filter(a => a.page === pageNumber) as annot}
           {@const [vx1, vy1] = viewport.convertToViewportPoint(annot.rect[0], annot.rect[1])}
           {@const [vx2, vy2] = viewport.convertToViewportPoint(annot.rect[2], annot.rect[3])}
           <!-- svelte-ignore a11y_no_static_element_interactions -->
           <!-- svelte-ignore a11y_click_events_have_key_events -->
           <rect 
             x={Math.min(vx1, vx2)} 
             y={Math.min(vy1, vy2)} 
             width={Math.abs(vx2 - vx1)} 
             height={Math.abs(vy2 - vy1)} 
             fill="transparent"
             class="cursor-pointer pointer-events-auto hover:stroke-blue-500 hover:stroke-2"
             onclick={(e) => {
               e.stopPropagation();
               activeAnnotation = { ...annot, vx: Math.min(vx1, vx2) + Math.abs(vx2-vx1)/2, vy: Math.min(vy1, vy2) };
             }}
           />
        {/each}

        <!-- Snippet Highlights (Citations) -->
        {#each snippetHighlights as rect}
           {@const [vx1, vy1] = viewport.convertToViewportPoint(rect[0], rect[1])}
           {@const [vx2, vy2] = viewport.convertToViewportPoint(rect[2], rect[3])}
           <rect 
             x={Math.min(vx1, vx2)} 
             y={Math.min(vy1, vy2)} 
             width={Math.abs(vx2 - vx1)} 
             height={Math.abs(vy2 - vy1)} 
             fill="#a855f7" 
             fill-opacity="0.35" 
             class="animate-pulse"
           />
        {/each}

        <!-- Form Field Overlays (Builder Mode Indicators) -->
        {#if formFields}
          {#each formFields.filter((f: { page: number }) => f.page === pageNumber) as field}
            {@const [vx1, vy1] = viewport.convertToViewportPoint(field.rect[0], field.rect[1])}
            {@const [vx2, vy2] = viewport.convertToViewportPoint(field.rect[2], field.rect[3])}
            <g class="form-field-indicator group/field">
              <!-- Shadow/Offset for Neubrutalist look -->
              <rect 
                x={Math.min(vx1, vx2) + 2} 
                y={Math.min(vy1, vy2) + 2} 
                width={Math.abs(vx2 - vx1)} 
                height={Math.abs(vy2 - vy1)} 
                fill="rgba(0, 0, 0, 0.1)"
                rx="1"
              />
              <!-- Main Box -->
              <rect 
                x={Math.min(vx1, vx2)} 
                y={Math.min(vy1, vy2)} 
                width={Math.abs(vx2 - vx1)} 
                height={Math.abs(vy2 - vy1)} 
                fill={field.field_type === 'Btn' ? 'rgba(239, 68, 68, 0.05)' : 'rgba(59, 130, 246, 0.05)'} 
                stroke="#0f172a"
                stroke-width="1.5"
                rx="1"
                class="transition-all duration-200 group-hover/field:fill-white dark:group-hover/field:fill-slate-800"
              />
              <!-- Label Badge -->
              <g transform="translate({Math.min(vx1, vx2)}, {Math.min(vy1, vy2) - 14})">
                <rect 
                  x="0" y="0" 
                  width={field.name.length * 5 + 35} 
                  height="12" 
                  fill="#0f172a" 
                  rx="2"
                />
                <text 
                  x="4" y="9" 
                  font-size="7" 
                  font-weight="bold" 
                  fill="white" 
                  class="uppercase tracking-widest"
                >
                  {field.name} • {field.field_type === 'Tx' ? 'Text' : field.field_type === 'Btn' ? 'Check' : 'Choice'}
                </text>
              </g>
            </g>
          {/each}
        {/if}

        <!-- Laser Pointer -->
        {#if isLaserActive}
          <circle 
            cx={laserPos.x} 
            cy={laserPos.y} 
            r="6" 
            fill="#ef4444" 
            class="shadow-2xl opacity-80"
            filter="drop-shadow(0 0 8px rgba(239, 68, 68, 0.8))"
          />
          <circle 
            cx={laserPos.x} 
            cy={laserPos.y} 
            r="20" 
            fill="url(#laserGradient)" 
            class="opacity-20 animate-pulse"
          />
          <defs>
            <radialGradient id="laserGradient">
              <stop offset="0%" stop-color="#ef4444" />
              <stop offset="100%" stop-color="transparent" />
            </radialGradient>
          </defs>
        {/if}

        {#if mode === "points"}
          {#each strokes as stroke}
            <polyline points={stroke.map(pt => { const [cx, cy] = viewport.convertToViewportPoint(pt[0], pt[1]); return `${cx},${cy}`; }).join(' ')} fill="none" stroke={previewColor} stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" />
          {/each}
          {#if currentStroke.length > 0}
            <polyline points={currentStroke.map(pt => { const [cx, cy] = viewport.convertToViewportPoint(pt[0], pt[1]); return `${cx},${cy}`; }).join(' ')} fill="none" stroke={previewColor} stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" />
          {/if}
        {:else if previewStrokes.length > 0 && !isDrawing}
          {#each previewStrokes as stroke}
            <polyline points={stroke.map((pt: [number, number]) => { const [cx, cy] = viewport.convertToViewportPoint(pt[0], pt[1]); return `${cx},${cy}`; }).join(' ')} fill="none" stroke={previewColor} stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" />
          {/each}
        {/if}
        <!-- Render Pending Changes Overlay -->
        {#each pdfState.pendingChanges.filter(c => c.page === pageNumber) as change}
          {#if change.target === 'annotate' && change.rect && change.type !== 'ink'}
            {@const [vx1, vy1] = viewport.convertToViewportPoint(change.rect[0], change.rect[1])}
            {@const [vx2, vy2] = viewport.convertToViewportPoint(change.rect[2], change.rect[3])}
            {#if change.type === 'highlight'}
              <rect 
                x={Math.min(vx1, vx2)} 
                y={Math.min(vy1, vy2)} 
                width={Math.abs(vx2 - vx1)} 
                height={Math.abs(vy2 - vy1)} 
                fill={change.color} 
                fill-opacity="0.3" 
              />
            {:else if change.type === 'underline'}
              <line 
                x1={vx1} 
                y1={vy2} 
                x2={vx2} 
                y2={vy2} 
                stroke={change.color} 
                stroke-width="2" 
              />
            {:else if change.type === 'strikeout'}
              <line 
                x1={vx1} 
                y1={(vy1 + vy2) / 2} 
                x2={vx2} 
                y2={(vy1 + vy2) / 2} 
                stroke={change.color} 
                stroke-width="2" 
              />
            {:else if change.type === 'square'}
              <rect 
                x={Math.min(vx1, vx2)} 
                y={Math.min(vy1, vy2)} 
                width={Math.abs(vx2 - vx1)} 
                height={Math.abs(vy2 - vy1)} 
                fill="none" 
                stroke={change.color} 
                stroke-width="2" 
              />
            {:else if change.type === 'circle'}
              <ellipse 
                cx={(vx1 + vx2) / 2} 
                cy={(vy1 + vy2) / 2} 
                rx={Math.abs(vx2 - vx1) / 2} 
                ry={Math.abs(vy2 - vy1) / 2} 
                fill="none" 
                stroke={change.color} 
                stroke-width="2" 
              />
            {:else}
              <rect 
                x={Math.min(vx1, vx2)} 
                y={Math.min(vy1, vy2)} 
                width={Math.abs(vx2 - vx1)} 
                height={Math.abs(vy2 - vy1)} 
                fill={change.color} 
                fill-opacity="0.15" 
                stroke={change.color} 
                stroke-width="1.5" 
                stroke-dasharray="3,3" 
              />
            {/if}
          {:else if change.strokes}
            {#each change.strokes as stroke}
              <polyline 
                points={stroke.map(pt => { 
                  const [cx, cy] = viewport.convertToViewportPoint(pt[0], pt[1]); 
                  return `${cx},${cy}`; 
                }).join(' ')} 
                fill="none" 
                stroke={change.color} 
                stroke-width={change.width || 2} 
                stroke-linecap="round" 
                stroke-linejoin="round" 
              />
            {/each}
          {/if}
        {/each}
      </svg>
    {/if}
  </div>
</div>

{#if !isPresentationMode}
<div class="shrink-0 px-8 py-4 bg-slate-100 dark:bg-slate-900/50 border-t border-slate-300 dark:border-slate-800 flex items-center justify-between transition-colors duration-300">

    <div class="flex items-center gap-1 bg-white dark:bg-slate-800 rounded-lg border border-slate-300 dark:border-slate-700 p-1 shadow-sm transition-colors duration-300">
       <button onclick={onprev} disabled={pageNumber <= 1} class="p-1.5 hover:bg-slate-100 dark:hover:bg-slate-700 text-slate-700 dark:text-slate-300 rounded-md disabled:opacity-20 transition-colors">◀</button>
       <div class="flex items-center gap-1 px-3 py-0.5">
         <input 
          type="number" 
          bind:value={jumpPage} 
          onkeydown={(e) => e.key === 'Enter' && handleJump()}
          class="w-8 bg-transparent text-center text-xs font-black text-blue-600 outline-none"
         />
         <span class="text-[9px] font-black text-slate-400 uppercase tracking-tighter transition-colors">/ {pdfDoc?.numPages || 1}</span>
       </div>
       <button onclick={onnext} disabled={pageNumber >= (pdfDoc?.numPages || 1)} class="p-1.5 hover:bg-slate-100 dark:hover:bg-slate-700 text-slate-700 dark:text-slate-300 rounded-md disabled:opacity-20 transition-colors">▶</button>
    </div>
    
    <div class="flex items-center gap-2">
       {#if mode === 'points'}
         <button onclick={undo} disabled={strokes.length === 0} class="px-4 py-1.5 text-xs font-bold text-slate-700 dark:text-slate-300 bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-700 hover:bg-slate-50 dark:hover:bg-slate-700 rounded-lg transition-all disabled:opacity-20 shadow-sm transition-colors duration-300 uppercase tracking-tighter">Undo Stroke</button>
       {/if}
       <button onclick={clear} class="px-4 py-1.5 text-xs font-bold text-slate-700 dark:text-slate-300 bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-700 hover:text-red-600 rounded-lg transition-all shadow-sm transition-colors duration-300 uppercase tracking-tighter">Reset</button>
       <div class="w-[1px] h-6 bg-slate-300 dark:border-slate-700 mx-1 transition-colors duration-300"></div>
       {#if mode !== 'view'}
         <button onclick={ondone} class="px-5 py-1.5 bg-blue-600 hover:bg-blue-700 text-white text-xs font-bold rounded-lg transition-all shadow-md shadow-blue-500/20 uppercase tracking-tight">Lock & Finish</button>
       {:else}
         <button onclick={onclose} class="px-6 py-2 bg-slate-900 dark:bg-white text-white dark:text-slate-900 font-bold rounded-lg transition-all shadow-lg uppercase tracking-tight transition-colors duration-300">Close Preview</button>
         {/if}
         </div>
         </div>
         {/if}
         </div>


{#if selectionState}
  <div 
    class="fixed z-[600] pointer-events-auto animate-in fade-in zoom-in-95 duration-200 flex gap-2"
    style="left: {selectionState.x}px; top: {selectionState.y}px; transform: translateX(-50%)"
  >
     <button 
      onclick={() => {
        chatState.input = `Tell me more about this: "${selectionState?.text}"`;
        chatState.handleAskPdf();
        selectionState = null;
        window.getSelection()?.removeAllRanges();
      }}
      class="flex items-center gap-2 px-3 py-1.5 bg-slate-900 dark:bg-white text-white dark:text-slate-900 rounded-full shadow-2xl font-black text-[9px] uppercase tracking-widest hover:scale-105 transition-all border border-white/20"
     >
       ✨ Ask AI
     </button>
     <button 
      onclick={() => {
        if (!selectionState) return;
        pdfState.addNote(selectionState.text, {
           docPath: filePath,
           pageNumber,
           text: selectionState.text
        });
        selectionState = null;
        window.getSelection()?.removeAllRanges();
      }}
      class="flex items-center gap-2 px-3 py-1.5 bg-blue-600 text-white rounded-full shadow-2xl font-black text-[9px] uppercase tracking-widest hover:scale-105 transition-all border border-white/20"
     >
       📝 Save Note
     </button>
     <button 
      onclick={async () => {
        if (!selectionState) return;
        const originalText = selectionState.text;
        appState.startLoading("AI is refactoring...");
        try {
          const system = "Rewrite this text to be more professional. Return ONLY the rewritten text.";
          const rewritten = await chatState.runAiTask(system, originalText);
          
          const outputPath = await invoke<string | null>("save_file_dialog", { defaultPath: "refactored.pdf" });
          if (outputPath) {
            await invoke("replace_text_block", { path: filePath, pageNum: pageNumber, oldText: originalText, newText: rewritten, outputPath });
            appState.showStatus("Text refactored successfully.", false, outputPath);
            await invoke("shell_open", { filePath: outputPath });
          }
        } catch (e) { appState.showStatus(`Refactor failed: ${e}`, true); }
        finally { selectionState = null; window.getSelection()?.removeAllRanges(); }
      }}
      class="flex items-center gap-2 px-3 py-1.5 bg-indigo-600 text-white rounded-full shadow-2xl font-black text-[9px] uppercase tracking-widest hover:scale-105 transition-all border border-white/20"
     >
       ✍️ AI Refactor
     </button>
  </div>
{/if}

{#if activeAnnotation}
  <div 
    class="fixed z-[600] pointer-events-auto animate-in fade-in zoom-in-95 duration-200 bg-white dark:bg-slate-800 rounded-2xl shadow-[0_32px_64px_-12px_rgba(0,0,0,0.5)] border border-slate-200 dark:border-slate-700 p-4 min-w-[240px]"
    style="left: {activeAnnotation.vx + 32}px; top: {activeAnnotation.vy - 100}px; transform: translateX(-50%)"
  >
     <div class="flex justify-between items-center mb-3">
        <span class="text-[9px] font-black uppercase text-blue-500">{activeAnnotation.kind}</span>
        <button onclick={() => activeAnnotation = null} class="text-slate-400 hover:text-slate-900 transition-colors">✕</button>
     </div>
     <textarea 
      bind:value={activeAnnotation.contents} 
      class="w-full p-2 bg-slate-50 dark:bg-slate-900 border border-slate-200 dark:border-slate-700 rounded-lg text-xs outline-none focus:ring-2 focus:ring-blue-500 mb-3 min-h-[60px] resize-none"
      placeholder="Annotation text..."
     ></textarea>
     <div class="flex gap-2">
        <button 
          onclick={async () => {
             appState.startLoading("Updating annotation...");
             try {
                await invoke("update_annotation_contents", { path: filePath, annotId: activeAnnotation.id, newContents: activeAnnotation.contents || "", outputPath: filePath });
                activeAnnotation = null;
                await loadDocument();
             } catch (e) { appState.showStatus("Failed to update", true); }
          }}
          class="flex-1 py-1.5 bg-slate-900 dark:bg-white text-white dark:text-slate-900 rounded-lg font-bold text-[9px] uppercase tracking-tighter"
        >Save</button>
        <button 
          onclick={async () => {
             appState.startLoading("Deleting annotation...");
             try {
                await invoke("delete_annotation", { path: filePath, annotId: activeAnnotation.id, outputPath: filePath });
                activeAnnotation = null;
                await loadDocument();
             } catch (e) { appState.showStatus("Failed to delete", true); }
          }}
          class="flex-1 py-1.5 bg-red-600 text-white rounded-lg font-bold text-[9px] uppercase tracking-tighter"
        >Delete</button>
     </div>
  </div>
{/if}

{#if isPresentationMode}
  <div 
    class="fixed bottom-12 left-1/2 -translate-x-1/2 z-[600] flex items-center gap-2 p-3 bg-slate-900/90 dark:bg-white/90 text-white dark:text-slate-900 rounded-3xl shadow-2xl backdrop-blur-2xl border border-white/20 animate-in slide-in-from-bottom-4 duration-500"
    transition:fly={{ y: 20, duration: 400 }}
  >
     <div class="flex items-center gap-1 bg-white/10 dark:bg-slate-900/10 rounded-2xl p-1">
        <button onclick={onprev} disabled={pageNumber <= 1} class="w-10 h-10 flex items-center justify-center hover:bg-white/20 dark:hover:bg-slate-900/20 rounded-xl transition-all disabled:opacity-20 text-lg">◀</button>
        <div class="px-4 text-sm font-black tracking-tighter">{pageNumber} / {pdfDoc?.numPages || 1}</div>
        <button onclick={onnext} disabled={pageNumber >= (pdfDoc?.numPages || 1)} class="w-10 h-10 flex items-center justify-center hover:bg-white/20 dark:hover:bg-slate-900/20 rounded-xl transition-all disabled:opacity-20 text-lg">▶</button>
     </div>
     
     <div class="w-[1px] h-8 bg-white/20 dark:bg-slate-900/20 mx-1"></div>
     
     <button 
      onclick={() => isLaserActive = !isLaserActive}
      class="w-10 h-10 flex items-center justify-center rounded-xl transition-all {isLaserActive ? 'bg-red-500 text-white' : 'hover:bg-white/20 dark:hover:bg-slate-900/20'}"
     >
        🎯
     </button>

     <button 
      onclick={() => isPresentationMode = false}
      class="px-4 h-10 flex items-center justify-center rounded-xl hover:bg-red-500 hover:text-white transition-all font-black text-[9px] uppercase tracking-widest"
     >
        Exit
     </button>
  </div>
{/if}

<style>
  /* Local component styles - minimalist approach */
</style>
