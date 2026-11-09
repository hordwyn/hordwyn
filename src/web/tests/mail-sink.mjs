// A mail sink for the e2e tests (phase 4 WP4-2): a plain SMTP listener (RFC 5321 commands
// EHLO/HELO, MAIL, RCPT, DATA, RSET, NOOP, QUIT; no TLS, no AUTH needed) that keeps every
// message in memory, and an HTTP side to read them: `GET /messages?to=<address>` answers
// `[{ "to": [...], "data": "<the message>" }]`, newest last; `GET /health` answers `ok`.
// Written from RFC 5321 only; no npm packages.
import { createServer as createTcp } from "node:net";
import { createServer as createHttp } from "node:http";

const smtpPort = Number(process.env.MAIL_SINK_SMTP_PORT ?? 3013);
const httpPort = Number(process.env.MAIL_SINK_HTTP_PORT ?? 3014);
const messages = [];

createTcp((sock) => {
  let buffer = "";
  let rcpt = [];
  let data = null;
  const reply = (line) => sock.write(`${line}\r\n`);
  reply("220 mail-sink ESMTP");
  sock.on("data", (chunk) => {
    buffer += chunk.toString("utf8");
    let at;
    while ((at = buffer.indexOf("\r\n")) >= 0) {
      const line = buffer.slice(0, at);
      buffer = buffer.slice(at + 2);
      if (data !== null) {
        if (line === ".") {
          messages.push({ to: rcpt, data: data.join("\n") });
          data = null;
          rcpt = [];
          reply("250 queued");
        } else {
          // RFC 5321 4.5.2: a leading dot was doubled by the sender
          data.push(line.startsWith("..") ? line.slice(1) : line);
        }
        continue;
      }
      const cmd = line.slice(0, 4).toUpperCase();
      if (cmd === "EHLO" || cmd === "HELO") reply("250 mail-sink");
      else if (cmd === "MAIL") reply("250 ok");
      else if (cmd === "RCPT") {
        const m = /<([^>]*)>/.exec(line);
        if (m) rcpt.push(m[1].toLowerCase());
        reply("250 ok");
      } else if (cmd === "DATA") {
        data = [];
        reply("354 end with <CRLF>.<CRLF>");
      } else if (cmd === "RSET") {
        rcpt = [];
        reply("250 ok");
      } else if (cmd === "QUIT") {
        reply("221 bye");
        sock.end();
      } else reply("250 ok");
    }
  });
  sock.on("error", () => {});
}).listen(smtpPort, "127.0.0.1");

createHttp((req, res) => {
  const url = new URL(req.url, `http://127.0.0.1:${httpPort}`);
  if (url.pathname === "/health") {
    res.writeHead(200, { "content-type": "text/plain" });
    res.end("ok");
    return;
  }
  if (url.pathname === "/messages") {
    const to = (url.searchParams.get("to") ?? "").toLowerCase();
    const list = to ? messages.filter((m) => m.to.includes(to)) : messages;
    res.writeHead(200, { "content-type": "application/json" });
    res.end(JSON.stringify(list));
    return;
  }
  res.writeHead(404);
  res.end();
}).listen(httpPort, "127.0.0.1");
