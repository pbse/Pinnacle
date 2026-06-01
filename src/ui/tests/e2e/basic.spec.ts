import { test, expect } from '@playwright/test';

test.describe('Pinnacle UI', () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(() => {
      window.localStorage.setItem('onboarding_complete', 'true');
      (window as any).__TAURI_INTERNALS__ = {
        invoke: async (cmd: string, args: any) => {
          console.log(`Mocked invoke: ${cmd}`, args);
          if (cmd === 'pdf_to_text_string') return 'Mock PDF Content';
          if (cmd === 'get_file_hash') return 'mock-hash';
          if (cmd === 'open_file_dialog') return ['/mock/test.pdf'];
          if (cmd === 'save_file_dialog') return '/mock/output.pdf';
          if (cmd === 'read_file_bytes') return new Uint8Array([37, 80, 68, 70, 45]);
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
    await page.evaluate(() => {
      localStorage.setItem('onboarding_complete', 'true');
    });
    
    // Wait for any overlays to be gone
    await page.locator('.fixed.inset-0.z-\\[400\\]').waitFor({ state: 'hidden', timeout: 5000 }).catch(() => {});

    // Wait for landing page
    await expect(page.getByRole('heading', { name: 'Pinnacle Intelligence' })).toBeVisible({ timeout: 15000 });
  });

  test('should load the app and show the welcome screen', async ({ page }) => {
    await expect(page.getByRole('heading', { name: 'Pinnacle Intelligence' })).toBeVisible();
  });

  test('should switch to Split tool', async ({ page }) => {
    await page.click('button[title="Split"]', { force: true });
    
    // Wait for the active pane container to update
    await expect(page.getByTestId('active-pane-split')).toBeVisible({ timeout: 10000 });

    // Check for the heading inside the split pane
    await expect(page.getByRole('heading', { name: /Split/i })).toBeVisible({ timeout: 10000 });
  });

  test('should switch to Extract tool', async ({ page }) => {
    await page.click('button[title="Assistant"]', { force: true });
    
    await expect(page.getByTestId('active-pane-extract')).toBeVisible({ timeout: 10000 });

    await expect(page.getByRole('heading', { name: /Assistant/i })).toBeVisible({ timeout: 10000 });
  });
});
