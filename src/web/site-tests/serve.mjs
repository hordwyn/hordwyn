// Static file server for the built website, the way GitLab Pages serves it: `/x/` is
// `/x/index.html`, an unknown path answers 404 with `/404.html`.
import { createServer } from "node:http";
import { existsSync } from "node:fs";
import { readFile, stat } from "node:fs/promises";
import { extname, join, normalize, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = fileURLToPath(new URL(".", import.meta.url));
const root = resolve(process.env.SITE_DIR ?? join(here, "../../../build/public"));
const port = Number(process.env.SITE_PORT ?? 3500);
if (!existsSync(join(root, "404.html"))) {
  console.error(`no built website in ${root}: run \`cargo xtask site\` first (or set SITE_DIR)`);
  process.exit(1);
}
const types = {
  ".html": "text/html; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".txt": "text/plain; charset=utf-8",
  ".xml": "application/xml",
  ".woff2": "font/woff2",
  ".png": "image/png",
};

async function file(path) {
  const info = await stat(path).catch(() => null);
  if (info?.isDirectory()) return file(join(path, "index.html"));
  return info?.isFile() ? path : null;
}

createServer(async (req, res) => {
  const url = new URL(req.url, "http://localhost");
  const wanted = normalize(join(root, decodeURIComponent(url.pathname)));
  const found = wanted.startsWith(root) ? await file(wanted) : null;
  const path = found ?? join(root, "404.html");
  res.writeHead(found ? 200 : 404, {
    "content-type": types[extname(path)] ?? "application/octet-stream",
  });
  res.end(await readFile(path));
}).listen(port, "127.0.0.1");
