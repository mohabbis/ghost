/**
 * Bundled fixture page for the demo workflow. Served by the app so the worker
 * can drive it over HTTP with no external dependency. Public (no auth) — it is
 * inert sample markup.
 */
const HTML = `<!doctype html>
<html lang="en">
  <head><meta charset="utf-8" /><title>Ghost fixture — order form</title></head>
  <body style="font-family: system-ui; max-width: 32rem; margin: 3rem auto;">
    <h1>Order form</h1>
    <label>Full name <input aria-label="Full name" id="name" type="text" /></label>
    <p>
      <button id="submit" type="button"
        onclick="document.getElementById('result').textContent = 'Order submitted'">
        Submit order
      </button>
    </p>
    <p id="result"></p>
  </body>
</html>`;

export function GET() {
  return new Response(HTML, { headers: { "content-type": "text/html; charset=utf-8" } });
}
