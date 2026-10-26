async function controllerAcceptance(page) {
    const assert = (value, message) => { if (!value) throw new Error(message); };
    const context = await page.context().browser().newContext();
    const tab = await context.newPage();
    const errors = [];
    tab.on('pageerror', error => errors.push(error.message));
    tab.on('console', message => { if (message.type() === 'error') errors.push(message.text()); });
    try {
        await tab.goto('http://127.0.0.1:8788');
        await tab.waitForFunction(() => window.ready);
        const result = await tab.evaluate(async () => {
            const check = (value, message) => { if (!value) throw new Error(message); };
            const run = command => JSON.parse(window.demo.controller_probe(command));
            run(0);
            const editor = document.querySelector('.controller-editor');
            const input = document.querySelector('input[type=file]');
            check(editor && input, 'rich and file controls mounted');
            const token = editor.getAttribute('data-gly-node');
            let reads = 0;
            const get = editor.getAttribute;
            editor.getAttribute = function (name) {
                if (name === 'data-gly-rich-ancestor-disabled') reads++;
                return get.call(this, name);
            };
            const stage = (command, editable) => {
                const result = run(command);
                check(editor.getAttribute('contenteditable') === String(editable), 'editing eligibility at ' + command);
                check(editor.getAttribute('data-gly-node') === token, 'stable identity at ' + command);
                return result;
            };
            const before = reads;
            for (let i = 0; i < 8; i++) run(1);
            const unrelatedReads = reads - before;
            check(unrelatedReads === 0, 'unrelated labels/classes must not reconcile Rich');
            editor.dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true }));
            run(20);
            stage(1, true);
            check(run(20).ends === 0, 'unrelated commit preserves composition');
            check(stage(2, false).ends === 1, 'inert ancestor terminates composition');
            stage(3, true);
            stage(4, false);
            check(editor.getAttribute('data-gly-rich-ancestor-disabled') === '', 'fieldset disabling is inherited');
            stage(5, true);
            stage(6, false);
            stage(7, true);
            stage(10, true);
            check(editor.getAttribute('tabindex') === '-1', 'programmatic focus policy');
            stage(11, true);
            check(editor.getAttribute('tabindex') === null, 'automatic focus policy reset');
            run(8);
            check(!editor.isConnected, 'suspend detaches original editor');
            stage(9, true);
            check(document.querySelector('.controller-editor') === editor, 'resume original editor');
            stage(13, false);
            stage(14, true);
            stage(15, true);
            check(editor.textContent === 'program 😀', 'controlled document update');
            stage(16, false);
            stage(17, false);
            stage(18, true);
            stage(14, true);

            const data = new DataTransfer();
            data.items.add(new File(['abc'], 'probe.txt', { type: 'text/plain' }));
            input.files = data.files;
            input.dispatchEvent(new Event('change', { bubbles: true }));
            check(run(20).validFiles === 1, 'native file selection accepted');
            const selected = run(1);
            check(selected.validFiles === 1, 'unrelated update preserves file rights');
            run(19);
            for (let i = 0; i < 100 && run(20).read === 'pending'; i++) {
                await new Promise(resolve => setTimeout(resolve, 10));
            }
            check(run(20).read === '[97, 98, 99]', 'selected file bytes remain readable');
            const retired = run(12);
            check(retired.validFiles === 0, 'configuration revision revokes selected tokens');
            check(input === document.querySelector('input[type=file]'), 'file control identity retained');
            const oldEditor = editor;
            window.demo.controller_probe(255);
            check(!oldEditor.isConnected && !input.isConnected, 'all controls removed');
            check(document.querySelector('#app').childNodes.length === 0, 'application released');
            return { unrelatedReads, inheritedChanges: true, compositionEnds: retired.ends, fileRead: selected.files, stableIdentities: true };
        });
        assert(errors.length === 0, JSON.stringify(errors));
        return result;
    } finally {
        await context.close();
    }
}
