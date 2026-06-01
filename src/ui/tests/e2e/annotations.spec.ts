import { test, expect } from '@playwright/test';

test.describe('Pinnacle Annotations', () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(() => {
      window.localStorage.setItem('onboarding_complete', 'true');
      (window as any).__TAURI_INTERNALS__ = {
        invoke: async (cmd: string, args: any) => {
          console.log(`Mocked invoke: ${cmd}`, args);
          if (cmd === 'open_file_dialog') return ['/mock/test.pdf'];
          if (cmd === 'save_file_dialog') return '/mock/output.pdf';
          if (cmd === 'read_file_bytes') return new Uint8Array([37, 80, 68, 70, 45]);
          if (cmd === 'add_annotation' || cmd === 'add_ink_annotation') return {};
          if (cmd === 'shell_open') return {};
          if (cmd === 'pdf_to_images') return ['/mock/page1.png'];
          return {};
        },
        metadata: { version: '2.0.0' }
      };
      (window as any).__TAURI__ = {
        invoke: (window as any).__TAURI_INTERNALS__.invoke,
        event: { 
          listen: async () => () => {},
          emit: async () => {}
        },
        webviewWindow: {
          getCurrentWebviewWindow: () => ({
            onDragDropEvent: async () => () => {},
            label: 'main'
          })
        }
      };
    });
    await page.goto('/');
  });

  test('should allow selecting and applying a square annotation', async ({ page }) => {
    // Switch to Annotate tool
    await page.click('button[title="Annotate"]', { force: true });
    await expect(page.getByTestId('active-pane-annotate')).toBeVisible();

    // Select PDF
    await page.click('button:has-text("Select PDF")');
    
    // Enter rect selection mode (mocked as openViewer('rect'))
    await page.click('button:has-text("🎯")');

    // Simulate selecting a rect (this would normally happen in PdfViewer, but we mock the input)
    await page.fill('input[placeholder="x1, y1, x2, y2"]', '100, 100, 200, 200');

    // Select Square type
    await page.selectOption('select#annotate-type', 'square');

    // Apply annotation
    await page.click('button:has-text("Queue Annotation")');

    // Check for success status
    await expect(page.getByTestId('status-message')).toContainText('Annotation added to queue');

    // Commit all changes
    await page.click('button:has-text("Apply All & Save")');
    await expect(page.getByTestId('status-message')).toContainText('Successfully applied all changes');
  });

  test('should toggle drawing mode and switch to ink annotation', async ({ page }) => {
    await page.click('button[title="Annotate"]', { force: true });
    
    // Select PDF first
    await page.click('button:has-text("Select PDF")');

    // Toggle drawing mode
    await page.click('button[title="Toggle Freehand Mode"]');
    
    // Type should change to ink
    await expect(page.locator('select#annotate-type')).toHaveValue('ink');

    // Applying without strokes should show error (handled by Svelte state)
    await page.click('button:has-text("Queue Annotation")');
    await expect(page.getByTestId('status-message')).toContainText('Please draw on the document first');
  });
});
