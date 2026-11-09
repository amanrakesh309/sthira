import puppeteer from 'puppeteer-core';
import fs from 'node:fs';
import path from 'node:path';

const OUT_DIR = path.resolve('../../../docs/design/visual-audit/settings/before');
fs.mkdirSync(OUT_DIR, { recursive: true });

const resolutions = [
  { name: '1440x900', width: 1440, height: 900 },
  { name: '1366x768', width: 1366, height: 768 },
];

const categories = [
  { id: 'models', label: 'Models & Providers' },
  { id: 'search', label: 'Web Search' },
  { id: 'appearance', label: 'Appearance' },
  { id: 'vault', label: 'Vault & Storage' },
];

async function run() {
  const browser = await puppeteer.launch({
    executablePath: '/usr/bin/google-chrome',
    args: ['--no-sandbox', '--disable-setuid-sandbox', '--disable-dev-shm-usage', '--headless=new'],
  });

  const page = await browser.newPage();
  const report = {
    measurements: {},
    issues: [],
  };

  for (const res of resolutions) {
    await page.setViewport({ width: res.width, height: res.height, deviceScaleFactor: 1 });
    await page.goto('http://localhost:1420/', { waitUntil: 'networkidle0' });

    // Open settings via UI: find button with 'Settings' text
    await page.evaluate(() => {
      const btns = Array.from(document.querySelectorAll('button'));
      const sBtn = btns.find(b => b.textContent?.trim() === 'Settings' || b.getAttribute('title') === 'Model Settings');
      if (sBtn) sBtn.click();
    });

    await page.waitForSelector('[data-testid="model-settings-modal"]', { timeout: 5000 });
    await new Promise(r => setTimeout(r, 600));

    // Capture each category
    for (const cat of categories) {
      // Click category tab/button
      await page.evaluate((tabId) => {
        const quickBtn = document.querySelector(`[data-testid="quick-setup-${tabId}"]`);
        if (quickBtn) {
          quickBtn.click();
          return;
        }
        const tabPills = Array.from(document.querySelectorAll('.settings-tab-pill'));
        const pill = tabPills.find(p => p.textContent?.toLowerCase().includes(tabId));
        if (pill) pill.click();
      }, cat.id);

      await new Promise(r => setTimeout(r, 400));

      const screenshotPath = path.join(OUT_DIR, `before_${cat.id}_${res.name}_light.png`);
      await page.screenshot({ path: screenshotPath });
      console.log(`Saved screenshot: ${screenshotPath}`);

      // Measure dimensions
      const metrics = await page.evaluate(() => {
        const modal = document.querySelector('.settings-modal-dialog') || document.querySelector('[data-testid="model-settings-modal"]');
        const backdrop = document.querySelector('.settings-modal-backdrop');
        const grid = document.querySelector('.settings-body-grid');
        const guidance = document.querySelector('.settings-guidance-col');
        const content = document.querySelector('.settings-content-col') || document.querySelector('.settings-guidance-col + div');
        const tabsRow = document.querySelector('.settings-tabs-row');

        return {
          isModalOverlay: !!backdrop && window.getComputedStyle(backdrop).position === 'fixed',
          modalRect: modal ? modal.getBoundingClientRect() : null,
          gridRect: grid ? grid.getBoundingClientRect() : null,
          guidanceRect: guidance ? guidance.getBoundingClientRect() : null,
          contentRect: content ? content.getBoundingClientRect() : null,
          tabsRowExists: !!tabsRow,
          modalMaxHeight: modal ? window.getComputedStyle(modal).maxHeight : null,
          modalMaxWidth: modal ? window.getComputedStyle(modal).maxWidth : null,
          gridOverflowY: grid ? window.getComputedStyle(grid).overflowY : null,
        };
      });

      report.measurements[`${cat.id}_${res.name}`] = metrics;
    }

    // Also test dark mode
    await page.evaluate(() => {
      document.documentElement.classList.add('dark');
      localStorage.setItem('sthira_theme', 'dark');
    });
    await new Promise(r => setTimeout(r, 400));

    for (const cat of categories) {
      await page.evaluate((tabId) => {
        const quickBtn = document.querySelector(`[data-testid="quick-setup-${tabId}"]`);
        if (quickBtn) quickBtn.click();
      }, cat.id);
      await new Promise(r => setTimeout(r, 300));
      const darkScreenshotPath = path.join(OUT_DIR, `before_${cat.id}_${res.name}_dark.png`);
      await page.screenshot({ path: darkScreenshotPath });
      console.log(`Saved screenshot: ${darkScreenshotPath}`);
    }

    // Reset dark mode
    await page.evaluate(() => {
      document.documentElement.classList.remove('dark');
      localStorage.setItem('sthira_theme', 'light');
    });
  }

  fs.writeFileSync(path.join(OUT_DIR, 'before_metrics.json'), JSON.stringify(report, null, 2));
  console.log('Saved before_metrics.json');

  await browser.close();
}

run().catch(err => {
  console.error('Audit run failed:', err);
  process.exit(1);
});
