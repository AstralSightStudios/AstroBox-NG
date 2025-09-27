const fs = require('fs');
const os = require('os')
const { execSync } = require('child_process');

const licenseDir = 'src/licences';

if (!fs.existsSync(licenseDir)) {
  fs.mkdirSync(licenseDir, { recursive: true });
}

fs.writeFileSync(`${licenseDir}/npm.ts`, 'export default ');
fs.writeFileSync(`${licenseDir}/crate.ts`, 'export default ');

console.log("Generating licenses, this may take a while...")

try {
  var command = 'pm -pm pnpm licenses ls -P -json';
  if (os.platform != "win32") {
    command = './pm -pm pnpm licenses ls -P -json'
  }
  execSync(
    command,
    { stdio: ['ignore', fs.openSync(`${licenseDir}/npm.ts`, 'a'), 'inherit'] }
  );

  execSync(
    'cargo license -j --direct-deps-only --manifest-path src-tauri/Cargo.toml',
    { stdio: ['ignore', fs.openSync(`${licenseDir}/crate.ts`, 'a'), 'inherit'] }
  );

  console.log("Generating licenses success!")
} catch(error) {
  console.log("Generating licenses fail, skiped!")
  fs.writeFileSync(`${licenseDir}/npm.ts`, 'export default { }');
  fs.writeFileSync(`${licenseDir}/crate.ts`, 'export default[{\"name\":\"Astro-Box\",\"version\":\"1.5.2\",\"authors\":\"AstralSightStudios\",\"repository\":\"\",\"license\":\"\",\"license_file\":null,\"description\":\"A multifunctional toolbox designed for Xiaomi Vela wearable devices\"}]');
}