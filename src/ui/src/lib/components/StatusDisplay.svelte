<script lang="ts">
    import { appState } from '$lib/state/appState.svelte';
    import { invoke } from '@tauri-apps/api/core';

    async function openFile() {
        if (appState.lastSuccessPath) {
            await invoke('shell_open', { filePath: appState.lastSuccessPath });
        }
    }

    async function revealInFolder() {
        if (appState.lastSuccessPath) {
            await invoke('reveal_in_folder', { filePath: appState.lastSuccessPath });
        }
    }
</script>

<div data-testid="status-container">
{#key appState.statusMessage}
    {#if appState.statusMessage}
        <div class="fixed bottom-6 right-6 z-[9999] p-4 flex flex-col sm:flex-row sm:items-center justify-between gap-4 transition-all duration-300 rounded-lg border-2 backdrop-blur-md max-w-sm sm:max-w-lg
            {appState.isError 
                ? 'border-red-600 dark:border-red-500 bg-red-50/95 dark:bg-red-950/90 text-red-900 dark:text-red-100 shadow-[4px_4px_0px_0px_#dc2626]' 
                : 'border-slate-900 dark:border-slate-700 bg-white/95 dark:bg-slate-900/95 text-slate-950 dark:text-slate-50 shadow-[4px_4px_0px_0px_#000] dark:shadow-[4px_4px_0px_0px_#fff]'}"
            data-testid="status-message"
        >
            
            <div class="flex items-center gap-3">
                {#if appState.isLoading}
                    <div class="w-4 h-4 border-2 border-slate-300 border-t-slate-900 dark:border-slate-700 dark:border-t-white rounded-full animate-spin"></div>
                {:else if appState.isError}
                    <span class="text-sm">⚠️</span>
                {:else}
                    <span class="text-sm">✅</span>
                {/if}
                
                <p class="text-xs font-bold tracking-tight">{appState.statusMessage}</p>
            </div>

            {#if appState.lastSuccessPath && !appState.isLoading && !appState.isError}
                <div class="flex gap-2 shrink-0">
                    <button 
                        onclick={openFile}
                        class="px-2.5 py-1 text-[9px] font-black uppercase tracking-wider rounded border border-slate-950 dark:border-slate-700 bg-slate-100 hover:bg-slate-200 dark:bg-slate-800 dark:hover:bg-slate-700 transition-colors text-slate-800 dark:text-slate-200"
                    >
                        Open File
                    </button>
                    <button 
                        onclick={revealInFolder}
                        class="px-2.5 py-1 text-[9px] font-black uppercase tracking-wider rounded border border-slate-950 dark:border-slate-700 bg-slate-100 hover:bg-slate-200 dark:bg-slate-800 dark:hover:bg-slate-700 transition-colors text-slate-800 dark:text-slate-200"
                    >
                        Show in Folder
                    </button>
                </div>
            {/if}
        </div>
    {/if}
{/key}
</div>
