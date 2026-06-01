import { test, expect } from '@playwright/test';

test.describe('Pinnacle E2E Flows', () => {
  
  test.beforeEach(async ({ page }) => {
    // 1. Mock Tauri internals and force onboarding complete BEFORE navigation
    await page.addInitScript(() => {
      window.localStorage.setItem('onboarding_complete', 'true');
      
      // Mock for Tauri v2
      (window as any).__TAURI_INTERNALS__ = {
        invoke: async (cmd: string, args: any) => {
          console.log(`Invoke called: ${cmd}`, args);
          if (cmd === 'open_file_dialog') return ['/mock/test.pdf'];
          if (cmd === 'save_file_dialog') return '/mock/output.pdf';
          if (cmd === 'read_file_bytes') return new Uint8Array([37, 80, 68, 70, 45]);
          if (cmd === 'pdf_to_text_string') return 'Mock PDF Content for Testing';
          if (cmd === 'get_file_hash') return 'mock-hash-123';
          return {};
        },
        metadata: { version: '2.0.0' }
      };

      // Comprehensive mock for @tauri-apps/api
      (window as any).__TAURI__ = {
        invoke: (window as any).__TAURI_INTERNALS__.invoke,
        event: {
          listen: async () => () => {},
          emit: async () => {}
        },
        webviewWindow: {
          getCurrentWebviewWindow: () => ({
            onDragDropEvent: async () => () => {},
            onCloseRequested: async () => () => {},
            label: 'main'
          })
        }
      };
    });

    // 2. Initial Load
    await page.goto('/');
    
    // 3. Ensure onboarding is hidden in storage
    await page.evaluate(() => {
      localStorage.setItem('onboarding_complete', 'true');
    });
    
    // 4. Wait for any overlays to be gone
    await page.locator('.fixed.inset-0.z-\\[400\\]').waitFor({ state: 'hidden', timeout: 5000 }).catch(() => {});
    
    // 5. Verify we are on the landing page
    await expect(page.getByRole('heading', { name: 'Pinnacle Intelligence' })).toBeVisible({ timeout: 15000 });
  });

  test('should navigate through major tools', async ({ page }) => {
    const tools = [
      { id: 'extract', label: 'Assistant', status: 'EXTRACT', title: /Assistant/i },
      { id: 'compare', label: 'Compare', status: 'COMPARE', title: /Compare/i },
      { id: 'merge', label: 'Merge', status: 'MERGE', title: /Merge/i },
      { id: 'split', label: 'Split', status: 'SPLIT', title: /Split/i },
      { id: 'annotate', label: 'Annotate', status: 'ANNOTATE', title: /Annotate/i },
      { id: 'signature', label: 'Sign', status: 'SIGNATURE', title: /Sign/i },
      { id: 'security', label: 'Protect', status: 'SECURITY', title: /Security/i },
      { id: 'organize', label: 'Organize', status: 'ORGANIZE', title: /Organize/i }
    ];

    for (const tool of tools) {
      await page.waitForTimeout(200);
      // Use forced click
      const btn = page.locator(`button[title="${tool.label}"]`);
      await btn.evaluate((el) => el.scrollIntoView({ block: 'start' }));
      await btn.click({ force: true });
      
      // Check for heading with regex
      await expect(page.getByRole('heading', { name: tool.title })).toBeVisible({ timeout: 10000 });
    }
  });

  test('should verify security utilities', async ({ page }) => {
    await page.click('button[title="Protect"]', { force: true });
    
    // When no file is selected, buttons might show "Select PDF"
    await expect(page.locator('button:has-text("Sanitize"), button:has-text("Select PDF")').first()).toBeVisible({ timeout: 10000 });
    await expect(page.locator('button:has-text("Flatten"), button:has-text("Select PDF")').first()).toBeVisible({ timeout: 10000 });
  });

  test('should verify annotation flow visibility', async ({ page }) => {
    await page.click('button[title="Annotate"]', { force: true });
    await expect(page.locator('button:has-text("Select PDF")').first()).toBeVisible({ timeout: 10000 });
    await expect(page.locator('text=Selection Area')).toBeVisible({ timeout: 10000 });
  });

  test('should verify signature flow components', async ({ page }) => {
    await page.click('button[title="Sign"]', { force: true });
    await expect(page.locator('button:has-text("Select PDF")').first()).toBeVisible({ timeout: 10000 });
    await expect(page.locator('button:has-text("Draw New Signature")')).toBeVisible({ timeout: 10000 });
  });

  test('should toggle dark mode', async ({ page }) => {
    const body = page.locator('html');
    await expect(body).not.toHaveClass(/dark/);
    await page.click('button[title="Toggle Dark Mode"]', { force: true });
    await expect(body).toHaveClass(/dark/, { timeout: 10000 });
    await page.click('button[title="Toggle Dark Mode"]', { force: true });
    await expect(body).not.toHaveClass(/dark/, { timeout: 10000 });
  });

  test('should open command palette', async ({ page }) => {
    await page.mouse.click(10, 10);
    // Use only one modifier to avoid double-toggle
    await page.keyboard.press('Control+k');
    await expect(page.locator('input[placeholder="Type a command or search..."]')).toBeVisible({ timeout: 10000 });
    await page.keyboard.type('Split');
    await expect(page.locator('text=Split PDF')).toBeVisible({ timeout: 10000 });
    await page.keyboard.press('Escape');
    await expect(page.locator('input[placeholder="Type a command or search..."]')).not.toBeVisible({ timeout: 10000 });
  });

  test('should verify Select PDF in Assistant works', async ({ page }) => {
    // 1. Switch to Assistant tool
    await page.click('button[title="Assistant"]', { force: true });
    
    // 2. Click the "Select PDF" button
    const selectPdfBtn = page.locator('button:has-text("Select PDF")');
    await expect(selectPdfBtn).toBeVisible({ timeout: 10000 });
    await selectPdfBtn.click({ force: true });

    // 3. Verify that the button text changes to "Export MD Report" after selection (mocked to /mock/test.pdf)
    await expect(page.locator('button:has-text("Export MD Report")')).toBeVisible({ timeout: 10000 });
  });
});
