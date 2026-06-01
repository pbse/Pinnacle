import { describe, it, expect, vi, beforeEach } from 'vitest';
import { historyState } from './historyState.svelte';
import { db } from './db';
import { chatState } from './chatState.svelte';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockImplementation((cmd) => {
    if (cmd === 'pdf_to_layout_json') {
      return Promise.resolve(JSON.stringify({
        pages: [
          {
            page: 1,
            width: 612.0,
            height: 792.0,
            text: 'mocked full text',
            text_items: []
          }
        ]
      }));
    }
    if (cmd === 'get_file_hash') return Promise.resolve('mocked_hash');
    return Promise.resolve();
  })
}));

describe('HistoryState', () => {
  beforeEach(async () => {
    await db.documents.clear();
    await db.entities.clear();
    vi.clearAllMocks();
  });

  it('should add a file and trigger auto-indexing', async () => {
    vi.spyOn(chatState, 'nameDocument').mockResolvedValue('Mocked Summary');
    vi.spyOn(chatState, 'getDocumentInsights').mockResolvedValue({ dates: ['2023-01-01'], amounts: [], orgs: ['MockOrg'] });
    
    await historyState.addFile('/mock/path.pdf');
    
    // Wait for async background extraction
    await new Promise(resolve => setTimeout(resolve, 50));
    
    const doc = await db.documents.where('path').equals('/mock/path.pdf').first();
    expect(doc).toBeDefined();
    expect(doc?.summary).toBe('Mocked Summary');
    expect(doc?.fullText).toBe('mocked full text');
    
    const orgs = await db.entities.where('type').equals('org').toArray();
    expect(orgs.length).toBeGreaterThan(0);
    expect(orgs[0].name).toBe('MockOrg');
    expect(orgs[0].docPaths).toContain('/mock/path.pdf');
  });
});
