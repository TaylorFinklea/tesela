/**
 * Self-registering synchronous resolve hook so plain `node --import` can run
 * the C2.2 convergence check against the REAL app `.ts` modules
 * (`note-doc.ts` → `loro-client.ts`), which use two bundler-isms node doesn't
 * resolve on its own:
 *
 *   - extensionless relative imports (`./loro-client`)  → try `.ts`/`.js`/`.mjs`
 *   - the SvelteKit `$app/environment` alias             → a node stub where
 *     `browser` is true (this script runs in a node context that CAN load the
 *     loro wasm, so the browser-only code paths must execute)
 *
 * Node strips the TS types natively (v23+); this hook only fixes resolution.
 * `registerHooks` runs the hook synchronously on the main thread (no worker,
 * no deprecation). Register with `node --import ./scripts/ts-resolve-hook.mjs`.
 */
import { registerHooks } from "node:module";
import { fileURLToPath, pathToFileURL } from "node:url";
import { existsSync } from "node:fs";
import { dirname, resolve as resolvePath } from "node:path";

const APP_ENV_STUB = "data:text/javascript,export const browser = true;";

registerHooks({
  resolve(specifier, context, nextResolve) {
    if (specifier === "$app/environment") {
      return { url: APP_ENV_STUB, shortCircuit: true };
    }
    if (
      (specifier.startsWith("./") || specifier.startsWith("../")) &&
      !/\.[mc]?[jt]s$/.test(specifier)
    ) {
      const parentURL = context.parentURL;
      if (parentURL && parentURL.startsWith("file:")) {
        const baseDir = dirname(fileURLToPath(parentURL));
        for (const ext of [".ts", ".js", ".mjs"]) {
          const candidate = resolvePath(baseDir, specifier + ext);
          if (existsSync(candidate)) {
            return {
              url: pathToFileURL(candidate).href,
              shortCircuit: true,
            };
          }
        }
      }
    }
    return nextResolve(specifier, context);
  },
});
