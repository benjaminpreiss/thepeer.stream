import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { once } from 'node:events';
import { connect } from 'node:net';
import { createApplicationFixture } from '../../../tests/fixtures/application-app.mjs';

async function start(t) {
  const fixture = createApplicationFixture();
  fixture.server.listen(0, '127.0.0.1'); await once(fixture.server, 'listening');
  t.after(() => fixture.close());
  return fixture.server.address().port;
}

test('public application fixture supplies scripts, writes, cookies, redirects and binary integrity', { timeout: 10000 }, async t => {
  const port = await start(t), base = `http://127.0.0.1:${port}`;
  const get = path => fetch(base + path);
  const html = await (await get('/')).text();
  assert.match(html, /SCRIPT NOT RUN/); assert.match(html, /type="module" src="\/app.js"/);
  assert.match(html, /Healthy sessions have no fixed lifetime/);
  assert.doesNotMatch(html, /120 seconds|ten-minute session/);
  assert.equal((await get('/app.js')).headers.get('content-type'), 'text/javascript');
  assert.equal((await get('/module.js')).status, 200);
  assert.deepEqual(await (await get('/api/counter')).json(), { value: 0 });
  assert.deepEqual(await (await fetch(base + '/api/counter', { method: 'POST', body: '{"increment":1}' })).json(), { value: 1 });
  assert.equal((await fetch(base + '/api/counter', { method: 'POST', body: 'invalid' })).status, 400);
  const bytes = Buffer.from(await (await get('/binary')).arrayBuffer());
  assert.equal(bytes.length, 65536); assert.ok(bytes.every((b,i) => b === i % 256));
  const upload = await fetch(base + '/api/upload', { method: 'POST', body: bytes });
  assert.equal(upload.status, 201);
  assert.deepEqual(await upload.json(), { bytes: 65536, sha256: createHash('sha256').update(bytes).digest('hex') });
  const cookies = (await get('/api/cookie/set')).headers.getSetCookie(); assert.equal(cookies.length, 2);
  assert.deepEqual(await (await fetch(base + '/api/cookie/check', { headers: { Cookie: 'thepeer_fixture=public; thepeer_fixture_hidden=public' } })).json(), { readable: true, httpOnly: true });
  const redirected = await get('/redirect'); assert.ok(redirected.redirected); assert.equal(redirected.status, 200);
  assert.match((await get('/download')).headers.get('content-disposition'), /attachment/);
  assert.equal((await get('/missing')).status, 404);
});

test('public fixture produces incremental SSE and actual WebSocket upgrade/binary echo', { timeout: 10000 }, async t => {
  const port = await start(t), base = `http://127.0.0.1:${port}`;
  const stream = await fetch(base + '/events'); const reader = stream.body.getReader();
  assert.equal(new TextDecoder().decode((await reader.read()).value), 'data: first\n\n');
  assert.equal((await fetch(base + '/api/counter')).status, 200);
  assert.equal(new TextDecoder().decode((await reader.read()).value), 'data: second\n\n');
  assert.ok((await reader.read()).done);
  const socket = connect(port, '127.0.0.1'); t.after(() => socket.destroy()); await once(socket, 'connect');
  let buffered = Buffer.alloc(0); socket.on('data', bytes => { buffered = Buffer.concat([buffered, bytes]); });
  const wait = async predicate => {
    const deadline = Date.now() + 3000;
    while (!predicate()) { assert.ok(Date.now() < deadline, 'fixture response deadline'); await new Promise(r => setTimeout(r, 10)); }
  };
  socket.write(`GET /socket HTTP/1.1\r\nHost: 127.0.0.1:${port}\r\nOrigin: ${base}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n`);
  await wait(() => buffered.includes('\r\n\r\n'));
  assert.match(buffered.toString(), /^HTTP\/1.1 101/);
  assert.match(buffered.toString(), /s3pPLMBiTxaQ9kYGzzhZRbK\+xOo=/);
  buffered = buffered.subarray(buffered.indexOf('\r\n\r\n') + 4);
  socket.write(Buffer.from([0x82,0x84,1,2,3,4,1,253,124,132]));
  await wait(() => buffered.length >= 6);
  assert.deepEqual(buffered, Buffer.from([0x82,4,0,255,127,128]));
});
