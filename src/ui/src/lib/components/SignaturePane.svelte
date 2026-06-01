<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { pdfState } from "$lib/state/pdfState.svelte";
  import { chatState } from "$lib/state/chatState.svelte";
  import { appState } from "$lib/state/appState.svelte";
  import ToolPane from "./ToolPane.svelte";

  let makePermanent = $state(true); // Default to true for signatures for better security

  async function handleSecureShare() {
    const file = pdfState.selectedSignatureFile || pdfState.viewerFilePath;
    if (!file) return;
    try {
      let pdfText = await invoke<string>("pdf_to_text_string", { path: file });
      if (pdfText.length > 5000) pdfText = pdfText.substring(0, 5000);

      const system = "Draft a professional email sharing a signed document. Mention privacy. Return ONLY the Subject and Body.";
      const draft = await chatState.runAiTask(system, `TEXT:\n${pdfText}`);
      
      const subject = `Signed Document: ${file.split(/[/\\]/).pop()}`;
      await invoke("shell_open", { filePath: `mailto:?subject=${encodeURIComponent(subject)}&body=${encodeURIComponent(draft)}` });
      appState.showStatus("Opening mail client...", false);
    } catch (e) {
      console.error(e);
      appState.showStatus("Error sharing.", true);
    }
  }

  async function selectFile() {
    const result = await invoke<string[]>("open_file_dialog", { multiple: false });
    if (result && result.length > 0) {
      const path = result[0];
      pdfState.setFileForTarget('signature', path);
      pdfState.openTab(path);
    }
  }

  function openSignPad() {
    if (!pdfState.selectedSignatureFile && !pdfState.viewerFilePath) {
      selectFile().then(() => {
        if (pdfState.selectedSignatureFile) {
          pdfState.showSignPad = true;
        }
      });
      return;
    }
    if (!pdfState.selectedSignatureFile && pdfState.viewerFilePath) {
      pdfState.setFileForTarget('signature', pdfState.viewerFilePath);
    }
    pdfState.showSignPad = true;
  }
</script>

<ToolPane title="Sign">
  <div class="space-y-6">
    <div class="flex flex-col gap-2">
      <button onclick={selectFile} class="w-full py-2 px-4 bg-white dark:bg-slate-900 border border-slate-300 dark:border-slate-700 text-slate-700 dark:text-slate-200 rounded-md text-sm font-medium transition-colors shadow-sm">
        {pdfState.selectedSignatureFile ? pdfState.selectedSignatureFile.split(/[/\\]/).pop() : 'Select PDF'}
      </button>
    </div>
    
    <div class="space-y-4">
      <h3 class="text-[10px] font-bold text-slate-500 dark:text-slate-400 uppercase tracking-widest transition-colors">Drawing</h3>
      <button 
        onclick={openSignPad} 
        class="w-full py-2.5 bg-blue-600 hover:bg-blue-700 text-white rounded font-bold text-xs uppercase tracking-widest transition-colors shadow-md shadow-blue-500/20"
      >
        {pdfState.activeStamp ? 'Redraw Signature' : 'Draw New Signature'}
      </button>
      
      {#if pdfState.activeStamp}
        <div class="flex items-center gap-2 text-green-600 dark:text-green-500 text-[10px] font-bold uppercase tracking-wider transition-colors mt-2">
          <span class="w-1.5 h-1.5 rounded-full bg-green-500 animate-pulse"></span>
          Signature Loaded: Click and drag stamp on document to place
        </div>
      {/if}
    </div>

    <div class="space-y-4 pt-4 border-t border-slate-100 dark:border-slate-900 transition-colors">
      <div class="space-y-1.5">
        <label for="sig-color" class="text-[10px] font-bold text-slate-500 dark:text-slate-400 uppercase tracking-widest transition-colors">Ink Color</label>
        <div class="flex items-center gap-3">
          <input id="sig-color" type="color" bind:value={pdfState.signatureColor} class="w-10 h-10 p-0 border-0 bg-transparent cursor-pointer rounded-full overflow-hidden transition-colors shadow-sm" />
          <span class="text-[10px] font-mono uppercase text-slate-400 dark:text-slate-500 font-bold transition-colors tracking-widest">{pdfState.signatureColor}</span>
        </div>
      </div>
      
      <label class="flex items-center gap-2 cursor-pointer pt-2 group">
        <input type="checkbox" bind:checked={makePermanent} class="w-4 h-4 rounded border-slate-300 text-blue-600 focus:ring-blue-500 transition-all" />
        <span class="text-[10px] font-bold text-slate-500 group-hover:text-slate-700 dark:group-hover:text-slate-300 transition-colors uppercase tracking-tight">Non-Deletable Signature (Flatten)</span>
      </label>

      <button 
        onclick={handleSecureShare}
        disabled={!pdfState.selectedSignatureFile && !pdfState.viewerFilePath}
        class="w-full py-2.5 border border-blue-600 text-blue-600 dark:text-blue-400 rounded-lg font-bold text-[10px] uppercase tracking-widest hover:bg-blue-50 dark:hover:bg-blue-900/20 transition-all shadow-md disabled:opacity-30"
      >
        Draft Email Share
      </button>
    </div>

    <!-- Pending Changes Checklist Area -->
    {#if pdfState.pendingChanges.length > 0}
      <div class="mt-6 pt-6 border-t-2 border-slate-950 dark:border-slate-800 transition-colors">
        <h3 class="text-[10px] font-bold text-slate-500 dark:text-slate-400 uppercase tracking-widest mb-3">Pending Changes ({pdfState.pendingChanges.length})</h3>
        <div class="space-y-2 max-h-48 overflow-y-auto mb-4">
          {#each pdfState.pendingChanges as change}
            <div class="flex items-center justify-between p-2 bg-slate-50 dark:bg-slate-900 border border-slate-200 dark:border-slate-800 text-[10px] rounded">
              <div class="truncate mr-2 font-mono flex items-center gap-1.5">
                <span class="px-1.5 py-0.5 bg-slate-900 text-white dark:bg-white dark:text-slate-900 font-black rounded-xs">p.{change.page}</span>
                <span class="text-slate-600 dark:text-slate-300 font-bold uppercase tracking-wider">{change.target === 'signature' ? 'Signature Stamp' : change.type}</span>
              </div>
              <button 
                onclick={() => pdfState.removePendingChange(change.id)} 
                class="text-red-500 hover:text-red-700 font-bold px-1 transition-colors"
                title="Remove Change"
              >
                ✕
              </button>
            </div>
          {/each}
        </div>
        
        <div class="flex flex-col gap-2">
          <button 
            onclick={() => pdfState.commitAllPending(makePermanent)} 
            class="w-full py-3 bg-slate-900 text-white dark:bg-white dark:text-slate-900 border-2 border-slate-950 dark:border-white font-black text-xs uppercase tracking-widest hover:scale-[1.02] active:scale-95 transition-all shadow-[4px_4px_0px_0px_rgba(15,23,42,1)] dark:shadow-[4px_4px_0px_0px_rgba(255,255,255,1)]"
          >
            Apply All & Save
          </button>
          <button 
            onclick={() => pdfState.clearPendingChanges()} 
            class="w-full py-1.5 text-center text-[10px] font-bold text-red-500 hover:underline uppercase tracking-wider"
          >
            Clear All Changes
          </button>
        </div>
      </div>
    {/if}
  </div>
</ToolPane>
