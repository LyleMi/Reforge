import { copyFile, mkdir } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";

const appDir = path.dirname(fileURLToPath(import.meta.url));
const rootAssets = path.resolve(appDir, "../../assets");
const packageAssets = path.resolve(appDir, "../../crates/reforge-output/assets");

await mkdir(packageAssets, { recursive: true });
for (const file of ["report-app.css", "report-app.js"]) {
  await copyFile(path.join(rootAssets, file), path.join(packageAssets, file));
}
