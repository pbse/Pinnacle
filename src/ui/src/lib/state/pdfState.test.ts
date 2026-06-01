import { describe, it, expect, beforeEach } from 'vitest';
import { pdfState } from './pdfState.svelte';

describe('PdfState Rune', () => {
  beforeEach(() => {
    pdfState.activeTool = 'merge';
    pdfState.selectedMergeFiles = [];
    pdfState.viewerFilePath = "";
    pdfState.openTabs = [];
    pdfState.history = [];
    pdfState.redoStack = [];
  });

  it('should switch tools correctly', () => {
    pdfState.switchTool('split');
    expect(pdfState.activeTool).toBe('split');
    expect(pdfState.viewerFilePath).toBe("");
  });

  it('should handle single PDF drop', () => {
    const paths = ['/test/file.pdf'];
    pdfState.handleDroppedFiles(paths);
    expect(pdfState.viewerFilePath).toBe('/test/file.pdf');
    expect(pdfState.selectedSplitFile).toBe('/test/file.pdf');
  });

  it('should handle smart citation jumping', () => {
    // Open a completely different file first
    pdfState.openTab('/another.pdf');
    pdfState.viewerPageNumber = 5;

    // Simulate clicking a citation
    const citation = { docPath: '/test/source.pdf', pageNumber: 42, text: 'This is the cited snippet.' };
    pdfState.openTab(citation.docPath);
    pdfState.viewerPageNumber = citation.pageNumber;
    pdfState.highlightedSnippet = citation.text;

    expect(pdfState.viewerFilePath).toBe('/test/source.pdf');
    expect(pdfState.viewerPageNumber).toBe(42);
    expect(pdfState.highlightedSnippet).toBe('This is the cited snippet.');
    // Check that it's actually in openTabs
    expect(pdfState.openTabs).toContain('/test/source.pdf');
  });

  it('should handle tab management', () => {
    pdfState.openTab('/a.pdf');
    pdfState.openTab('/b.pdf');
    expect(pdfState.openTabs).toHaveLength(2);
    expect(pdfState.viewerFilePath).toBe('/b.pdf');
    
    pdfState.closeTab('/b.pdf');
    expect(pdfState.openTabs).toHaveLength(1);
    expect(pdfState.viewerFilePath).toBe('/a.pdf');
  });

  it('should handle undo/redo correctly', () => {
    pdfState.annotationRectInput = "10,10,50,50";
    pdfState.pushHistory({ annotationRectInput: "" });
    
    pdfState.annotationRectInput = "100,100,200,200";
    pdfState.undo();
    expect(pdfState.annotationRectInput).toBe("");
    
    pdfState.redo();
    expect(pdfState.annotationRectInput).toBe("100,100,200,200");
  });

  it('should reject non-PDF drops', () => {
    const paths = ['/test/audio.mp3'];
    pdfState.handleDroppedFiles(paths);
    expect(pdfState.viewerFilePath).toBe("");
  });

  it('should manage pending changes queue correctly', () => {
    pdfState.clearPendingChanges();
    expect(pdfState.pendingChanges).toHaveLength(0);
    
    const change = {
      id: 'change-1',
      target: 'annotate' as const,
      page: 1,
      rect: [0, 0, 10, 10],
      strokes: null,
      type: 'highlight',
      color: '#ff0000'
    };
    
    pdfState.addPendingChange(change);
    expect(pdfState.pendingChanges).toHaveLength(1);
    expect(pdfState.pendingChanges[0].id).toBe('change-1');
    
    pdfState.removePendingChange('change-1');
    expect(pdfState.pendingChanges).toHaveLength(0);
  });
});
