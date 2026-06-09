// Framework-agnostic benchmark harness.
//
// Every app under benchmarks/ renders the SAME DOM and exposes the SAME
// control buttons, so this one script can drive glory, leptos and dioxus
// identically. It only ever talks to the DOM — it never touches framework
// internals — which is what keeps the comparison fair.
//
// DOM contract each app must satisfy:
//   buttons:  #run #runlots #add #update #clear #swaprows
//   table:    table.test-data > tbody > tr
//   per row:  td.col-md-1            -> row id (text)
//             td.col-md-4 > a.lbl    -> label; click selects the row
//             td.col-md-1 > a.remove -> click removes the row
//   selected row carries class "danger".
//
// Methodology: for each operation we warm up, then take N measured samples.
// A sample is t0 = performance.now() right before the click, t1 = after two
// requestAnimationFrame callbacks (i.e. after the browser has had a chance to
// run layout + paint of the resulting frame). We report the median, which is
// what the official js-framework-benchmark uses to reject outliers. These are
// relative numbers meant for comparing the three apps on one machine — not a
// substitute for the official Chrome-tracing harness.

const WARMUP = 3;
const SAMPLES = 12;

const $ = (sel) => document.querySelector(sel);
const rows = () => document.querySelectorAll("table.test-data tbody tr");

// One animation frame, but with a setTimeout fallback so the harness still
// makes progress in a backgrounded / headless tab where requestAnimationFrame
// is throttled or paused (otherwise a non-visible tab can stall forever).
const nextFrame = () =>
  new Promise((r) => {
    let done = false;
    const fin = () => {
      if (!done) {
        done = true;
        r();
      }
    };
    requestAnimationFrame(fin);
    setTimeout(fin, 100);
  });
// Two frames: the click handler mutates the DOM in frame A; frame B fires
// after the browser has laid out and painted A.
const afterPaint = async () => {
  await nextFrame();
  await nextFrame();
};

function click(elOrSel) {
  const el = typeof elOrSel === "string" ? $(elOrSel) : elOrSel;
  if (!el) throw new Error(`bench: element not found: ${elOrSel}`);
  el.click();
}

// Drives one operation: `setup` puts the app in the required precondition
// (not timed), then we time a single `action`.
async function sample(setup, action) {
  if (setup) {
    await setup();
    await afterPaint();
  }
  const t0 = performance.now();
  action();
  await afterPaint();
  return performance.now() - t0;
}

function stats(times) {
  const sorted = [...times].sort((a, b) => a - b);
  const median = sorted[Math.floor(sorted.length / 2)];
  const mean = times.reduce((a, b) => a + b, 0) / times.length;
  return { median, mean, min: sorted[0], max: sorted[sorted.length - 1] };
}

const create1k = () => click("#run");
const clearAll = () => click("#clear");

// Each entry: a precondition (run before every sample, untimed) + the timed action.
const BENCHES = [
  {
    key: "create_1k",
    name: "create 1,000 rows",
    setup: () => clearAll(),
    action: create1k,
  },
  {
    key: "replace_1k",
    name: "replace all 1,000 rows",
    setup: () => create1k(),
    action: create1k,
  },
  {
    key: "update_10th",
    name: "partial update (every 10th of 1,000)",
    setup: () => create1k(),
    action: () => click("#update"),
  },
  {
    key: "select_row",
    name: "select a row",
    setup: () => create1k(),
    action: () => click(rows()[1].querySelector("a.lbl")),
  },
  {
    key: "swap_rows",
    name: "swap two rows",
    setup: () => create1k(),
    action: () => click("#swaprows"),
  },
  {
    key: "remove_row",
    name: "remove one row",
    setup: () => create1k(),
    action: () => click(rows()[1].querySelector("a.remove")),
  },
  {
    key: "append_1k",
    name: "append 1,000 to 1,000 rows",
    setup: () => create1k(),
    action: () => click("#add"),
  },
  {
    key: "create_10k",
    name: "create 10,000 rows",
    setup: () => clearAll(),
    action: () => click("#runlots"),
  },
  {
    key: "clear_1k",
    name: "clear 1,000 rows",
    setup: () => create1k(),
    action: () => clearAll(),
  },
];

async function runBench(b) {
  for (let i = 0; i < WARMUP; i++) await sample(b.setup, b.action);
  const times = [];
  for (let i = 0; i < SAMPLES; i++) times.push(await sample(b.setup, b.action));
  return { key: b.key, name: b.name, ...stats(times), samples: SAMPLES };
}

function render(results) {
  let box = $("#bench-results");
  if (!box) {
    box = document.createElement("div");
    box.id = "bench-results";
    document.body.appendChild(box);
  }
  const fmt = (n) => n.toFixed(2);
  box.innerHTML =
    `<h3>Results — ${document.title} (median of ${SAMPLES})</h3>` +
    "<table border='1' cellpadding='4' style='border-collapse:collapse'>" +
    "<thead><tr><th>operation</th><th>median ms</th><th>mean ms</th>" +
    "<th>min</th><th>max</th></tr></thead><tbody>" +
    results
      .map(
        (r) =>
          `<tr><td>${r.name}</td><td>${fmt(r.median)}</td><td>${fmt(
            r.mean,
          )}</td><td>${fmt(r.min)}</td><td>${fmt(r.max)}</td></tr>`,
      )
      .join("") +
    "</tbody></table>";
}

export async function runAll() {
  const results = [];
  for (const b of BENCHES) {
    const r = await runBench(b);
    results.push(r);
    render(results);
    // eslint-disable-next-line no-console
    console.log(`${r.name.padEnd(38)} ${r.median.toFixed(2)} ms (median)`);
  }
  console.table(results.map(({ name, median, mean, min, max }) => ({ name, median, mean, min, max })));
  clearAll();
  window.__BENCH_RESULTS__ = results;
  window.dispatchEvent(new CustomEvent("bench-done", { detail: results }));
  return results;
}

function mountButton() {
  const btn = document.createElement("button");
  btn.id = "bench-run-all";
  btn.textContent = "▶ Run all benchmarks";
  btn.style.cssText = "margin:8px;padding:8px 16px;font-size:16px";
  btn.addEventListener("click", () => {
    btn.disabled = true;
    btn.textContent = "running…";
    runAll().finally(() => {
      btn.disabled = false;
      btn.textContent = "▶ Run all benchmarks";
    });
  });
  document.body.insertBefore(btn, document.body.firstChild);
}

// Wait until the app has mounted its control buttons before wiring up.
function whenReady(cb) {
  const tick = () => ($("#run") ? cb() : setTimeout(tick, 30));
  tick();
}

whenReady(() => {
  mountButton();
  if (new URLSearchParams(location.search).has("autorun")) runAll();
});
