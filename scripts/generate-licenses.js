import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);
const outputPath = path.join(
  projectRoot,
  "web/src/generated/openSourceLicenses.json",
);
const documentPattern = /^(?:licen[cs]e|copying|notice|unlicense)(?:[._-].*)?$/i;
const documents = new Map();

function runJson(command, args) {
  const output = execFileSync(command, args, {
    cwd: projectRoot,
    encoding: "utf8",
    maxBuffer: 128 * 1024 * 1024,
    stdio: ["ignore", "pipe", "inherit"],
  });
  return JSON.parse(output);
}

function normalizeText(value) {
  return value.replaceAll("\r\n", "\n").trimEnd() + "\n";
}

function addDocument(filePath, displayName = path.basename(filePath)) {
  if (!filePath || !fs.existsSync(filePath) || !fs.statSync(filePath).isFile()) {
    return null;
  }
  const text = normalizeText(fs.readFileSync(filePath, "utf8"));
  if (!text.trim()) return null;
  const id = createHash("sha256").update(text).digest("hex").slice(0, 16);
  if (!documents.has(id)) {
    documents.set(id, { id, name: displayName, text });
  }
  return id;
}

function findPackageDocuments(packageDir, explicitLicenseFile) {
  const candidates = new Set();
  if (explicitLicenseFile) {
    const resolved = path.isAbsolute(explicitLicenseFile)
      ? explicitLicenseFile
      : path.resolve(packageDir, explicitLicenseFile);
    candidates.add(resolved);
  }
  if (fs.existsSync(packageDir)) {
    for (const entry of fs.readdirSync(packageDir, { withFileTypes: true })) {
      if (entry.isFile() && documentPattern.test(entry.name)) {
        candidates.add(path.join(packageDir, entry.name));
      }
    }
  }
  return [...candidates]
    .sort((a, b) => path.basename(a).localeCompare(path.basename(b), "en"))
    .map((filePath) => addDocument(filePath))
    .filter(Boolean);
}

function normalizePerson(value) {
  if (typeof value === "string") return value;
  if (!value || typeof value !== "object") return undefined;
  const name = typeof value.name === "string" ? value.name : "";
  const email = typeof value.email === "string" ? ` <${value.email}>` : "";
  return `${name}${email}`.trim() || undefined;
}

function normalizeRepository(value) {
  const raw =
    typeof value === "string"
      ? value
      : value && typeof value === "object" && typeof value.url === "string"
        ? value.url
        : undefined;
  if (!raw) return undefined;
  const normalized = raw
    .replace(/^git\+/, "")
    .replace(/^git:\/\/github\.com\//, "https://github.com/")
    .replace(/^ssh:\/\/git@github\.com\//, "https://github.com/")
    .replace(/^git@github\.com:/, "https://github.com/")
    .replace(/\.git$/, "");
  return /^https?:\/\//.test(normalized) ? normalized : undefined;
}

function detectLicense(filePath) {
  if (!filePath || !fs.existsSync(filePath)) return null;
  const text = fs.readFileSync(filePath, "utf8");
  if (/GNU AFFERO GENERAL PUBLIC LICENSE[\s\S]{0,80}Version 3/i.test(text)) {
    return "AGPL-3.0";
  }
  if (/^MIT License\b/im.test(text)) return "MIT";
  if (/Apache License[\s\S]{0,80}Version 2\.0/i.test(text)) {
    return "Apache-2.0";
  }
  const astroBoxLicense = text.match(
    /^(AstroBox-NG Non-Commercial Community Source License)\s*\nVersion\s+([^\n]+)/i,
  );
  if (astroBoxLicense) return `${astroBoxLicense[1]} ${astroBoxLicense[2]}`;
  return null;
}

function readRepositoryVersion(repositoryPath) {
  const packageJsonPath = path.join(repositoryPath, "package.json");
  if (fs.existsSync(packageJsonPath)) {
    const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, "utf8"));
    if (typeof packageJson.version === "string") return packageJson.version;
  }
  const cargoManifestPath = path.join(repositoryPath, "Cargo.toml");
  if (fs.existsSync(cargoManifestPath)) {
    const cargoManifest = fs.readFileSync(cargoManifestPath, "utf8");
    let inPackageSection = false;
    for (const line of cargoManifest.split("\n")) {
      if (line.trim() === "[package]") {
        inPackageSection = true;
        continue;
      }
      if (inPackageSection && /^\s*\[/.test(line)) break;
      if (!inPackageSection) continue;
      const version = line.match(/^\s*version\s*=\s*"([^"]+)"\s*$/)?.[1];
      if (version) return version;
    }
  }
  return "";
}

function parseRepositories() {
  const manifest = fs.readFileSync(path.join(projectRoot, "repos.xml"), "utf8");
  return [...manifest.matchAll(/<repo\b([\s\S]*?)\/>/g)]
    .map((match) =>
      Object.fromEntries(
        [...match[1].matchAll(/([a-zA-Z]+)="([^"]*)"/g)].map((attribute) => [
          attribute[1],
          attribute[2],
        ]),
      ),
    )
    .map((repository) => {
      const repositoryPath = path.resolve(projectRoot, repository.path);
      if (!repositoryPath.startsWith(`${projectRoot}${path.sep}`)) {
        throw new Error(`Repository path is outside project root: ${repository.path}`);
      }
      const licenseFiles = fs.existsSync(repositoryPath)
        ? fs
            .readdirSync(repositoryPath, { withFileTypes: true })
            .filter(
              (entry) => entry.isFile() && /^licen[cs]e(?:[._-].*)?$/i.test(entry.name),
            )
            .map((entry) => path.join(repositoryPath, entry.name))
            .sort((a, b) => path.basename(a).localeCompare(path.basename(b), "en"))
        : [];
      const repositoryName = repository.url
        .split("/")
        .at(-1)
        ?.replace(/\.git$/, "");
      return {
        id: `project:${repository.path}`,
        name: repositoryName || repository.name,
        version: readRepositoryVersion(repositoryPath),
        license: detectLicense(licenseFiles[0]),
        visibility: repository.visibility,
        authors: ["AstralSightStudios"],
        repository: repository.url,
        documentIds: findPackageDocuments(repositoryPath),
      };
    })
    .sort((a, b) => a.name.localeCompare(b.name, "en"));
}

function generateFrontendLicenses() {
  const grouped = runJson("pnpm", ["licenses", "list", "--prod", "--json"]);
  const result = [];
  for (const [license, records] of Object.entries(grouped)) {
    for (const record of records) {
      const versions = Array.isArray(record.versions) ? record.versions : [];
      const paths = Array.isArray(record.paths) ? record.paths : [];
      const count = Math.max(versions.length, paths.length, 1);
      for (let index = 0; index < count; index += 1) {
        const packageDir = paths[index] ?? paths[0];
        const version = versions[index] ?? versions[0] ?? "unknown";
        let packageJson = {};
        if (packageDir) {
          const packageJsonPath = path.join(packageDir, "package.json");
          if (fs.existsSync(packageJsonPath)) {
            packageJson = JSON.parse(fs.readFileSync(packageJsonPath, "utf8"));
          }
        }
        result.push({
          id: `frontend:${record.name}@${version}`,
          name: record.name,
          version,
          license,
          authors: [normalizePerson(packageJson.author ?? record.author)].filter(Boolean),
          repository:
            normalizeRepository(packageJson.repository) ??
            normalizeRepository(record.repository) ??
            packageJson.homepage ??
            record.homepage,
          documentIds: packageDir ? findPackageDocuments(packageDir) : [],
        });
      }
    }
  }
  return result.sort((a, b) =>
    a.name.localeCompare(b.name, "en") || a.version.localeCompare(b.version, "en"),
  );
}

function reachableNativePackageIds(metadata, appPackageId) {
  const nodeMap = new Map(metadata.resolve.nodes.map((node) => [node.id, node]));
  const visited = new Set();
  const pending = [appPackageId];
  while (pending.length > 0) {
    const packageId = pending.pop();
    if (!packageId || visited.has(packageId)) continue;
    visited.add(packageId);
    const node = nodeMap.get(packageId);
    if (!node) continue;
    for (const dependency of node.deps) {
      const isRuntimeDependency = dependency.dep_kinds.some(
        (kind) => kind.kind === null,
      );
      if (isRuntimeDependency && !visited.has(dependency.pkg)) {
        pending.push(dependency.pkg);
      }
    }
  }
  return visited;
}

function generateNativeLicenses() {
  const metadata = runJson("cargo", [
    "metadata",
    "--manifest-path",
    "src-tauri/modules/app/Cargo.toml",
    "--format-version",
    "1",
    "--locked",
    "--offline",
  ]);
  const appPackage = metadata.packages.find(
    (packageInfo) => packageInfo.name === "AstroBox-ng",
  );
  if (!appPackage) throw new Error("AstroBox-ng package not found in Cargo metadata");
  const reachable = reachableNativePackageIds(metadata, appPackage.id);
  const packages = metadata.packages
    .filter((packageInfo) => reachable.has(packageInfo.id) && packageInfo.source)
    .map((packageInfo) => {
      const packageDir = path.dirname(packageInfo.manifest_path);
      const sourceId = createHash("sha256")
        .update(packageInfo.id)
        .digest("hex")
        .slice(0, 8);
      return {
        id: `native:${packageInfo.name}@${packageInfo.version}:${sourceId}`,
        name: packageInfo.name,
        version: packageInfo.version,
        license: packageInfo.license ?? "Unknown",
        authors: Array.isArray(packageInfo.authors) ? packageInfo.authors : [],
        repository: packageInfo.repository ?? packageInfo.homepage,
        documentIds: findPackageDocuments(packageDir, packageInfo.license_file),
      };
    });
  return {
    appVersion: appPackage.version,
    packages: packages.sort((a, b) =>
      a.name.localeCompare(b.name, "en") ||
      a.version.localeCompare(b.version, "en") ||
      a.id.localeCompare(b.id, "en"),
    ),
  };
}

function generateProjectLicenses(nativeVersion) {
  const readme = fs.readFileSync(path.join(projectRoot, "README.md"), "utf8");
  const termsStart = readme.indexOf("### 额外条款 / Additional Terms");
  const termsEnd = readme.indexOf("\n## ", termsStart + 1);
  const additionalTerms =
    termsStart >= 0
      ? normalizeText(readme.slice(termsStart, termsEnd >= 0 ? termsEnd : undefined))
      : "";
  const additionalTermsId = additionalTerms
    ? (() => {
        const id = createHash("sha256")
          .update(additionalTerms)
          .digest("hex")
          .slice(0, 16);
        documents.set(id, {
          id,
          name: "ADDITIONAL-TERMS.md",
          text: additionalTerms,
        });
        return id;
      })()
    : null;

  return [
    {
      id: "project:astrobox-ng",
      name: "AstroBox-NG",
      version: nativeVersion,
      license: "AGPL-3.0 with additional attribution terms",
      visibility: "public",
      authors: ["AstralSightStudios"],
      repository: "https://github.com/AstralSightStudios/AstroBox-NG",
      documentIds: [
        addDocument(path.join(projectRoot, "LICENSE")),
        additionalTermsId,
      ].filter(Boolean),
    },
    ...parseRepositories(),
  ];
}

console.log("正在生成开源许可清单……");
const frontend = generateFrontendLicenses();
const nativeResult = generateNativeLicenses();
const project = generateProjectLicenses(nativeResult.appVersion);
const output = {
  schemaVersion: 1,
  project,
  frontend,
  native: nativeResult.packages,
  documents: [...documents.values()].sort((a, b) => a.id.localeCompare(b.id)),
};

fs.mkdirSync(path.dirname(outputPath), { recursive: true });
fs.writeFileSync(outputPath, `${JSON.stringify(output)}\n`);
console.log(
  `已生成 ${project.length} 项项目许可、${frontend.length} 项前端依赖和 ${nativeResult.packages.length} 项原生依赖。`,
);
