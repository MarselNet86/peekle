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

  /// A drag that crosses the window used to select its markup, and the
  /// highlight paints elements rather than text: the transparent parts of the
  /// window went grey with it, a sheet across half the screen. tech.md 9.
  test('the overlay itself cannot be selected', async ({ page }) => {
    await page.goto('/island/');
    // Vite injects the styles as its modules run, so the shape being on
    // screen is what says the stylesheet has arrived.
    await expect(page.locator('.shape')).toBeVisible();

    const selectable = await page.evaluate(() => {
      const style = getComputedStyle(document.body);
      return style.userSelect || style.webkitUserSelect;
    });
    expect(selectable).toBe('none');
  });

  /// Off by default, back on where text is taken by hand.
  test('a message stays selectable', async ({ page }) => {
    await page.goto('/kitchen-sink/');
    await expect(page.locator('.bubble').first()).toBeVisible();

    const message = await page
      .locator('.bubble')
      .first()
      .evaluate((node) => {
        const style = getComputedStyle(node);
        return style.userSelect || style.webkitUserSelect;
      });
    expect(message).toBe('text');
  });

  test('the island route survives having no Tauri host', async ({ page }) => {
    const errors: string[] = [];
    page.on('pageerror', (error) => errors.push(error.message));

    await page.goto('/island/');
    // No event has arrived, so the island rests: the mark and nothing else.
    await expect(page.locator('.shape')).toHaveAttribute('data-view', 'Collapsed');
    await expect(page.locator('.rest')).toBeVisible();
    expect(errors).toEqual([]);
  });

  /// S12 acceptance, as far as a browser can carry it: the mark is on screen
  /// and it is a control. Whether the panel then takes the click is native and
  /// belongs to the checklist of section 15.
  test('a resting island shows a mark that opens the session list', async ({ page }) => {
    const errors: string[] = [];
    page.on('pageerror', (error) => errors.push(error.message));

    await page.goto('/island/');
    const mark = page.getByRole('button', { name: /Open the session list/ });

    await expect(mark).toBeVisible();
    // Outside the app shell there is no Tauri to answer, so the intent goes
    // nowhere. What matters here is that pressing it is not an error.
    await mark.click();
    expect(errors).toEqual([]);
  });

  test('the island shape grows and collapses without the window moving', async ({ page }) => {
    await page.goto('/kitchen-sink/');
    const shape = page.locator('.stage').first().locator('.shape');

    await page.getByRole('button', { name: 'Collapsed', exact: true }).click();
    const collapsed = await shape.boundingBox();

    await page.getByRole('button', { name: 'Sessions', exact: true }).click();
    await expect
      .poll(async () => (await shape.boundingBox())?.height ?? 0)
      .toBeGreaterThan(collapsed?.height ?? 0);

    await page.getByRole('button', { name: 'Collapsed', exact: true }).click();
    await expect
      .poll(async () => (await shape.boundingBox())?.height ?? 0)
      .toBeLessThanOrEqual((collapsed?.height ?? 0) + 1);
  });

  test('kitchen sink renders every primitive', async ({ page }) => {
    await page.goto('/kitchen-sink/');

    for (const heading of [
      'Shape',
      'RestMark',
      'Message formatting',
      'SearchField',
      'UsageDial',
      'AgentBar',
      'UsageCorner',
      'PickerMenu',
      'Objects with a body',
      'WorkLine',
      'ShotPrompt',
      'ShotChip',
      'ShotBlock',
      'ShotPreview',
      'QuestionPrompt',
      'PromptInput',
      'OptionList',
      'MessageBlock',
      'UsageBar',
      'TaskRow and LabelPill',
      'FeedRow',
      'SessionRow',
      'PermissionRow',
      'AskPanel',
      'Button',
      'FeedRow',
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
