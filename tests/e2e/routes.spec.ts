import { expect, test } from '@playwright/test';

/** Nothing in the overlay may paint an opaque ground over the screen. */
async function bodyIsTransparent(page: import('@playwright/test').Page) {
  return page.evaluate(() => getComputedStyle(document.body).backgroundColor);
}

test.describe('overlay routes', () => {
  for (const route of ['/island/', '/kitchen-sink/']) {
    test(`${route} renders on a transparent ground`, async ({ page }) => {
      const errors: string[] = [];
      page.on('pageerror', (error) => errors.push(error.message));

      await page.goto(route);
      await expect(page.locator('body')).toBeAttached();

      expect(await bodyIsTransparent(page)).toBe('rgba(0, 0, 0, 0)');
      expect(errors).toEqual([]);
    });
  }

  test('the island route survives having no Tauri host', async ({ page }) => {
    const errors: string[] = [];
    page.on('pageerror', (error) => errors.push(error.message));

    await page.goto('/island/');
    // No toast has arrived, so the shape is absent rather than empty black.
    await expect(page.locator('.notch')).toHaveCount(0);
    expect(errors).toEqual([]);
  });

  test('kitchen sink renders every primitive', async ({ page }) => {
    await page.goto('/kitchen-sink/');

    for (const heading of [
      'Panel',
      'PromptInput',
      'OptionList',
      'MessageBlock',
      'UsageBar',
      'TaskRow and LabelPill',
      'ScrollHint',
      'Toast',
      'Kbd',
    ]) {
      await expect(page.getByRole('heading', { name: heading, exact: true })).toBeVisible();
    }

    // Unavailable usage renders as dashes, never as an invented number.
    await expect(page.getByText('––').first()).toBeVisible();
  });
});
