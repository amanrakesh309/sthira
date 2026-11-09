import puppeteer from 'puppeteer-core';
import fs from 'node:fs';
import path from 'node:path';

const OUT_DIR = path.resolve('../../../docs/design/visual-audit/settings/after');
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
    stabilityVerification: {},
    navigationVerification: {},
  };

  for (const res of resolutions) {
    await page.setViewport({ width: res.width, height: res.height, deviceScaleFactor: 1 });
    await page.goto('http://localhost:1420/', { waitUntil: 'networkidle0' });

    // Open settings via LeftSidebar Settings button
    await page.evaluate(() => {
      const btns = Array.from(document.querySelectorAll('button'));
      const sBtn = btns.find(b => b.textContent?.trim() === 'Settings' || b.getAttribute('title') === 'Model Settings');
      if (sBtn) sBtn.click();
    });

    await page.waitForSelector('[data-testid="model-settings-modal"]', { timeout: 5000 });
    await new Promise(r => setTimeout(r, 600));

    // Capture each category in Light theme
    for (const cat of categories) {
      await page.evaluate((tabId) => {
        const quickBtn = document.querySelector(`[data-testid="quick-setup-${tabId}"]`);
        if (quickBtn) quickBtn.click();
      }, cat.id);
      await new Promise(r => setTimeout(r, 350));

      const screenshotPath = path.join(OUT_DIR, `after_${cat.id}_${res.name}_light.png`);
      await page.screenshot({ path: screenshotPath });
      console.log(`Saved screenshot: ${screenshotPath}`);

      const metrics = await page.evaluate(() => {
        const modal = document.querySelector('.settings-modal-dialog') || document.querySelector('[data-testid="model-settings-modal"]');
        const grid = document.querySelector('.settings-body-grid');
        const guidance = document.querySelector('.settings-guidance-col');
        const content = document.querySelector('.settings-content-col');
        const header = document.querySelector('.settings-header');
        const footer = document.querySelector('.settings-footer');
        const backBtn = document.querySelector('[data-testid="settings-back-btn"]');

        const mRect = modal ? modal.getBoundingClientRect() : null;
        const gRect = guidance ? guidance.getBoundingClientRect() : null;
        const cRect = content ? content.getBoundingClientRect() : null;

        return {
          isFullPageWorkspace: mRect ? (mRect.width === window.innerWidth && mRect.height === window.innerHeight) : false,
          modalRect: mRect ? { width: mRect.width, height: mRect.height, top: mRect.top, left: mRect.left } : null,
          guidanceRect: gRect ? { width: gRect.width, height: gRect.height, top: gRect.top, left: gRect.left } : null,
          contentRect: cRect ? { width: cRect.width, height: cRect.height, top: cRect.top, left: cRect.left } : null,
          hasBackButton: !!backBtn,
          bodyOverflow: grid ? window.getComputedStyle(grid).overflow : null,
          contentOverflowY: content ? window.getComputedStyle(content).overflowY : null,
          guidanceOverflowY: guidance ? window.getComputedStyle(guidance).overflowY : null,
        };
      });

      report.measurements[`${cat.id}_${res.name}`] = metrics;
    }

    // Capture each category in Dark theme
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
      await new Promise(r => setTimeout(r, 350));

      const darkScreenshotPath = path.join(OUT_DIR, `after_${cat.id}_${res.name}_dark.png`);
      await page.screenshot({ path: darkScreenshotPath });
      console.log(`Saved screenshot: ${darkScreenshotPath}`);
    }

    // Reset theme
    await page.evaluate(() => {
      document.documentElement.classList.remove('dark');
      localStorage.setItem('sthira_theme', 'light');
    });
  }

  // --- Specific Functional Validation Tests ---

  // 1. Left navigation stability during content scroll
  await page.setViewport({ width: 1440, height: 900 });
  await page.evaluate(() => {
    const modelsBtn = document.querySelector('[data-testid="quick-setup-models"]');
    if (modelsBtn) modelsBtn.click();
  });
  await new Promise(r => setTimeout(r, 300));

  const scrollTest = await page.evaluate(async () => {
    const guidance = document.querySelector('.settings-guidance-col');
    const content = document.querySelector('.settings-content-col');
    const initialGuidanceTop = guidance.getBoundingClientRect().top;
    const initialGuidanceLeft = guidance.getBoundingClientRect().left;

    // Scroll content down 400px
    content.scrollTop = 400;
    await new Promise(r => setTimeout(r, 150));

    const scrolledGuidanceTop = guidance.getBoundingClientRect().top;
    const scrolledGuidanceLeft = guidance.getBoundingClientRect().left;
    const contentScrollTop = content.scrollTop;

    return {
      initialGuidanceTop,
      scrolledGuidanceTop,
      topDifference: Math.abs(initialGuidanceTop - scrolledGuidanceTop),
      initialGuidanceLeft,
      scrolledGuidanceLeft,
      contentScrollTop,
      isLeftNavStable: Math.abs(initialGuidanceTop - scrolledGuidanceTop) === 0,
      isContentScrolled: contentScrollTop > 0,
    };
  });
  report.stabilityVerification.scrollTest = scrollTest;
  console.log('Left nav stability test:', scrollTest);

  // Take screenshot of scrolled state
  await page.screenshot({ path: path.join(OUT_DIR, 'after_scroll_test_1440x900.png') });

  // 2. Tab dimension stability: verify that switching between all categories doesn't change container bounds
  const dimTest = await page.evaluate(async () => {
    const tabs = ['models', 'search', 'appearance', 'vault'];
    const dimensions = [];
    const modal = document.querySelector('.settings-modal-dialog');

    for (const tab of tabs) {
      const btn = document.querySelector(`[data-testid="quick-setup-${tab}"]`);
      if (btn) btn.click();
      await new Promise(r => setTimeout(r, 200));
      const rect = modal.getBoundingClientRect();
      dimensions.push({
        tab,
        width: rect.width,
        height: rect.height,
        top: rect.top,
        left: rect.left,
      });
    }

    const allWidthsMatch = dimensions.every(d => d.width === dimensions[0].width);
    const allHeightsMatch = dimensions.every(d => d.height === dimensions[0].height);
    const allTopsMatch = dimensions.every(d => d.top === dimensions[0].top);

    return {
      dimensions,
      allDimensionsStable: allWidthsMatch && allHeightsMatch && allTopsMatch,
    };
  });
  report.stabilityVerification.dimensionTest = dimTest;
  console.log('Dimension stability test:', dimTest);

  // 3. Back to workspace button test
  const backBtnTest = await page.evaluate(async () => {
    const backBtn = document.querySelector('[data-testid="settings-back-btn"]');
    if (!backBtn) return { error: 'No back button found' };
    backBtn.click();
    await new Promise(r => setTimeout(r, 400));
    const modalInDom = !!document.querySelector('[data-testid="model-settings-modal"]');
    const homeInDom = !!document.querySelector('[data-testid="center-workspace"]') || !!document.querySelector('.home-hero');
    return {
      modalDismissed: !modalInDom,
      workspaceVisible: homeInDom,
    };
  });
  report.navigationVerification.backButton = backBtnTest;
  console.log('Back button test:', backBtnTest);

  // 4. Keyboard Escape key test
  // Re-open settings
  await page.evaluate(() => {
    const btns = Array.from(document.querySelectorAll('button'));
    const sBtn = btns.find(b => b.textContent?.trim() === 'Settings' || b.getAttribute('title') === 'Model Settings');
    if (sBtn) sBtn.click();
  });
  await new Promise(r => setTimeout(r, 400));

  await page.keyboard.press('Escape');
  await new Promise(r => setTimeout(r, 400));

  const escTest = await page.evaluate(() => {
    const modalInDom = !!document.querySelector('[data-testid="model-settings-modal"]');
    return {
      modalDismissedOnEsc: !modalInDom,
    };
  });
  report.navigationVerification.escapeKey = escTest;
  console.log('Escape key test:', escTest);

  fs.writeFileSync(path.join(OUT_DIR, 'after_metrics.json'), JSON.stringify(report, null, 2));
  console.log('Saved after_metrics.json');

  await browser.close();
}

run().catch(err => {
  console.error('Audit capture after failed:', err);
  process.exit(1);
});

