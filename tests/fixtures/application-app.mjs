// Public-data application fixture. No private files, credentials, external requests or request logging.
import http from 'node:http';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

const script = readFileSync(new URL('./application-client.js', import.meta.url));
const binary = Buffer.from(Array.from({ length: 65536 }, (_, i) => i % 256));
const html = `<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>thepeer.stream test application</title><link rel="stylesheet" href="/style.css"><script type="module" src="/app.js"></script><body><main><h1>thepeer.stream test application</h1><p>Public synthetic data only. Transport is owned by Android, not this page.</p><p id="boot">SCRIPT NOT RUN — this mode requires JavaScript.</p><button id="run" disabled>Run application checks</button><ol id="results"></ol><p id="summary">Not run.</p><nav><a href="/page-two">Page two</a> · <a href="/">Home</a> · <a href="/download" download="public-fixture.bin">Download public fixture</a></nav><p>Check normal navigation/reload and the actual download manually. Stop access in the APK when finished.</p><section><h2>Prototype limits</h2><p>One configured localhost app. No external resources, workers, domain cookies or external redirects. No claim of arbitrary website compatibility or production readiness.</p><p>Healthy sessions have no fixed lifetime. Concurrent requests, byte budgets, setup and operation deadlines remain bounded. Stop in Android ends access.</p></section><section><h2>Scrolling space</h2><p>Scroll normally; this is a top-level page, not an iframe.</p></section><footer>End of public fixture.</footer></main></body></html>`;
const css = 'body{font:18px system-ui;margin:0;background:#111827;color:#f9fafb}main{max-width:48rem;margin:auto;padding:1rem}a{color:#93c5fd}button{font:inherit;padding:.8rem}li{padding:.4rem;overflow-wrap:anywhere}section{min-height:60vh;padding-top:2rem}.pass{color:#86efac}.fail{color:#fca5a5}nav{line-height:2.5}';

async function body(req) {
  const chunks = []; let size = 0;
  for await (const chunk of req) { size += chunk.length; if (size > 1024 * 1024) throw new Error('limit'); chunks.push(chunk); }
  return Buffer.concat(chunks);
}
function reply(res, status, type, data, headers = {}) {
  const bytes = Buffer.isBuffer(data) ? data : Buffer.from(data);
  res.writeHead(status, { 'content-type': type, 'content-length': bytes.length, 'cache-control': 'no-store', 'x-content-type-options': 'nosniff', ...headers });
  res.end(bytes);
}
function json(res, status, value, headers) { reply(res, status, 'application/json', JSON.stringify(value), headers); }

export function createApplicationFixture() {
  let counter = 0;
  const server = http.createServer({ maxHeaderSize: 16384, requestTimeout: 10000, headersTimeout: 5000 }, async (req, res) => {
    try {
      const url = new URL(req.url, 'http://fixture.invalid');
      const path = url.pathname;
      if (req.method === 'GET' && (path === '/' || path === '/page-two')) return reply(res, 200, 'text/html; charset=utf-8', html);
      if (req.method === 'GET' && path === '/style.css') return reply(res, 200, 'text/css', css);
      if (req.method === 'GET' && path === '/app.js') return reply(res, 200, 'text/javascript', script);
      if (req.method === 'GET' && path === '/module.js') return reply(res, 200, 'text/javascript', 'export const answer = 42;');
      if (req.method === 'GET' && path === '/api/counter') return json(res, 200, { value: counter });
      if (req.method === 'POST' && path === '/api/counter') {
        if (JSON.parse((await body(req)).toString()).increment !== 1) return json(res, 400, { error: 'invalid increment' });
        return json(res, 200, { value: ++counter });
      }
      if (req.method === 'POST' && path === '/api/upload') {
        const bytes = await body(req);
        return json(res, 201, { bytes: bytes.length, sha256: createHash('sha256').update(bytes).digest('hex') });
      }
      if (req.method === 'GET' && (path === '/binary' || path === '/download')) return reply(res, 200, 'application/octet-stream', binary,
        path === '/download' ? { 'content-disposition': 'attachment; filename="public-fixture.bin"' } : {});
      if (req.method === 'GET' && path === '/api/cookie/set') return json(res, 200, { set: true }, { 'set-cookie': ['thepeer_fixture=public; Path=/; SameSite=Strict', 'thepeer_fixture_hidden=public; HttpOnly; Path=/; SameSite=Strict'] });
      if (req.method === 'GET' && path === '/api/cookie/check') {
        const values = new Set((req.headers.cookie || '').split(';').map(v => v.trim()));
        return json(res, 200, { readable: values.has('thepeer_fixture=public'), httpOnly: values.has('thepeer_fixture_hidden=public') });
      }
      if (req.method === 'GET' && path === '/redirect') return reply(res, 303, 'text/plain', '', { location: '/api/counter' });
      if (req.method === 'GET' && path === '/events') {
        res.writeHead(200, { 'content-type': 'text/event-stream', 'cache-control': 'no-store' });
        res.write('data: first\n\n');
        const timer = setTimeout(() => res.end('data: second\n\n'), 750);
        res.on('close', () => clearTimeout(timer)); return;
      }
      json(res, 404, { error: 'not found' });
    } catch { if (!res.headersSent) json(res, 400, { error: 'invalid fixture request' }); else res.destroy(); }
  });
  // Deliberately tiny echo fixture, not a production WebSocket implementation.
  const upgraded = new Set();
  server.on('upgrade', (req, socket, head) => {
    const key = req.headers['sec-websocket-key'];
    if (req.url !== '/socket' || req.headers.upgrade?.toLowerCase() !== 'websocket'
      || req.headers['sec-websocket-version'] !== '13' || typeof key !== 'string'
      || !/^[A-Za-z0-9+/]{22}==$/.test(key)
      || req.headers.origin !== `http://${req.headers.host}` || upgraded.size >= 16) { socket.destroy(); return; }
    const accept = createHash('sha1').update(key + '258EAFA5-E914-47DA-95CA-C5AB0DC85B11').digest('base64');
    socket.write(`HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Accept: ${accept}\r\n\r\n`);
    upgraded.add(socket); socket.on('close', () => upgraded.delete(socket)); socket.on('error', () => {});
    socket.setTimeout(30000, () => socket.destroy());
    let buffered = Buffer.alloc(0), total = 0;
    function receive(chunk) {
      total += chunk.length;
      if (total > 1024 * 1024 || buffered.length + chunk.length > 131100) { socket.destroy(); return; }
      buffered = Buffer.concat([buffered, chunk]);
      while (buffered.length >= 2) {
        const opcode = buffered[0] & 15;
        if ((buffered[0] & 0xf0) !== 0x80 || !(buffered[1] & 128) || ![1, 2, 8, 9, 10].includes(opcode)) { socket.destroy(); return; }
        let n = buffered[1] & 127, offset = 2;
        if (n === 126) { if (buffered.length < 4) return; n = buffered.readUInt16BE(2); offset = 4; }
        if (n === 127 || (opcode >= 8 && n > 125)) { socket.destroy(); return; }
        if (buffered.length < offset + 4 + n) return;
        const mask = buffered.subarray(offset, offset + 4), data = Buffer.from(buffered.subarray(offset + 4, offset + 4 + n));
        for (let i = 0; i < n; i++) data[i] ^= mask[i % 4];
        buffered = buffered.subarray(offset + 4 + n);
        if (opcode === 10) continue;
        const response = Buffer.alloc(n < 126 ? 2 : 4); response[0] = 128 | (opcode === 9 ? 10 : opcode); response[1] = n < 126 ? n : 126;
        if (n >= 126) response.writeUInt16BE(n, 2);
        socket.write(Buffer.concat([response, data]));
        if (opcode === 8) { socket.end(); return; }
      }
    }
    socket.on('data', receive); if (head.length) receive(head);
  });
  return { server, close() { for (const socket of upgraded) socket.destroy(); server.closeAllConnections(); return new Promise(resolve => server.close(resolve)); } };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const value = process.argv[2] ?? '3002';
  if (!/^[1-9][0-9]{0,4}$/.test(value) || Number(value) > 65535) throw new Error('Expected port 1–65535');
  const fixture = createApplicationFixture();
  fixture.server.on('error', () => { console.error('Application fixture could not listen.'); process.exitCode = 1; });
  fixture.server.listen(Number(value), '127.0.0.1', () => console.log(`Public application fixture on computer loopback port ${value}. No request logging.`));
  for (const event of ['SIGINT', 'SIGTERM']) process.once(event, () => { void fixture.close(); });
}
