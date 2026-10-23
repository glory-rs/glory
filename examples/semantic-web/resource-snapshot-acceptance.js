async (hostPage) => {
  const context = await hostPage.context().browser().newContext({ viewport: { width: 1280, height: 900 }, deviceScaleFactor: 1 });
  try {
    const page = await context.newPage(); await page.bringToFront();
    const check = (value, message) => { if (!value) throw new Error(message); };
    const errors = []; page.on('pageerror', error => errors.push(error.message));
    page.on('console', message => { if (message.type() === 'error') errors.push(message.text()); });
    const open = async () => { await page.goto('http://127.0.0.1:8788/resource-ssr.html'); await page.waitForFunction(() => window.ready); };
    await open();
    const server = await page.evaluate(() => {
      window.originalElements = [...document.querySelectorAll('#resource-ssr [data-gly-key]')];
      return { profile: document.querySelector('.snapshot-profile').textContent, error: document.querySelector('.snapshot-problem').textContent,
        injected: window.payloadInjected === 1, scriptCount: document.querySelectorAll('script').length,
        records: JSON.parse(document.getElementById('resource-data').textContent).records.length,
        raw: document.getElementById('resource-data').textContent };
    });
    check(!server.injected && server.scriptCount === 2 && server.records === 2, JSON.stringify(server));
    check(server.profile.startsWith('</script><script>window.payloadInjected=1</script>'), 'unsafe-looking value is literal text');
    check(!server.raw.includes('<') && !server.raw.includes('&'), 'raw script data escaped without HTML entities');
    check(server.error === 'failed:server error', 'server business failure rendered');
    await page.evaluate(() => window.demo.resource_hydrate());
    const adopted = await page.evaluate(() => ({ ...JSON.parse(window.demo.resource_snapshot_stats()),
      identity: window.originalElements.every(node => node.isConnected), profile: document.querySelector('.snapshot-profile').textContent }));
    check(adopted.loads === 0 && adopted.tasks === 0 && adopted.registered === 2 && adopted.remaining === 0, JSON.stringify(adopted));
    check(adopted.identity && adopted.profile === server.profile, 'resource data and existing DOM adopted');
    check(await page.locator('#resource-data').count() === 0, 'successful adoption releases serialized data block');
    await page.locator('.snapshot-refresh').click();
    await page.waitForFunction(() => document.querySelector('.snapshot-profile').textContent === 'client:7');
    check(await page.evaluate(() => JSON.parse(window.demo.resource_snapshot_stats()).loads) === 1, 'explicit refresh really loaded');
    await page.evaluate(() => window.demo.resource_snapshot_key(8n));
    await page.waitForFunction(() => document.querySelector('.snapshot-profile').textContent === 'client:8');
    check(await page.evaluate(() => JSON.parse(window.demo.resource_snapshot_stats()).loads) === 2, 'new key does not reuse old seed');
    check(await page.locator('.snapshot-problem').textContent() === 'failed:server error', 'unrelated seed stayed terminal');
    await page.evaluate(() => window.demo.unmount());
    check(await page.locator('#resource-ssr [data-gly-node]').count() === 0, 'runtime and DOM cleaned');
    const rejected = [];
    for (const kind of ['version', 'block', 'duplicate', 'bytes', 'schema', 'type', 'structure']) {
      await open();
      await page.evaluate(kind => {
        window.rootBefore = document.getElementById('resource-ssr').innerHTML;
        window.payloadBefore = document.getElementById('resource-data').textContent;
        const block = document.getElementById('resource-data'); const data = JSON.parse(block.textContent);
        if (kind === 'version') { data.version = 999; block.textContent = JSON.stringify(data); }
        if (kind === 'block') block.setAttribute('data-gly-resources', '999');
        if (kind === 'duplicate') { data.records.push(data.records[0]); block.textContent = JSON.stringify(data); }
        if (kind === 'schema') { data.records.find(record => record.name === 'profile').schema = 'profile/999'; block.textContent = JSON.stringify(data); }
        if (kind === 'type') { data.records.find(record => record.name === 'problem').result.value = 999; block.textContent = JSON.stringify(data); }
        if (kind === 'bytes') block.textContent = ' '.repeat(8 * 1024 * 1024 + 1);
        if (kind === 'structure') document.querySelector('.snapshot-refresh').setAttribute('title', 'unexpected');
        window.modifiedBefore = document.getElementById('resource-ssr').innerHTML;
      }, kind);
      const result = await page.evaluate(() => {
        let failure; try { window.demo.resource_hydrate(); } catch (error) { failure = String(error); }
        return { failure, intact: document.getElementById('resource-ssr').innerHTML === window.modifiedBefore };
      });
      check(result.failure && result.intact, `rejected ${kind} without DOM loss: ${JSON.stringify(result)}`); rejected.push(kind);
      await page.evaluate(() => {
        const block = document.getElementById('resource-data'); block.textContent = window.payloadBefore; block.setAttribute('data-gly-resources', '1');
        document.querySelector('.snapshot-refresh').removeAttribute('title'); window.demo.resource_hydrate();
      });
      check(await page.evaluate(() => JSON.parse(window.demo.resource_snapshot_stats()).loads) === 0, 'fresh context retries from original payload');
      await page.evaluate(() => window.demo.unmount());
    }
    check(errors.length === 0, errors.join('\n'));
    return { server: { profile: server.profile, error: server.error, records: server.records, injected: server.injected, scriptCount: server.scriptCount }, adopted, rejected, errors };
  } finally { await context.close(); }
}
