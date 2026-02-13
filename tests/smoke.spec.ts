/// <reference types="node" />

import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { test, expect } from '@playwright/test';

function sha256(input: Buffer): string {
  return createHash('sha256').update(input).digest('hex');
}

async function getPrompt(): Promise<string> {
  if (process.env.CADAI_SMOKE_PROMPT && process.env.CADAI_SMOKE_PROMPT.trim().length > 0) {
    return process.env.CADAI_SMOKE_PROMPT;
  }

  const fixturePath = new URL('./fixtures/whoop_prompt.md', import.meta.url);
  return readFile(fixturePath, 'utf8');
}

test('smoke: send CAD prompt, capture feedback and viewport evidence', async ({ page }, testInfo) => {
  const prompt = await getPrompt();
  const requireTauri = process.env.CADAI_REQUIRE_TAURI === '1';

  await page.goto('/');
  await expect(page.locator('.chat-input')).toBeVisible();
  await expect(page.locator('.viewport-container')).toBeVisible();

  const tauriAvailable = await page.evaluate(() => typeof (window as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ !== 'undefined');
  await testInfo.attach('runtime.json', {
    body: JSON.stringify({ tauriAvailable, requireTauri }, null, 2),
    contentType: 'application/json',
  });

  if (requireTauri) {
    expect(
      tauriAvailable,
      'Tauri IPC is required for strict smoke mode. Start/run through Tauri (desktop webview), not plain browser.'
    ).toBeTruthy();
  }

  const viewport = page.locator('.viewport-container');
  const viewportBefore = await viewport.screenshot();
  const viewportBeforeHash = sha256(viewportBefore);
  await testInfo.attach('viewport-before.png', { body: viewportBefore, contentType: 'image/png' });

  await page.fill('textarea.chat-input', prompt);
  await page.getByRole('button', { name: 'Send' }).click();

  await expect(page.getByRole('button', { name: 'Send' })).toBeVisible({ timeout: 5 * 60 * 1000 });

  const allMessageTexts = await page.$$eval(
    '.chat-message .message-body .text-content',
    (els) => els.map((e) => (e.textContent ?? '').trim()).filter(Boolean)
  );
  const latestFeedback = allMessageTexts.at(-1) ?? '';

  const viewportAfter = await viewport.screenshot();
  const viewportAfterHash = sha256(viewportAfter);
  await testInfo.attach('viewport-after.png', { body: viewportAfter, contentType: 'image/png' });
  await testInfo.attach('page-after.png', { body: await page.screenshot({ fullPage: true }), contentType: 'image/png' });
  await testInfo.attach('latest-feedback.txt', { body: latestFeedback, contentType: 'text/plain' });

  const viewportChanged = viewportBeforeHash !== viewportAfterHash;
  await testInfo.attach('summary.json', {
    body: JSON.stringify(
      {
        tauriAvailable,
        viewportChanged,
        latestFeedbackPreview: latestFeedback.slice(0, 400),
      },
      null,
      2
    ),
    contentType: 'application/json',
  });

  expect(latestFeedback.length, 'No chat feedback was produced after Send.').toBeGreaterThan(0);

  if (!tauriAvailable) {
    expect(
      latestFeedback,
      'Browser-mode run must surface a backend/IPC error explicitly.'
    ).toMatch(/Error:/i);
    return;
  }

  expect(latestFeedback, 'Generation returned a backend error.').not.toMatch(/Error:/i);

  const hasMultiPartCard = await page.locator('.multi-part-progress').isVisible().catch(() => false);
  const hasDesignPlan = await page.locator('.design-plan-summary').isVisible().catch(() => false);

  expect(
    hasMultiPartCard || hasDesignPlan || viewportChanged,
    'No multipart card, no plan block, and no viewport change detected.'
  ).toBeTruthy();
});
