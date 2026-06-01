<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { pdfState } from "$lib/state/pdfState.svelte";
  import { historyState } from "$lib/state/historyState.svelte";
  import { appState } from "$lib/state/appState.svelte";
  import { chatState } from "$lib/state/chatState.svelte";
  import { db } from "$lib/state/db";
  import { fly } from "svelte/transition";
  import ToolPane from "./ToolPane.svelte";
  import KnowledgeGraph from "./KnowledgeGraph.svelte";

  let metaTitle = $state("");
  let metaAuthor = $state("");
  let metaSubject = $state("");
  let metaKeywords = $state("");
  let selectedFiles = $state<Set<string>>(new Set());
  let isMultiSelect = $state(false);
  let selectedFile = $state<string | null>(null);
  let tagsInput = $state("");
  let chatInput: HTMLInputElement | undefined = $state();
  let libraryTab = $state<"collection" | "activity" | "stats" | "watchers" | "knowledge">("collection");
  let viewMode = $state<"list" | "grid">("list");
  let contextMenu = $state<{ x: number, y: number, file?: any, collection?: any } | null>(null);

  function handleContextMenu(e: MouseEvent, item: any, type: 'file' | 'collection') {
    e.preventDefault();
    if (type === 'file') contextMenu = { x: e.clientX, y: e.clientY, file: item };
    else contextMenu = { x: e.clientX, y: e.clientY, collection: item };
  }

  async function handleCreateCollection() {
    const name = prompt("Collection name:");
    if (name) await historyState.createCollection(name);
  }

  async function handleReveal(path: string) {
    await invoke("reveal_in_folder", { filePath: path });
    contextMenu = null;
  }

  async function handleDeleteFile(path: string) {
    const item = await db.documents.where('path').equals(path).first();
    if (item?.id) await db.documents.delete(item.id);
    await historyState.loadHistory();
    contextMenu = null;
  }

  function toggleSelect(path: string) {
    if (selectedFiles.has(path)) selectedFiles.delete(path);
    else selectedFiles.add(path);
    selectedFiles = new Set(selectedFiles);
  }

  async function handleBatchDelete() {
    if (selectedFiles.size === 0) return;
    for (const path of selectedFiles) {
      const item = await db.documents.where('path').equals(path).first();
      if (item?.id) await db.documents.delete(item.id);
    }
    await historyState.loadHistory();
    selectedFiles = new Set();
    isMultiSelect = false;
    appState.showStatus(`Deleted documents.`, false);
  }

  async function handleSuggestTags() {
    if (!selectedFile) return;
    appState.startLoading("AI analyzing document...");
    const suggested = await chatState.suggestTags(selectedFile);
    if (suggested.length > 0) {
      tagsInput = [...new Set([...tagsInput.split(",").map(t => t.trim()).filter(t => t), ...suggested])].join(", ");
      appState.showStatus("AI suggested new tags.", false);
    } else {
      appState.showStatus("AI could not determine tags.", true);
    }
  }

  async function handleBatchTag() {
    if (selectedFiles.size === 0) return;
    const tags = prompt("Tags to add (comma separated):");
    if (!tags) return;
    const tagList = tags.split(",").map(t => t.trim()).filter(t => t);
    for (const path of selectedFiles) {
      const item = await db.documents.where('path').equals(path).first();
      if (item) {
        const newTags = [...new Set([...(item.tags || []), ...tagList])];
        await historyState.updateTags(path, newTags);
      }
    }
    selectedFiles = new Set();
    isMultiSelect = false;
    appState.showStatus("Batch tagging complete.", false);
  }

  async function handleBatchMetaStamp() {
    if (selectedFiles.size === 0) return;
    const author = prompt("Common Author Name:");
    if (author === null) return;
    appState.startLoading("Stamping batch metadata...");
    try {
      await invoke("batch_update_metadata", {
        paths: Array.from(selectedFiles),
        title: null,
        author: author || null,
        subject: null,
        keywords: null
      });
      appState.showStatus(`Stamped ${selectedFiles.size} documents.`, false);
      selectedFiles = new Set();
      isMultiSelect = false;
    } catch (e) { appState.showStatus(`Stamp failed: ${e}`, true); }
  }

  async function handleClusterLibrary() {
    if (historyState.recentFiles.length < 2) return;
    appState.startLoading("AI architect is clustering your library...");
    try {
      const docsToCluster = historyState.recentFiles.map(f => ({
        path: f.path,
        name: f.name,
        summary: f.summary || "General Document"
      }));
      const clusters = await chatState.clusterDocuments(docsToCluster);
      
      for (const cluster of clusters) {
         // Create collection
         const collId = await db.collections.add({ name: `✨ ${cluster.name}`, timestamp: Date.now() });
         // Add documents to it
         for (const path of cluster.docPaths) {
            await historyState.addToCollection(path, collId);
         }
      }
      await historyState.loadCollections();
      await historyState.loadHistory();
      appState.showStatus(`Generated ${clusters.length} semantic clusters.`, false);
    } catch (e) { appState.showStatus("Clustering failed.", true); }
  }

  async function handleUpdateMetadata() {
    if (!selectedFile) return;
    const outputPath = await invoke<string | null>("save_file_dialog", { defaultPath: "updated_meta.pdf" });
    if (!outputPath) return;
    
    appState.startLoading("Updating Metadata...");
    try {
      await invoke("update_metadata", {
        path: selectedFile,
        title: metaTitle || null,
        author: metaAuthor || null,
        subject: metaSubject || null,
        keywords: metaKeywords || null,
        outputPath
      });
      
      const tags = tagsInput.split(",").map(t => t.trim()).filter(t => t);
      historyState.updateTags(selectedFile, tags);

      appState.showStatus("Metadata & Tags updated successfully.", false, outputPath);
      await invoke("shell_open", { filePath: outputPath });
    } catch (e) { appState.showStatus(`Failed: ${e}`, true); }
  }

  async function handleExportBibtex() {
    if (!selectedFile) return;
    appState.startLoading("Generating citation...");
    try {
      const bib = await chatState.getBibtex(selectedFile);
      const outputPath = await invoke<string | null>("save_file_dialog", { defaultPath: "citation.bib" });
      if (outputPath) {
        await invoke("write_text_file", { path: outputPath, contents: bib });
        await invoke("shell_open", { filePath: outputPath });
        appState.showStatus("BibTeX citation exported.", false);
      }
    } catch (e) { appState.showStatus("Failed to generate citation.", true); }
  }

  function editFile(file: any) {
    selectedFile = file.path;
    metaTitle = ""; metaAuthor = ""; metaSubject = "";
    tagsInput = (file.tags || []).join(", ");
  }

  let searchQuery = $state("");
  let duplicateGroups = $state<any[][]>([]);
  let searchLayouts = $state<Record<string, any>>({});
  const loadingLayoutPaths = new Set<string>();
  
  $effect(() => {
    if (historyState.searchResults.length > 0) {
      historyState.searchResults.forEach(async (res) => {
        if (searchLayouts[res.path] || loadingLayoutPaths.has(res.path)) return;
        loadingLayoutPaths.add(res.path);
        const doc = await db.documents.where('path').equals(res.path).first();
        if (doc?.layoutJson) {
          try {
            searchLayouts = { ...searchLayouts, [res.path]: JSON.parse(doc.layoutJson) };
          } catch(e) {}
          loadingLayoutPaths.delete(res.path);
          return;
        }

        try {
          const layoutJson = await invoke<string>("pdf_to_layout_json", { path: res.path });
          const layout = JSON.parse(layoutJson);
          searchLayouts = { ...searchLayouts, [res.path]: layout };
          if (doc?.id) await db.documents.update(doc.id, { layoutJson });
        } catch (e) {
          console.error("Failed to build search heatmap:", e);
        } finally {
          loadingLayoutPaths.delete(res.path);
        }
      });
    }
  });

  function normalizeSearchText(value: string | undefined | null): string {
    return (value || "").toLowerCase().replace(/\s+/g, " ").trim();
  }

  function getPageText(pageData: any): string {
    return pageData?.text || "";
  }

  function getPageItems(pageData: any): any[] {
    return Array.isArray(pageData?.text_items) ? pageData.text_items : [];
  }

  function getItemText(item: any): string {
    return item?.text || item?.str || "";
  }

  function getMatchingPages(resPath: string, query: string): any[] {
    const layout = searchLayouts[resPath];
    if (!layout || !layout.pages || !query.trim()) return [];
    const needle = normalizeSearchText(query);
    return layout.pages.filter((p: any) => normalizeSearchText(getPageText(p)).includes(needle));
  }

  function getMatchingItems(pageData: any, query: string): any[] {
    const needle = normalizeSearchText(query);
    if (!needle) return [];
    return getPageItems(pageData).filter((item: any) => normalizeSearchText(getItemText(item)).includes(needle));
  }

  async function handleSearch() {
    await historyState.searchLibrary(searchQuery);
  }

  async function handleFindDuplicates() {
    appState.startLoading("Scanning for duplicates...");
    duplicateGroups = await historyState.findDuplicates();
    if (duplicateGroups.length === 0) appState.showStatus("No duplicate documents found.", false);
    else appState.showStatus(`Found ${duplicateGroups.length} duplicate groups.`, false);
  }
</script>

<ToolPane title="Library" subtitle="Knowledge Hub">
  <div class="space-y-8 h-full flex flex-col">
    <div class="flex gap-4 border-b border-slate-100 dark:border-slate-800 pb-2">
      <button onclick={() => libraryTab = 'collection'} class="text-[10px] font-black uppercase tracking-widest {libraryTab === 'collection' ? 'text-blue-600' : 'text-slate-400'}">Collection</button>
      <button onclick={() => libraryTab = 'activity'} class="text-[10px] font-black uppercase tracking-widest {libraryTab === 'activity' ? 'text-blue-600' : 'text-slate-400'}">Activity</button>
      <button onclick={() => libraryTab = 'stats'} class="text-[10px] font-black uppercase tracking-widest {libraryTab === 'stats' ? 'text-blue-600' : 'text-slate-400'}">Stats</button>
      <button onclick={() => libraryTab = 'watchers'} class="text-[10px] font-black uppercase tracking-widest {libraryTab === 'watchers' ? 'text-blue-600' : 'text-slate-400'}">Watchers</button>
      <button onclick={() => libraryTab = 'knowledge'} class="text-[10px] font-black uppercase tracking-widest {libraryTab === 'knowledge' ? 'text-blue-600' : 'text-slate-400'}">Graph</button>
    </div>

    {#if libraryTab === 'collection'}
      <div class="space-y-6 flex-1 overflow-y-auto pr-1">
        {#if pdfState.activeLoader}
          <div 
            class="p-4 bg-white/75 dark:bg-slate-900/75 backdrop-blur-xl border-2 border-slate-900 dark:border-slate-100 rounded-2xl shadow-[8px_8px_0px_0px_rgba(15,23,42,1.0)] dark:shadow-[8px_8px_0px_0px_rgba(255,255,255,0.95)] transition-all animate-in zoom-in-95 duration-200"
          >
            <div class="flex items-center justify-between mb-3">
              <span class="text-[9px] font-black uppercase text-blue-600 dark:text-blue-400 tracking-wider">File Import Loader</span>
              <span class="text-[8px] font-bold text-slate-400 uppercase">{pdfState.activeLoader.stage}</span>
            </div>
            <div class="text-[10px] font-bold text-slate-800 dark:text-slate-200 truncate mb-3">{pdfState.activeLoader.filename}</div>
            
            <div class="grid grid-cols-3 gap-2 text-center text-[7px] font-black uppercase tracking-wider mb-3">
              <div class="py-1 px-1.5 rounded-md border transition-all {pdfState.activeLoader.stage === 'converting' ? 'border-blue-500 bg-blue-500/10 text-blue-600 animate-pulse' : ['scanning', 'indexing', 'complete'].includes(pdfState.activeLoader.stage) ? 'border-green-500 bg-green-500/10 text-green-600' : 'border-slate-200 dark:border-slate-800 text-slate-400'}">
                1. Convert
              </div>
              <div class="py-1 px-1.5 rounded-md border transition-all {pdfState.activeLoader.stage === 'scanning' ? 'border-blue-500 bg-blue-500/10 text-blue-600 animate-pulse' : ['indexing', 'complete'].includes(pdfState.activeLoader.stage) ? 'border-green-500 bg-green-500/10 text-green-600' : 'border-slate-200 dark:border-slate-800 text-slate-400'}">
                2. Scan Layout
              </div>
              <div class="py-1 px-1.5 rounded-md border transition-all {pdfState.activeLoader.stage === 'indexing' ? 'border-blue-500 bg-blue-500/10 text-blue-600 animate-pulse' : pdfState.activeLoader.stage === 'complete' ? 'border-green-500 bg-green-500/10 text-green-600' : 'border-slate-200 dark:border-slate-800 text-slate-400'}">
                3. Index DB
              </div>
            </div>

            <div class="w-full h-2 bg-slate-100 dark:bg-slate-800 rounded-full overflow-hidden border border-slate-200 dark:border-slate-700">
              <div 
                class="h-full bg-blue-600 transition-all duration-300 ease-out" 
                style="width: {pdfState.activeLoader.progress}%"
              ></div>
            </div>
          </div>
        {/if}

        <div class="space-y-3">
          <div class="flex items-center bg-slate-50 dark:bg-slate-800 rounded-xl px-3 py-2 border border-slate-200 dark:border-slate-700 shadow-inner">
            <span class="text-xs mr-2">🔍</span>
            <input 
              bind:value={searchQuery}
              onkeydown={e => e.key === 'Enter' && handleSearch()}
              placeholder="Search entire library..." 
              class="bg-transparent outline-none text-xs text-slate-700 dark:text-slate-200 w-full"
            />
            {#if historyState.isSearching}
              <div class="w-3 h-3 border-2 border-blue-500 border-t-transparent rounded-full animate-spin"></div>
            {/if}
          </div>

          {#if historyState.searchResults.length > 0}
            <div class="space-y-4 p-4 bg-blue-50/50 dark:bg-blue-950/20 rounded-2xl border border-blue-100 dark:border-blue-900/40 animate-in fade-in zoom-in-95 duration-200">
              <h4 class="text-[9px] font-black text-blue-600 dark:text-blue-400 uppercase tracking-widest">Library Heatmap Matches</h4>
              {#each historyState.searchResults as res}
                <div class="p-3 bg-white dark:bg-slate-900 border border-slate-100 dark:border-slate-800 rounded-xl space-y-3">
                  <button 
                    onclick={() => pdfState.openTab(res.path)}
                    class="text-[10px] font-bold text-slate-700 dark:text-slate-200 truncate w-full text-left hover:text-blue-600 transition-colors uppercase tracking-tight"
                  >
                    📄 {res.file}
                  </button>
                  
                  {#if getMatchingPages(res.path, searchQuery).length > 0}
                    <div class="flex gap-3 overflow-x-auto py-1 no-scrollbar">
                      {#each getMatchingPages(res.path, searchQuery) as pageData}
                        {@const matchingItems = getMatchingItems(pageData, searchQuery)}
                        <button 
                          onclick={() => {
                            pdfState.openTab(res.path);
                            pdfState.viewerPageNumber = pageData.page;
                            pdfState.highlightedSnippet = searchQuery;
                          }}
                          class="shrink-0 flex flex-col items-center gap-1 group/thumb"
                        >
                          <div 
                            class="w-12 h-16 bg-slate-50 dark:bg-slate-950 border border-slate-200 dark:border-slate-800 relative rounded shadow-sm overflow-hidden transition-all group-hover/thumb:border-blue-500 shrink-0"
                            title="Page {pageData.page}: {getPageText(pageData).slice(0, 180)}"
                          >
                            <div class="absolute inset-1 flex flex-col gap-[2px] opacity-25">
                              <div class="h-[2px] bg-slate-400 rounded-full w-full"></div>
                              <div class="h-[2px] bg-slate-400 rounded-full w-4/5"></div>
                              <div class="h-[2px] bg-slate-400 rounded-full w-11/12"></div>
                              <div class="h-[2px] bg-slate-400 rounded-full w-3/4"></div>
                            </div>
                            
                            {#if matchingItems.length > 0}
                              {#each matchingItems as item}
                                {@const x_ratio = 48 / (pageData.width || 612)}
                                {@const y_ratio = 64 / (pageData.height || 792)}
                                <div 
                                  class="absolute bg-amber-500/80 rounded-[1px] animate-pulse border border-amber-600/30"
                                  style="left: {(Number(item.x) || 0) * x_ratio}px; top: {(Number(item.y) || 0) * y_ratio}px; width: {Math.max(2, (Number(item.width) || 12) * x_ratio)}px; height: {Math.max(2, (Number(item.height) || 8) * y_ratio)}px"
                                  title="Page {pageData.page}: {getItemText(item)}"
                                ></div>
                              {/each}
                            {:else}
                              <div 
                                class="absolute left-1 right-1 top-1/2 h-1 -translate-y-1/2 rounded-full bg-amber-500/80 animate-pulse border border-amber-600/30"
                                title="Match found in edited OCR text on page {pageData.page}"
                              ></div>
                            {/if}
                          </div>
                          <span class="text-[7px] font-black text-slate-400 group-hover/thumb:text-blue-500 uppercase tracking-tighter">P. {pageData.page}</span>
                        </button>
                      {/each}
                    </div>
                  {:else}
                    {#each res.matches as match}
                      <div class="text-[9px] text-slate-500 italic line-clamp-1">"...{match}..."</div>
                    {/each}
                  {/if}
                </div>
              {/each}
            </div>
          {/if}
        </div>

        <div class="space-y-4 pt-4 border-t border-slate-100 dark:border-slate-900">
          <div class="flex items-center justify-between">
            <h3 class="text-[10px] font-black text-slate-400 uppercase tracking-widest">Collections</h3>
            <div class="flex gap-2">
               <button onclick={handleClusterLibrary} class="text-[8px] font-black text-blue-500 uppercase tracking-tighter">✨ AI Cluster</button>
               <button onclick={handleCreateCollection} class="text-[9px] font-black text-blue-600 uppercase tracking-tighter hover:text-blue-700">+ New</button>
            </div>
          </div>

          <div class="flex gap-2 overflow-x-auto pb-2 no-scrollbar">
            {#each historyState.collections as coll}
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div 
                oncontextmenu={(e) => handleContextMenu(e, coll, 'collection')}
                class="shrink-0 px-3 py-1.5 bg-slate-50 dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-lg text-[10px] font-bold text-slate-600 dark:text-slate-300 hover:border-blue-500 transition-all cursor-pointer shadow-sm"
              >
                📁 {coll.name}
              </div>
            {/each}
          </div>
        </div>

        <div class="space-y-4 pt-4 border-t border-slate-100 dark:border-slate-900">
          <div class="flex items-center justify-between">
            <h3 class="text-[10px] font-black text-slate-400 uppercase tracking-widest">Library Collection</h3>
            <button 
              onclick={() => { isMultiSelect = !isMultiSelect; if(!isMultiSelect) selectedFiles = new Set(); }}
              class="text-[10px] font-bold {isMultiSelect ? 'text-blue-500' : 'text-slate-400 hover:text-slate-600'} transition-colors"
            >
              {isMultiSelect ? 'Done' : 'Select'}
            </button>
          </div>

          {#if isMultiSelect && selectedFiles.size > 0}
            <div class="flex gap-2 p-2 bg-slate-50 dark:bg-slate-900 rounded-xl border border-slate-200 dark:border-slate-800 shadow-sm animate-in fade-in slide-in-from-top-1">
              <button onclick={handleBatchDelete} class="flex-1 py-1.5 bg-red-50 dark:bg-red-950/20 text-red-600 rounded-lg text-[9px] font-black uppercase tracking-tighter hover:bg-red-100 transition-colors">Remove</button>
              <button onclick={handleBatchTag} class="flex-1 py-1.5 bg-blue-50 dark:bg-blue-900/20 text-blue-600 rounded-lg text-[9px] font-black uppercase tracking-tighter hover:bg-blue-100 transition-colors">Tag</button>
              <button onclick={handleBatchMetaStamp} class="flex-1 py-1.5 bg-slate-100 dark:bg-slate-800 text-slate-600 rounded-lg text-[9px] font-black uppercase tracking-tighter hover:bg-slate-200 transition-colors">Stamp</button>
            </div>
          {/if}

          <div class="space-y-2">
            {#each historyState.recentFiles as file}
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div 
                oncontextmenu={(e) => handleContextMenu(e, file, 'file')}
                class="group flex items-center justify-between p-3 bg-white dark:bg-slate-900 border border-slate-100 dark:border-slate-800 rounded-xl hover:border-blue-500 transition-all shadow-sm {selectedFiles.has(file.path) ? 'ring-2 ring-blue-500' : ''}"
              >
                <div class="flex items-center gap-3 min-w-0 flex-1">
                  {#if isMultiSelect}
                    <input type="checkbox" checked={selectedFiles.has(file.path)} onchange={() => toggleSelect(file.path)} class="w-3 h-3 rounded text-blue-600" />
                  {/if}
                  <div class="min-w-0 flex-1">
                    <div class="text-xs font-bold text-slate-700 dark:text-slate-200 truncate">{file.name}</div>
                    <div class="flex gap-1 mt-0.5 overflow-hidden">
                      {#each file.tags || [] as tag}
                        <span class="px-1 py-0.5 bg-blue-50 dark:bg-blue-900/30 text-blue-600 dark:text-blue-400 text-[7px] font-black uppercase rounded">{tag}</span>
                      {/each}
                    </div>
                  </div>
                </div>
                {#if !isMultiSelect}
                  <div class="flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                    <button onclick={() => pdfState.openTab(file.path)} class="p-1.5 text-blue-500 hover:bg-blue-50 rounded-lg">👁️</button>
                    <button onclick={() => { pdfState.viewerFilePath = file.path; pdfState.activeTool = 'peek'; }} class="p-1.5 text-blue-400 hover:bg-blue-50 rounded-lg">🔍</button>
                    <button onclick={() => editFile(file)} class="p-1.5 text-slate-400 hover:text-slate-900 dark:hover:text-white rounded-lg">✎</button>
                  </div>
                {/if}
              </div>
            {/each}
          </div>
        </div>

        {#if selectedFile}
          <div class="space-y-4 pt-4 border-t border-slate-100 dark:border-slate-900">
            <div class="flex items-center justify-between">
              <h3 class="text-[10px] font-black text-blue-600 dark:text-blue-400 uppercase tracking-widest">Edit Metadata</h3>
              <div class="flex gap-2">
                 <button onclick={handleExportBibtex} class="text-[8px] font-black text-slate-400 uppercase">BibTeX</button>
                 <button onclick={() => selectedFile = null} class="text-[10px] font-bold text-slate-400">Cancel</button>
              </div>
            </div>
            <div class="space-y-3">
              <div class="space-y-1">
                <label for="meta-title" class="text-[9px] font-bold text-slate-500 uppercase">Title</label>
                <input id="meta-title" type="text" bind:value={metaTitle} class="w-full p-2 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded text-xs outline-none focus:ring-2 focus:ring-blue-500" />
              </div>
              <div class="space-y-1">
                <div class="flex justify-between items-center">
                  <label for="meta-tags" class="text-[9px] font-bold text-slate-500 uppercase">Tags</label>
                  <button onclick={handleSuggestTags} class="text-[8px] font-black text-blue-500 uppercase tracking-tighter">✨ AI Suggest</button>
                </div>
                <input id="meta-tags" type="text" bind:value={tagsInput} class="w-full p-2 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded text-xs outline-none focus:ring-2 focus:ring-blue-500" placeholder="Work, Invoice, Project..." />
              </div>
              <button onclick={handleUpdateMetadata} class="w-full py-2 bg-slate-900 dark:bg-white text-white dark:text-slate-900 rounded font-bold text-[10px] uppercase tracking-widest shadow-md">Save Properties</button>
            </div>
          </div>
        {/if}
      </div>
    {:else if libraryTab === 'activity'}
      <div class="flex-1 overflow-y-auto pr-1 space-y-4">
        {#each historyState.actions as action}
          <div class="p-3 bg-white dark:bg-slate-900 border border-slate-100 dark:border-slate-800 rounded-xl shadow-sm">
            <div class="flex justify-between items-center mb-1">
              <span class="text-[8px] font-black uppercase text-blue-500">{action.type}</span>
              <span class="text-[8px] text-slate-400">{new Date(action.timestamp).toLocaleTimeString()}</span>
            </div>
            <div class="text-[10px] font-medium text-slate-700 dark:text-slate-300 leading-snug">{action.description}</div>
          </div>
        {/each}
      </div>
    {:else if libraryTab === 'stats'}
      <div class="flex-1 overflow-y-auto pr-1 space-y-6">
        <div class="grid grid-cols-2 gap-3">
          <div class="p-4 bg-blue-50 dark:bg-blue-900/20 rounded-2xl border border-blue-100 dark:border-blue-900/40">
             <div class="text-[8px] font-black uppercase text-blue-500 mb-1">Total Docs</div>
             <div class="text-2xl font-black text-slate-900 dark:text-white">{historyState.recentFiles.length}</div>
          </div>
          <div class="p-4 bg-green-50 dark:bg-green-900/20 rounded-2xl border border-green-100 dark:border-green-900/40">
             <div class="text-[8px] font-black uppercase text-green-500 mb-1">Total Pages</div>
             <div class="text-2xl font-black text-slate-900 dark:text-white">
                {historyState.recentFiles.reduce((acc, doc) => acc + (doc.totalPages || 0), 0)}
             </div>
          </div>
        </div>

        <div class="space-y-3">
          <h3 class="text-[10px] font-black text-slate-400 uppercase tracking-widest px-1">Library Maintenance</h3>
          <button onclick={handleFindDuplicates} class="w-full py-2 border border-slate-200 dark:border-slate-800 text-slate-600 dark:text-slate-300 rounded-lg text-[9px] font-black uppercase tracking-tighter hover:bg-white dark:hover:bg-slate-800 transition-all">Find Duplicates</button>
          {#if duplicateGroups.length > 0}
            <div class="space-y-3 mt-4">
              {#each duplicateGroups as group}
                <div class="p-3 bg-red-50 dark:bg-red-900/10 rounded-xl border border-red-100 dark:border-red-900/30">
                  <div class="text-[8px] font-black text-red-600 uppercase mb-2">Duplicate Group ({group.length} files)</div>
                  {#each group as doc}
                    <div class="text-[9px] font-bold text-slate-700 dark:text-slate-200 truncate">{doc.name}</div>
                  {/each}
                </div>
              {/each}
            </div>
          {/if}
        </div>
      </div>
    {:else if libraryTab === 'watchers'}
      <div class="flex-1 overflow-y-auto pr-1 space-y-6">
        <div class="space-y-4">
          <div class="flex items-center justify-between">
            <h3 class="text-[10px] font-black text-slate-400 uppercase tracking-widest">Document Watchers</h3>
            <button onclick={() => {
              const q = prompt("Topic to monitor:");
              if (q) historyState.createWatcher(q);
            }} class="text-[9px] font-black text-blue-600 uppercase tracking-tighter">+ New</button>
          </div>
          {#each historyState.watchers as watcher}
            <div class="p-3 bg-white dark:bg-slate-900 border border-slate-100 dark:border-slate-800 rounded-xl shadow-sm group">
              <div class="flex items-center justify-between">
                <span class="text-xs font-bold text-slate-700 dark:text-slate-200">"{watcher.query}"</span>
                <button onclick={() => watcher.id && historyState.deleteWatcher(watcher.id)} class="text-[9px] text-red-500">Delete</button>
              </div>
            </div>
          {/each}
        </div>
      </div>
    {:else if libraryTab === 'knowledge'}
      <div class="flex-1 min-h-0 flex flex-col space-y-4">
         <h3 class="text-[10px] font-black text-slate-400 uppercase tracking-widest px-1">Network Visualization</h3>
         <div class="flex-1 min-h-0 relative">
            <KnowledgeGraph />
         </div>
         <div class="p-4 bg-slate-900 dark:bg-white text-white dark:text-slate-900 rounded-2xl shadow-xl">
           <div class="text-[8px] font-black uppercase opacity-60 mb-2">Network Intelligence</div>
           <p class="text-[10px] font-medium leading-relaxed">This graph maps overlapping entities (People, Organizations, Dates) to build a unified Knowledge Graph of your private files.</p>
        </div>
      </div>
    {/if}

    <div class="flex-1 flex flex-col pt-4 border-t border-slate-100 dark:border-slate-900 min-h-0">
      <div class="flex items-center justify-between mb-3 shrink-0 px-1">
        <h3 class="text-[10px] font-black text-slate-400 uppercase tracking-[0.2em]">Library AI</h3>
        <span class="text-[8px] bg-blue-50 dark:bg-blue-900/30 text-blue-600 dark:text-blue-400 px-2 py-0.5 rounded-full font-black uppercase">Alpha</span>
      </div>
      
      <div class="flex-1 overflow-y-auto mb-4 space-y-3 pr-2 scroll-smooth bg-slate-50/50 dark:bg-slate-900/20 rounded-2xl p-3 border border-slate-100 dark:border-slate-800">
        {#each chatState.history as msg}
          <div class="text-[10px] p-2.5 rounded-xl {msg.role === 'user' ? 'bg-blue-100 dark:bg-blue-900/40 text-blue-900 dark:text-blue-100 ml-4 rounded-tr-none' : 'bg-white dark:bg-slate-800 text-slate-800 dark:text-slate-200 mr-4 rounded-tl-none border border-slate-100 dark:border-slate-700 shadow-sm'}">
            {msg.content}
          </div>
        {/each}
      </div>

      <div class="shrink-0 flex gap-2">
        <input 
          type="text" 
          bind:this={chatInput}
          bind:value={chatState.input} 
          onkeydown={(e) => e.key === 'Enter' && chatState.handleAskLibrary()} 
          placeholder="Ask library..." 
          class="flex-1 p-2 bg-white dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl text-xs outline-none focus:ring-2 focus:ring-blue-500 shadow-sm transition-all" 
        />
        <button onclick={() => chatState.handleAskLibrary()} disabled={chatState.isChatting || !chatState.input.trim()} class="w-8 h-8 flex items-center justify-center bg-blue-600 text-white rounded-xl shadow-lg disabled:opacity-50">↑</button>
      </div>
    </div>
  </div>
</ToolPane>

{#if contextMenu}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="fixed inset-0 z-[500]" onclick={() => contextMenu = null}>
    <div 
      class="absolute bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl shadow-2xl py-1.5 min-w-[140px] overflow-hidden"
      style="left: {contextMenu.x}px; top: {contextMenu.y}px"
      in:fly={{ y: -5, duration: 150 }}
    >
      {#if contextMenu.file}
        <button onclick={() => editFile(contextMenu?.file)} class="w-full text-left px-3 py-1.5 text-[10px] font-bold text-slate-700 dark:text-slate-300 hover:bg-blue-50 dark:hover:bg-blue-900/30 transition-colors uppercase tracking-tight">Edit Metadata</button>
        <button onclick={() => handleReveal(contextMenu?.file.path)} class="w-full text-left px-3 py-1.5 text-[10px] font-bold text-slate-700 dark:text-slate-300 hover:bg-blue-50 dark:hover:bg-blue-900/30 transition-colors uppercase tracking-tight">Reveal in Finder</button>
        <div class="h-[1px] bg-slate-100 dark:bg-slate-800 my-1"></div>
        <button onclick={() => handleDeleteFile(contextMenu?.file.path)} class="w-full text-left px-3 py-1.5 text-[10px] font-bold text-red-600 hover:bg-red-50 dark:hover:bg-red-950/20 transition-colors uppercase tracking-tight">Delete</button>
      {:else if contextMenu.collection}
        <button onclick={() => contextMenu?.collection.id && historyState.deleteCollection(contextMenu.collection.id)} class="w-full text-left px-3 py-1.5 text-[10px] font-bold text-red-600 hover:bg-red-50 dark:hover:bg-red-950/20 transition-colors uppercase tracking-tight">Delete Collection</button>
      {/if}
    </div>
  </div>
{/if}
