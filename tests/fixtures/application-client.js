// Browser-only fixture; every request is same-origin and all payloads are public.
const boot = document.querySelector('#boot');
const run = document.querySelector('#run');
const results = document.querySelector('#results');
const summary = document.querySelector('#summary');
boot.textContent = `External JavaScript running. Page: ${location.pathname}. Secure context: ${isSecureContext}.`;
run.disabled = false;
const assert = condition => { if (!condition) throw new Error('fixture assertion'); };
const fetchChecked = async (path, init = {}) => {
  const response = await fetch(path, { ...init, signal: AbortSignal.timeout(10000) });
  assert(response.ok); return response;
};
const deadline = operation => new Promise((resolve, reject) => {
  const timer = setTimeout(() => { cleanup(); reject(new Error('timeout')); }, 10000);
  let cleanup = () => {};
  const done = (error, value) => { clearTimeout(timer); cleanup(); error ? reject(error) : resolve(value); };
  cleanup = operation(done);
});

run.addEventListener('click', async () => {
  run.disabled = true; results.replaceChildren(); summary.textContent = 'Running…';
  let passed = 0, failed = 0;
  const cases = [
    ['Module import and DOM execution', async () => assert((await import('/module.js')).answer === 42)],
    ['JSON GET and POST', async () => {
      const before = await (await fetchChecked('/api/counter')).json();
      const after = await (await fetchChecked('/api/counter', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ increment: 1 }) })).json();
      assert(after.value === before.value + 1);
    }],
    ['Parallel requests', async () => {
      const replies = await Promise.all(Array.from({ length: 6 }, () => fetchChecked('/api/counter').then(r => r.json())));
      assert(replies.every(r => Number.isInteger(r.value)));
    }],
    ['64 KiB binary download and upload integrity', async () => {
      const bytes = new Uint8Array(await (await fetchChecked('/binary')).arrayBuffer());
      assert(bytes.length === 65536 && bytes.every((v, i) => v === i % 256));
      const digest = [...new Uint8Array(await crypto.subtle.digest('SHA-256', bytes))].map(v => v.toString(16).padStart(2, '0')).join('');
      const result = await (await fetchChecked('/api/upload', { method: 'POST', body: bytes })).json();
      assert(result.bytes === 65536 && result.sha256 === digest);
    }],
    ['Host-only readable and HttpOnly cookies', async () => {
      await fetchChecked('/api/cookie/set');
      const result = await (await fetchChecked('/api/cookie/check')).json();
      assert(result.readable && result.httpOnly);
      assert(document.cookie.split(';').some(v => v.trim() === 'thepeer_fixture=public'));
      assert(!document.cookie.includes('thepeer_fixture_hidden='));
    }],
    ['Same-origin redirect', async () => {
      const response = await fetchChecked('/redirect');
      assert(response.redirected && Number.isInteger((await response.json()).value));
    }],
    ['Incremental SSE while HTTP remains usable', () => deadline(done => {
      const events = new EventSource('/events'); let first = 0, parallel = null;
      events.onmessage = async event => {
        try {
          if (event.data === 'first') { first = performance.now(); parallel = fetchChecked('/api/counter'); }
          else { assert(event.data === 'second' && first > 0 && performance.now() - first > 300); await parallel; done(null); }
        } catch (error) { done(error); }
      };
      events.onerror = () => done(new Error('SSE failed'));
      return () => events.close();
    })],
    ['WebSocket text and binary echo', () => deadline(done => {
      const socket = new WebSocket(`ws://${location.host}/socket`); socket.binaryType = 'arraybuffer';
      let stage = 0;
      socket.onopen = () => socket.send('public-fixture');
      socket.onmessage = event => {
        try {
          if (stage++ === 0) { assert(event.data === 'public-fixture'); socket.send(new Uint8Array([0, 255, 127, 128])); }
          else { const bytes = new Uint8Array(event.data); assert(bytes.length === 4 && bytes[0] === 0 && bytes[1] === 255 && bytes[2] === 127 && bytes[3] === 128); done(null); }
        } catch (error) { done(error); }
      };
      socket.onerror = () => done(new Error('WebSocket failed'));
      return () => { socket.onclose = null; socket.close(); };
    })],
  ];
  for (const [name, check] of cases) {
    const row = document.createElement('li'); row.textContent = `${name}: RUNNING`; results.append(row);
    try { await check(); row.textContent = `${name}: PASS`; row.className = 'pass'; passed++; }
    catch { row.textContent = `${name}: FAIL`; row.className = 'fail'; failed++; }
  }
  summary.textContent = `${passed} passed; ${failed} failed. Navigation/reload and actual download still require manual checks. This is not a general browser certification.`;
  run.disabled = false;
});
