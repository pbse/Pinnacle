<script lang="ts">
  import { onMount } from "svelte";
  import { pdfState } from "$lib/state/pdfState.svelte";
  import { appState } from "$lib/state/appState.svelte";

  let canvas: HTMLCanvasElement | undefined = $state();
  let ctx: CanvasRenderingContext2D | null = null;
  let isDrawing = false;
  let strokes: [number, number][][] = $state([]);
  let currentStroke: [number, number][] = $state([]);

  // Style properties
  let strokeColor = $state("#0f172a");
  let strokeWidth = $state(3);

  const colors = [
    { name: "Charcoal", hex: "#0f172a" },
    { name: "Navy", hex: "#1e3a8a" },
    { name: "Crimson", hex: "#991b1b" },
    { name: "Emerald", hex: "#065f46" }
  ];

  onMount(() => {
    if (canvas) {
      ctx = canvas.getContext("2d");
      resizeCanvas();
    }
  });

  function resizeCanvas() {
    if (canvas && ctx) {
      // Get the CSS size of the canvas container
      const rect = canvas.parentElement?.getBoundingClientRect();
      canvas.width = (rect?.width || 600) - 32; // padding
      canvas.height = 300;
      
      // Canvas styling for smooth lines
      ctx.lineCap = "round";
      ctx.lineJoin = "round";
      redraw();
    }
  }

  function getMousePos(e: MouseEvent | TouchEvent) {
    if (!canvas) return { x: 0, y: 0 };
    const rect = canvas.getBoundingClientRect();
    
    if ("touches" in e) {
      if (e.touches.length === 0) return { x: 0, y: 0 };
      return {
        x: e.touches[0].clientX - rect.left,
        y: e.touches[0].clientY - rect.top
      };
    } else {
      return {
        x: e.clientX - rect.left,
        y: e.clientY - rect.top
      };
    }
  }

  function startDrawing(e: MouseEvent | TouchEvent) {
    e.preventDefault();
    isDrawing = true;
    const pos = getMousePos(e);
    currentStroke = [[pos.x, pos.y]];
    
    if (ctx) {
      ctx.strokeStyle = strokeColor;
      ctx.lineWidth = strokeWidth;
      ctx.beginPath();
      ctx.moveTo(pos.x, pos.y);
    }
  }

  function draw(e: MouseEvent | TouchEvent) {
    if (!isDrawing || !ctx) return;
    e.preventDefault();
    const pos = getMousePos(e);
    currentStroke = [...currentStroke, [pos.x, pos.y]];
    
    ctx.lineTo(pos.x, pos.y);
    ctx.stroke();
  }

  function stopDrawing() {
    if (!isDrawing) return;
    isDrawing = false;
    if (currentStroke.length > 0) {
      strokes = [...strokes, currentStroke];
      currentStroke = [];
    }
  }

  function clear() {
    strokes = [];
    currentStroke = [];
    if (canvas && ctx) {
      ctx.clearRect(0, 0, canvas.width, canvas.height);
    }
  }

  function undo() {
    if (strokes.length > 0) {
      strokes = strokes.slice(0, -1);
      redraw();
    }
  }

  function redraw() {
    if (!canvas || !ctx) return;
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    
    for (const stroke of strokes) {
      if (stroke.length === 0) continue;
      ctx.strokeStyle = strokeColor;
      ctx.lineWidth = strokeWidth;
      ctx.beginPath();
      ctx.moveTo(stroke[0][0], stroke[0][1]);
      for (let i = 1; i < stroke.length; i++) {
        ctx.lineTo(stroke[i][0], stroke[i][1]);
      }
      ctx.stroke();
    }
  }

  function handleDone() {
    if (strokes.length === 0) {
      appState.showStatus("Please draw a signature first.", true);
      return;
    }

    // Calculate bounding box of strokes to normalize coordinates
    const allPoints = strokes.flat();
    const xs = allPoints.map(p => p[0]);
    const ys = allPoints.map(p => p[1]);
    
    const minX = Math.min(...xs);
    const maxX = Math.max(...xs);
    const minY = Math.min(...ys);
    const maxY = Math.max(...ys);
    
    const width = maxX - minX;
    const height = maxY - minY;
    
    if (width <= 0 || height <= 0) {
      appState.showStatus("Signature drawing is too small.", true);
      return;
    }

    // Normalize strokes to 0..1 scale
    const normalizedStrokes = strokes.map(stroke => 
      stroke.map(p => [
        (p[0] - minX) / width,
        (p[1] - minY) / height
      ] as [number, number])
    );

    const aspectRatio = width / height;

    // Set the global active stamp
    pdfState.activeStamp = {
      strokes: normalizedStrokes,
      aspectRatio,
      width: 150,
      height: 150 / aspectRatio
    };

    pdfState.showSignPad = false;
    appState.showStatus("Signature captured! Drag and place it on the document.", false);
  }

  function handleCancel() {
    pdfState.showSignPad = false;
  }
</script>

<svelte:window onresize={resizeCanvas} />

<div class="fixed inset-0 z-[1000] flex items-center justify-center bg-slate-900/60 dark:bg-black/80 backdrop-blur-md p-4">
  <!-- Neubrutalist Modal Container -->
  <div 
    class="w-full max-w-2xl bg-white dark:bg-slate-900 border-4 border-slate-950 dark:border-white rounded-none shadow-[8px_8px_0px_0px_rgba(15,23,42,1)] dark:shadow-[8px_8px_0px_0px_rgba(255,255,255,1)] overflow-hidden flex flex-col transition-colors duration-300"
  >
    <!-- Modal Header -->
    <div class="bg-slate-950 text-white dark:bg-white dark:text-slate-950 px-6 py-4 flex justify-between items-center border-b-4 border-slate-950 dark:border-white">
      <h2 class="text-lg font-black uppercase tracking-widest">Draw Signature</h2>
      <button 
        onclick={handleCancel} 
        class="text-xl font-bold hover:scale-110 active:scale-95 transition-transform"
        aria-label="Close modal"
      >
        ✕
      </button>
    </div>

    <!-- Modal Body -->
    <div class="p-6 space-y-6 flex-1 flex flex-col min-h-0">
      <!-- Toolbar controls -->
      <div class="flex flex-wrap items-center justify-between gap-4">
        <!-- Colors -->
        <div class="flex items-center gap-3">
          <span class="text-xs font-black uppercase tracking-wider text-slate-500 dark:text-slate-400">Color:</span>
          <div class="flex gap-2">
            {#each colors as color}
              <button
                onclick={() => strokeColor = color.hex}
                class="w-6 h-6 rounded-full border-2 transition-transform hover:scale-110 active:scale-95 {strokeColor === color.hex ? 'border-slate-950 dark:border-white scale-110' : 'border-transparent'}"
                style="background-color: {color.hex}"
                title={color.name}
              ></button>
            {/each}
          </div>
        </div>

        <!-- Thickness -->
        <div class="flex items-center gap-3">
          <span class="text-xs font-black uppercase tracking-wider text-slate-500 dark:text-slate-400">Thickness:</span>
          <div class="flex items-center gap-2">
            <input 
              type="range" 
              min="1" 
              max="8" 
              bind:value={strokeWidth} 
              class="w-24 h-2 bg-slate-200 dark:bg-slate-700 rounded-lg appearance-none cursor-pointer accent-slate-950 dark:accent-white"
            />
            <span class="text-xs font-mono font-bold w-4 text-slate-700 dark:text-slate-300">{strokeWidth}px</span>
          </div>
        </div>
      </div>

      <!-- Canvas Box in Neubrutalist style -->
      <div class="relative bg-slate-50 dark:bg-slate-950 border-2 border-slate-950 dark:border-slate-800 flex-1 min-h-[300px] flex items-center justify-center overflow-hidden">
        <canvas
          bind:this={canvas}
          onmousedown={startDrawing}
          onmousemove={draw}
          onmouseup={stopDrawing}
          onmouseleave={stopDrawing}
          ontouchstart={startDrawing}
          ontouchmove={draw}
          ontouchend={stopDrawing}
          class="absolute inset-0 w-full h-full cursor-crosshair"
        ></canvas>
        {#if strokes.length === 0}
          <div class="absolute pointer-events-none text-slate-400 dark:text-slate-600 font-bold uppercase tracking-widest text-sm flex flex-col items-center gap-2">
            <span>✍️ Draw signature here</span>
            <span class="text-[10px] font-medium text-slate-400/80">Supports mouse, trackpad, or touch</span>
          </div>
        {/if}
      </div>

      <!-- Actions -->
      <div class="flex items-center justify-between gap-4 pt-2">
        <div class="flex gap-2">
          <button 
            onclick={undo} 
            disabled={strokes.length === 0}
            class="px-4 py-2 border-2 border-slate-950 dark:border-slate-700 font-bold text-xs uppercase hover:bg-slate-100 dark:hover:bg-slate-800 disabled:opacity-30 transition-all"
          >
            Undo
          </button>
          <button 
            onclick={clear} 
            disabled={strokes.length === 0}
            class="px-4 py-2 border-2 border-red-500 text-red-500 font-bold text-xs uppercase hover:bg-red-50 dark:hover:bg-red-950/20 disabled:opacity-30 transition-all"
          >
            Clear
          </button>
        </div>

        <div class="flex gap-2">
          <button 
            onclick={handleCancel} 
            class="px-5 py-2.5 border-2 border-slate-950 dark:border-slate-700 font-bold text-xs uppercase hover:bg-slate-100 dark:hover:bg-slate-800 transition-all"
          >
            Cancel
          </button>
          <button 
            onclick={handleDone} 
            disabled={strokes.length === 0}
            class="px-6 py-2.5 bg-slate-950 text-white dark:bg-white dark:text-slate-950 font-black text-xs uppercase hover:scale-[1.03] active:scale-95 disabled:opacity-30 transition-all"
          >
            Done
          </button>
        </div>
      </div>
    </div>
  </div>
</div>
