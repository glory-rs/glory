async (hostPage) => {
  const context = await hostPage.context().browser().newContext();
  const errors = [], results = [];
  const assert = (value, message) => { if (!value) throw new Error(message); };
  try {
    const page = await context.newPage();
    page.on('pageerror', error => errors.push(error.message));
    // Instrument real host requests without counting the harness's settle RAFs.
    await page.addInitScript(() => {
      window.nativeRAF = window.requestAnimationFrame.bind(window);
      window.hostRAFs = 0;
      window.requestAnimationFrame = callback => { window.hostRAFs++; return window.nativeRAF(callback); };
    });
    const settle = () => page.evaluate(() => new Promise(resolve => window.nativeRAF(() => window.nativeRAF(resolve))));
    for (const count of [0, 1, 1000]) {
      await page.goto('http://127.0.0.1:8788/');
      await page.waitForFunction(() => window.ready === true);
      await page.evaluate(count => window.demo.translation_mount(count), count);
      await settle();
      assert(await page.locator('.translation-node').count() === count, 'translation membership');
      const local = await page.evaluate(async () => {
        const input = document.querySelector('.translation-input');
        const animations = [...document.querySelectorAll('.translation-node')].map(e => e.getAnimations()[0]);
        const before = window.hostRAFs;
        for (let i = 0; i < 10; i++) {
          window.demo.translation_update();
          await new Promise(resolve => window.nativeRAF(() => window.nativeRAF(resolve)));
        }
        return { rafs: window.hostRAFs - before, value: input.value,
          identity: input === document.querySelector('.translation-input'),
          animationsPreserved: [...document.querySelectorAll('.translation-node')].every((e, i) => e.getAnimations()[0] === animations[i]) };
      });
      assert(local.rafs === 0 && local.identity && local.animationsPreserved && local.value === 'updated 10', `unrelated patch: ${JSON.stringify({count,local})}`);
      if (count) {
        const active = () => page.evaluate(() => [...document.querySelectorAll('.translation-node')].filter(e => e.getAnimations().length).length);
        assert(await active() === count, 'initial loop animations');
        for (const [stage, expected] of [[0,0],[1,count],[2,0],[3,count],[4,0],[5,count]]) {
          await page.evaluate(stage => window.demo.translation_stage(stage), stage);
          await settle();
          assert(await active() === expected, `motion stage ${stage}`);
        }
        await page.emulateMedia({ reducedMotion: 'reduce' }); await settle();
        assert(await active() === 0, 'reduced-motion cancellation');
        await page.emulateMedia({ reducedMotion: 'no-preference' }); await settle();
        assert(await active() === count, 'reduced-motion recovery');
      }
      await page.evaluate(() => window.demo.unmount()); await settle();
      assert(await page.locator('#app [data-gly-kind]').count() === 0, 'translation cleanup');
      results.push({count,local,lifecycle:true,reducedMotion:true,cleanup:true});
    }
    assert(errors.length === 0, JSON.stringify(errors));
    return {results,errors};
  } finally { await context.close(); }
}
