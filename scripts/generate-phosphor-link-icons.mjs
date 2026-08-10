import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const projectRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);
const packageRoot = path.dirname(
  fileURLToPath(import.meta.resolve("@phosphor-icons/react/package.json")),
);
const definitionsRoot = path.join(packageRoot, "dist/defs");
const outputPath = path.join(
  projectRoot,
  "web/src/components/resource/detail/phosphorRegularIcons.generated.json",
);
function extractRegularPath(value, iconName) {
  const children = value?.type === Symbol.for("react.fragment")
    ? value.props.children
    : value;
  const nodes = Array.isArray(children) ? children : [children];
  if (
    nodes.length !== 1 ||
    nodes[0]?.type !== "path" ||
    Object.keys(nodes[0].props).length !== 1 ||
    typeof nodes[0].props.d !== "string"
  ) {
    throw new Error(`Unsupported regular icon structure: ${iconName}`);
  }
  return nodes[0].props.d;
}

const files = (await fs.readdir(definitionsRoot))
  .filter((file) => file.endsWith(".es.js"))
  .sort();
const registry = {};

for (const file of files) {
  const iconName = file.slice(0, -6);
  const definition = await import(
    pathToFileURL(path.join(definitionsRoot, file)).href
  );
  registry[iconName] = extractRegularPath(
    definition.default.get("regular"),
    iconName,
  );
}

await fs.writeFile(outputPath, `${JSON.stringify(registry)}\n`);
console.log(`Generated ${files.length} Phosphor link icons.`);
