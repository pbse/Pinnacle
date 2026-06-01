<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { pdfState } from "$lib/state/pdfState.svelte";
  import { chatState } from "$lib/state/chatState.svelte";
  import { appState } from "$lib/state/appState.svelte";
  import ToolPane from "./ToolPane.svelte";
  import KnowledgeGraph from "./KnowledgeGraph.svelte";
  import NotepadPane from "./NotepadPane.svelte";

  let activeTab = $state<'chat' | 'graph' | 'notes' | 'tools'>('chat');

  // Batch Extraction State
  let batchExtractFiles = $state<{ path: string, status: 'pending' | 'processing' | 'done' | 'error', data?: any }[]>([]);
  let batchExtractResults = $state<any[]>([]);
  let documentReferences = $state<string[]>([]);
  let isBatchExtracting = $state(false);
  let batchSchema = $state("{\n  \"vendor\": \"string\",\n  \"total\": \"number\",\n  \"date\": \"string\"\n}");

  function handleCitationClick(page: number, context: string) {
    pdfState.viewerPageNumber = page;
    pdfState.viewerFilePath = pdfState.viewerFilePath || "";
    pdfState.highlightedSnippet = context;
    setTimeout(() => { if(pdfState.highlightedSnippet === context) pdfState.highlightedSnippet = null; }, 5000);
  }

  async function handleScanReferences() {
    if (!pdfState.viewerFilePath) return;
    appState.startLoading("Scanning for external references...");
    try {
      const text = await invoke<string>("pdf_to_text_string", { path: pdfState.viewerFilePath });
      const urlRegex = /(https?:\/\/[^\s]+)/g;
      const matches = text.match(urlRegex) || [];
      documentReferences = [...new Set(matches)];
      if (documentReferences.length === 0) appState.showStatus("No external references found.", false);
    } catch (e) { appState.showStatus("Scan failed.", true); }
  }

  async function selectBatchFiles() {
    const result = await invoke<string[]>("open_file_dialog", { multiple: true });
    if (result && result.length > 0) batchExtractFiles = result.map(p => ({ path: p, status: 'pending' }));
  }

  async function handleBatchExtract() {
    if (batchExtractFiles.length === 0) return;
    isBatchExtracting = true;
    batchExtractResults = [];
    for (let i = 0; i < batchExtractFiles.length; i++) {
      batchExtractFiles[i].status = 'processing';
      try {
        let pdfText = await invoke<string>("pdf_to_text_string", { path: batchExtractFiles[i].path });
        if (pdfText.length > 50000) pdfText = pdfText.substring(0, 50000);
        const system = `Extract data matching this JSON SCHEMA: ${batchSchema}. Return ONLY JSON.`;
        const result = await chatState.runAiTask(system, `TEXT:\n${pdfText}`, { json: true });
        const data = JSON.parse(result);
        batchExtractFiles[i].data = data;
        batchExtractFiles[i].status = 'done';
        batchExtractResults = [...batchExtractResults, { filename: batchExtractFiles[i].path.split(/[/\\]/).pop(), ...data }];
      } catch (e) { batchExtractFiles[i].status = 'error'; }
    }
    isBatchExtracting = false;
  }

  function exportBatchToCsv() {
    if (batchExtractResults.length === 0) return;
    const headers = Object.keys(batchExtractResults[0]);
    const csvContent = [headers.join(","), ...batchExtractResults.map(r => headers.map(h => `"${String(r[h]).replace(/"/g, '""')}"`).join(","))].join("\n");
    invoke<string | null>("save_file_dialog", { defaultPath: "batch_results.csv" }).then(path => {
      if (path) invoke("write_text_file", { path, contents: csvContent }).then(() => invoke("shell_open", { filePath: path }));
    });
  }

  async function handleBatchPdfToImages() {
    if (batchExtractFiles.length === 0) return;
    const baseDir = await invoke<string | null>("save_file_dialog", { defaultPath: "Exported_Images" });
    if (!baseDir) return;

    isBatchExtracting = true;
    appState.startLoading("Exporting PDFs to high-res images...");
    try {
      for (const file of batchExtractFiles) {
        const fileName = file.path.split(/[/\\]/).pop() || "doc";
        const fileOutputDir = `${baseDir}_${fileName.replace(/\.pdf$/i, "")}`;
        await invoke("pdf_to_images", { path: file.path, outputDir: fileOutputDir, format: "png" });
      }
      appState.showStatus("Batch image export completed successfully.", false, baseDir);
    } catch (e) {
      appState.showStatus(`Batch export failed: ${e}`, true);
    } finally {
      isBatchExtracting = false;
    }
  }

  async function handleExportToWord() {
    if (!pdfState.viewerFilePath) return;
    const outputPath = await invoke<string | null>("save_file_dialog", { defaultPath: "document.docx" });
    if (!outputPath) return;

    appState.startLoading("Converting PDF to Word (.docx)...");
    try {
      await invoke("pdf_to_docx", { path: pdfState.viewerFilePath, outputPath });
      appState.showStatus("Word document exported successfully.", false, outputPath);
      await invoke("shell_open", { filePath: outputPath });
    } catch (e) { appState.showStatus(`Word export failed: ${e}`, true); }
  }

  async function handleResearchReport() {
    if (!pdfState.viewerFilePath) return;
    appState.startLoading("Compiling research report...");
    try {
      let report = `# Research Report: ${pdfState.viewerFilePath.split(/[/\\]/).pop()}\n\n`;
      report += `## Executive Summary\nAnalyzed with Private AI Assistant.\n\n`;
      const annots = await invoke<any[]>("get_annotations", { path: pdfState.viewerFilePath });
      if (annots.length > 0) {
        report += `## Annotations\n`;
        for (const a of annots) report += `- **Page ${a.page}**: ${a.contents || 'No comment'}\n`;
      }
      const outputPath = await invoke<string | null>("save_file_dialog", { defaultPath: "research_report.md" });
      if (outputPath) {
        await invoke("write_text_file", { path: outputPath, contents: report });
        await invoke("shell_open", { filePath: outputPath });
        appState.showStatus("Report exported.", false);
      }
    } catch (e) { appState.showStatus("Report failed.", true); }
  }
  function extractLastSentence(text: string): string {
    if (!text) return "";
    const sentences = text.split(/[.!?]+/).map(s => s.trim()).filter(s => s.length > 5);
    return sentences.length > 0 ? sentences[sentences.length - 1] : text.trim();
  }
</script>

<ToolPane title="Assistant" subtitle="AI Intelligence">
  <div class="flex flex-col h-full overflow-hidden">
    <!-- Modern Tab Header -->
    <div class="flex items-center gap-1 p-1 bg-slate-50 dark:bg-slate-900/50 border border-slate-200 dark:border-slate-800 rounded-xl mb-4">
      {#each ['chat', 'graph', 'notes', 'tools'] as tab}
        <button 
          onclick={() => activeTab = tab as any}
          class="flex-1 py-1.5 text-[9px] font-black uppercase tracking-widest rounded-lg transition-all {activeTab === tab ? 'bg-white dark:bg-slate-800 text-blue-600 shadow-sm border border-slate-200 dark:border-slate-700' : 'text-slate-400 hover:text-slate-600'}"
        >
          {tab}
        </button>
      {/each}
    </div>

    <div class="flex-1 overflow-hidden relative">
      {#if activeTab === 'chat'}
        <div class="absolute inset-0 flex flex-col">
          <div class="flex-1 overflow-y-auto space-y-4 pr-2 mb-4 no-scrollbar scroll-smooth">
            {#if chatState.history.length === 0 && !chatState.isChatting}
              <div class="flex flex-col items-center justify-center h-full text-center opacity-40">
                <div class="text-3xl mb-4">💬</div>
                <p class="text-[10px] font-bold uppercase tracking-widest">Assistant is ready</p>
                <p class="text-[9px] mt-2 max-w-[160px]">Ask anything about your document. All processing is local.</p>
              </div>
            {/if}

            {#each chatState.history as msg}
              <div class="group relative">
                <div class="text-[8px] font-black uppercase tracking-tighter mb-1 {msg.role === 'user' ? 'text-blue-600 text-right mr-4' : 'text-slate-400 ml-4'}">
                  {msg.role}
                </div>
                <div class="text-[11px] p-3 rounded-2xl shadow-sm leading-relaxed transition-all {msg.role === 'user' ? 'bg-blue-600 text-white ml-8' : 'bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-200 mr-8'}">
                  {#if msg.role === 'assistant'}
                    {@const parts = msg.content.split(/(\[p\. \d+\])/g)}
                    {#each parts as part, index}
                        {#if part.match(/\[p\. (\d+)\]/)}
                          {@const pageNum = parseInt(part.match(/\d+/)![0])}
                          {@const prevPart = index > 0 ? parts[index - 1] : ""}
                          {@const lastSentence = extractLastSentence(prevPart)}
                          <button onclick={() => handleCitationClick(pageNum, lastSentence)} class="text-blue-500 font-bold hover:underline">
                            {part}
                          </button>
                        {:else}
                          {part}
                        {/if}
                    {/each}
                  {:else}
                    {msg.content}
                  {/if}
                </div>
              </div>
            {/each}
            {#if chatState.isChatting}
              <div class="flex gap-1 ml-4 py-2">
                <div class="w-1.5 h-1.5 bg-blue-500 rounded-full animate-bounce"></div>
                <div class="w-1.5 h-1.5 bg-blue-500 rounded-full animate-bounce [animation-delay:0.2s]"></div>
                <div class="w-1.5 h-1.5 bg-blue-500 rounded-full animate-bounce [animation-delay:0.4s]"></div>
              </div>
            {/if}
          </div>

          <div class="shrink-0 space-y-3">
            <div class="flex items-center justify-between px-2">
              <button 
                onclick={() => !pdfState.viewerFilePath ? pdfState.selectFile('extract') : handleResearchReport()} 
                class="text-[8px] font-black text-blue-500 uppercase tracking-tighter"
              >
                {!pdfState.viewerFilePath ? 'Select PDF' : 'Export MD Report'}
              </button>
              <select bind:value={chatState.aiProvider} class="bg-transparent text-[8px] font-black text-slate-400 uppercase tracking-tighter outline-none cursor-pointer">
                <option value="ollama">Ollama</option>
                <option value="webllm">In-App AI</option>
              </select>
            </div>
            <div class="relative">
              <input 
                type="text" 
                bind:value={chatState.input} 
                onkeydown={(e) => e.key === 'Enter' && chatState.handleAskPdf()}
                placeholder="Type your message..." 
                class="w-full pl-4 pr-12 py-3 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-2xl text-xs outline-none focus:ring-2 focus:ring-blue-500 transition-all shadow-inner"
              />
              <button 
                onclick={() => chatState.handleAskPdf()}
                class="absolute right-2 top-2 w-8 h-8 bg-blue-600 text-white rounded-xl flex items-center justify-center shadow-lg hover:scale-105 active:scale-95 transition-all"
              >
                ↑
              </button>
            </div>
          </div>
        </div>
      {:else if activeTab === 'graph'}
        <div class="absolute inset-0">
          <div class="flex items-center justify-between mb-4 px-2">
            <h3 class="text-[10px] font-black text-slate-400 uppercase tracking-widest">Entity Relationship</h3>
            <span class="text-[8px] px-2 py-0.5 bg-blue-100 dark:bg-blue-900/30 text-blue-600 rounded-full font-black uppercase">Live</span>
          </div>
          <div class="h-[calc(100%-40px)]">
            <KnowledgeGraph />
          </div>
        </div>
      {:else if activeTab === 'notes'}
        <div class="absolute inset-0">
          <NotepadPane />
        </div>
      {:else if activeTab === 'tools'}
        <div class="absolute inset-0 p-4 overflow-y-auto no-scrollbar space-y-8">
          <section class="space-y-4">
            <h3 class="text-[10px] font-black text-slate-400 uppercase tracking-[0.2em] border-b border-slate-100 dark:border-slate-800 pb-2">Batch Extraction</h3>
            <div class="space-y-3">
              <button onclick={selectBatchFiles} class="w-full py-2 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl text-[10px] font-bold uppercase tracking-widest shadow-sm">Select Files</button>
              {#if batchExtractFiles.length > 0}
                  <textarea bind:value={batchSchema} class="w-full p-3 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl text-[10px] font-mono h-24 outline-none focus:ring-2 focus:ring-blue-500"></textarea>
                  <button onclick={handleBatchExtract} disabled={isBatchExtracting} class="w-full py-2.5 bg-blue-600 text-white rounded-xl font-black text-[10px] uppercase tracking-widest shadow-lg shadow-blue-500/20">
                    {batchExtractFiles.length === 0 ? 'Select Files' : isBatchExtracting ? 'Extracting...' : 'Run Intelligence Batch'}
                  </button>
                  <button onclick={handleBatchPdfToImages} disabled={isBatchExtracting} class="w-full py-2 border border-slate-200 dark:border-slate-800 rounded-xl text-[10px] font-bold uppercase tracking-widest hover:border-blue-500 transition-colors">
                    Export as High-Res Images
                  </button>
                  {#if batchExtractResults.length > 0}
                    <button onclick={exportBatchToCsv} class="w-full py-2 border-2 border-green-500 text-green-600 font-black text-[10px] uppercase tracking-widest rounded-xl">Download CSV Results</button>
                  {/if}
              {/if}
            </div>
          </section>

          <section class="space-y-4">
            <h3 class="text-[10px] font-black text-slate-400 uppercase tracking-[0.2em] border-b border-slate-100 dark:border-slate-800 pb-2">Advanced Intelligence</h3>
            <div class="grid grid-cols-2 gap-3">
                <button onclick={() => !pdfState.viewerFilePath ? pdfState.selectFile('extract') : handleScanReferences()} class="p-3 bg-slate-50 dark:bg-slate-900 rounded-2xl border border-slate-100 dark:border-slate-800 hover:border-blue-500 transition-all text-left group">
                  <div class="text-lg mb-1 group-hover:scale-110 transition-transform">🔗</div>
                  <div class="text-[9px] font-black uppercase text-slate-500">{!pdfState.viewerFilePath ? 'Select PDF' : 'Scan URLs'}</div>
                </button>
                <button onclick={() => !pdfState.viewerFilePath ? pdfState.selectFile('extract') : (pdfState.ocrTrigger = Date.now())} class="p-3 bg-slate-50 dark:bg-slate-900 rounded-2xl border border-slate-100 dark:border-slate-800 hover:border-blue-500 transition-all text-left group">
                  <div class="text-lg mb-1 group-hover:scale-110 transition-transform">👁️</div>
                  <div class="text-[9px] font-black uppercase text-slate-500">{!pdfState.viewerFilePath ? 'Select PDF' : 'Force OCR'}</div>
                </button>
                <button onclick={handleExportToWord} class="p-3 bg-slate-50 dark:bg-slate-900 rounded-2xl border border-slate-100 dark:border-slate-800 hover:border-blue-500 transition-all text-left group">
                  <div class="text-lg mb-1 group-hover:scale-110 transition-transform">📝</div>
                  <div class="text-[9px] font-black uppercase text-slate-500">Export Word</div>
                </button>
            </div>
          </section>
        </div>
      {/if}
    </div>
  </div>
</ToolPane>

<style>
  .no-scrollbar::-webkit-scrollbar { display: none; }
  .no-scrollbar { -ms-overflow-style: none; scrollbar-width: none; }
</style>
